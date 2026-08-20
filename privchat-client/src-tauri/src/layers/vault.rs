//! Layer 0 · 密码保险箱 (Vault)
//!
//! 客户端首次使用提示创建密码，此后所有落盘数据（SQLite 数据库）都用
//! 「密码派生的密钥」加密存储。
//!
//! - 密钥派生：PBKDF2-HMAC-SHA256(密码, 随机 salt, 迭代次数) → 32 字节
//!   密钥加密密钥 KEK。
//! - 数据库密钥：随机生成独立的 32 字节 DEK，由 KEK 加密后写入 vault.json。
//! - 加解密：ChaCha20-Poly1305 AEAD，每次加密随机 12 字节 nonce，
//!   密文文件格式为 `nonce(12) || ciphertext || tag(16)` —— **nonce 直接
//!   记录在密文文件头部**，解密时从文件中读出。
//! - 持久化：`vault.json` 只存 salt 与迭代次数（不存密码、不存密钥），
//!   外加一个用派生密钥加密的固定明文校验串，用于登录时验证密码正确性。

use std::path::Path;

use anyhow::{anyhow, Result};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use pbkdf2::pbkdf2_hmac;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;

/// 生产默认迭代次数（OWASP 对 PBKDF2-HMAC-SHA256 的推荐量级）。
pub const DEFAULT_ITERATIONS: u32 = 600_000;

pub const VAULT_FILE: &str = "vault.json";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
/// 校验串明文：解密后比对，判断密码是否正确。
const VERIFIER: &[u8] = b"privchat-vault-v1";
const FORMAT: u64 = 2;
const KDF: &str = "pbkdf2-hmac-sha256";
const CIPHER: &str = "chacha20-poly1305";
/// 密码保险箱：持有随机生成、由用户密码派生 KEK 包裹的数据库密钥。
pub struct Vault {
    db_key: [u8; 32],
}

impl Vault {
    /// 首次使用：生成随机 salt，派生主密钥，写入 `vault.json` 并返回保险箱。
    pub fn create(data_dir: &Path, password: &str, iterations: u32) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        let (vault, bytes) = Self::prepare_new(password, iterations)?;
        std::fs::write(data_dir.join(VAULT_FILE), bytes)?;
        Ok(vault)
    }

    /// 后续使用：读取并解包由密码派生 KEK 加密的数据库密钥。
    pub fn unlock(data_dir: &Path, password: &str) -> Result<Self> {
        let bytes = std::fs::read(data_dir.join(VAULT_FILE))
            .map_err(|_| anyhow!("vault not initialized"))?;
        Self::unlock_bytes(&bytes, password)
    }

    fn unlock_bytes(bytes: &[u8], password: &str) -> Result<Self> {
        let meta: serde_json::Value = serde_json::from_slice(bytes)?;
        if meta["format"].as_u64() != Some(FORMAT) {
            return Err(anyhow!("incompatible vault format"));
        }
        if meta["kdf"].as_str() != Some(KDF) || meta["cipher"].as_str() != Some(CIPHER) {
            return Err(anyhow!("incompatible vault format"));
        }
        let salt = hex_decode(
            meta["salt"]
                .as_str()
                .ok_or_else(|| anyhow!("vault missing salt"))?,
        )?;
        let iterations = meta["iterations"]
            .as_u64()
            .ok_or_else(|| anyhow!("vault missing iterations"))? as u32;
        let kek = derive_key(password, &salt, iterations);
        let wrapped = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            meta["wrapped_key"]
                .as_str()
                .ok_or_else(|| anyhow!("vault missing wrapped key"))?,
        )?;
        let db_key: [u8; 32] = decrypt_with_key(&kek, &wrapped)
            .map_err(|_| anyhow!("wrong password"))?
            .try_into()
            .map_err(|_| anyhow!("invalid database key"))?;
        let verifier_b64 = meta["verifier"]
            .as_str()
            .ok_or_else(|| anyhow!("vault missing verifier"))?;
        let verifier =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, verifier_b64)?;
        let plain = decrypt_with_key(&kek, &verifier).map_err(|_| anyhow!("wrong password"))?;
        if plain != VERIFIER {
            return Err(anyhow!("wrong password"));
        }
        Ok(Self { db_key })
    }

    /// 保险箱是否存在（= 是否已完成首次密码设置）。
    pub fn is_initialized(data_dir: &Path) -> bool {
        data_dir.join(VAULT_FILE).exists()
    }

    /// SQLCipher 数据库密钥（随机 DEK）。
    pub fn db_key(&self) -> [u8; 32] {
        self.db_key
    }

    pub fn prepare_new(password: &str, iterations: u32) -> Result<(Self, Vec<u8>)> {
        let mut db_key = [0u8; 32];
        OsRng.fill_bytes(&mut db_key);
        Self::prepare_with_db_key(password, iterations, db_key)
    }

    pub fn prepare_with_db_key(
        password: &str,
        iterations: u32,
        db_key: [u8; 32],
    ) -> Result<(Self, Vec<u8>)> {
        let mut salt = [0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        let kek = derive_key(password, &salt, iterations);
        let wrapped_db_key = encrypt_with_key(&kek, &db_key)?;
        let verifier = encrypt_with_key(&kek, VERIFIER)?;
        let meta = serde_json::json!({
            "format": FORMAT,
            "kdf": KDF,
            "cipher": CIPHER,
            "salt": hex_encode(&salt),
            "iterations": iterations,
            "wrapped_key": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, wrapped_db_key),
            "verifier": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, verifier),
        });
        Ok((Self { db_key }, serde_json::to_vec(&meta)?))
    }

    /// 使用旧密码解包 DEK，验证新 vault 后再原子替换正式文件。
    pub fn change_password_atomic(
        data_dir: &Path,
        old_password: &str,
        new_password: &str,
        iterations: u32,
    ) -> Result<()> {
        let current = Self::unlock(data_dir, old_password)?;
        let (_, bytes) = Self::prepare_with_db_key(new_password, iterations, current.db_key)?;
        let tmp = data_dir.join("vault.json.tmp");
        std::fs::write(&tmp, &bytes)?;
        let validation = Self::unlock_bytes(&std::fs::read(&tmp)?, new_password)?;
        if validation.db_key != current.db_key {
            let _ = std::fs::remove_file(&tmp);
            return Err(anyhow!("database key mismatch"));
        }
        if let Err(error) = std::fs::rename(&tmp, data_dir.join(VAULT_FILE)) {
            let _ = std::fs::remove_file(&tmp);
            return Err(error.into());
        }
        Ok(())
    }
}

/// 用派生密钥加密：`nonce(12) || ciphertext || tag(16)`，nonce 随机且记录在文件头。
pub fn encrypt_with_key(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|e| anyhow!("encrypt failed: {e}"))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// 用派生密钥解密：从文件头读取 nonce，还原明文。
pub fn decrypt_with_key(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < NONCE_LEN {
        return Err(anyhow!("ciphertext too short"));
    }
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(&blob[..NONCE_LEN]), &blob[NONCE_LEN..])
        .map_err(|e| anyhow!("decrypt failed: {e}"))
}

/// PBKDF2-HMAC-SHA256 派生 32 字节主密钥。
fn derive_key(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, &mut key);
    key
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(s: &str) -> Result<[u8; SALT_LEN]> {
    if s.len() != SALT_LEN * 2 {
        return Err(anyhow!("bad salt hex length"));
    }
    let mut out = [0u8; SALT_LEN];
    for (i, byte) in out.iter_mut().enumerate() {
        let hi = hex_val(s.as_bytes()[i * 2])?;
        let lo = hex_val(s.as_bytes()[i * 2 + 1])?;
        *byte = (hi << 4) | lo;
    }
    Ok(out)
}

fn hex_val(b: u8) -> Result<u8> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(anyhow!("invalid hex char: {}", b as char)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("privchat-vault-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn create_unlock_roundtrip() {
        let dir = temp_dir("roundtrip");
        assert!(!Vault::is_initialized(&dir));
        let v = Vault::create(&dir, "hunter2", 1000).expect("create");
        assert!(Vault::is_initialized(&dir));

        // 密文包含 nonce 前缀（记录在文件中）。
        let blob = encrypt_with_key(&v.db_key(), b"hello").expect("encrypt");
        assert!(blob.len() >= NONCE_LEN);
        let plain = decrypt_with_key(&v.db_key(), &blob).expect("decrypt");
        assert_eq!(plain, b"hello");

        // 用正确密码解锁可派生同一 db_key。
        let v2 = Vault::unlock(&dir, "hunter2").expect("unlock");
        assert_eq!(v2.db_key(), v.db_key());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn wrong_password_rejected() {
        let dir = temp_dir("wrongpw");
        Vault::create(&dir, "correct", 1000).expect("create");
        assert!(Vault::unlock(&dir, "wrong").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn distinct_nonces_in_files() {
        let key = [7u8; 32];
        let a = encrypt_with_key(&key, b"same").unwrap();
        let b = encrypt_with_key(&key, b"same").unwrap();
        assert_ne!(a, b, "same plaintext must not produce identical ciphertext");
        assert_eq!(
            decrypt_with_key(&key, &a).unwrap(),
            decrypt_with_key(&key, &b).unwrap()
        );
    }

    #[test]
    fn tamper_detected() {
        let key = [9u8; 32];
        let mut blob = encrypt_with_key(&key, b"secret").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert!(decrypt_with_key(&key, &blob).is_err());
    }

    #[test]
    fn database_key_is_random_and_wrapped() {
        let dir = temp_dir("wrapped-key");
        let v = Vault::create(&dir, "pw", 1000).expect("create");
        let second = Vault::create(&temp_dir("wrapped-key-2"), "pw", 1000).expect("create");
        assert_ne!(v.db_key(), second.db_key());
        let v2 = Vault::unlock(&dir, "pw").expect("unlock");
        assert_eq!(v2.db_key(), v.db_key());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn password_change_keeps_database_key_and_restarts() {
        let dir = temp_dir("password-change");
        let original = Vault::create(&dir, "old", 1000).expect("create");
        let key = original.db_key();
        Vault::change_password_atomic(&dir, "old", "new", 1000).expect("change password");
        assert!(Vault::unlock(&dir, "old").is_err());
        let reopened = Vault::unlock(&dir, "new").expect("reopen with new password");
        assert_eq!(reopened.db_key(), key);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_vault_is_rejected_without_replacement() {
        let dir = temp_dir("corrupt");
        Vault::create(&dir, "old", 1000).expect("create");
        std::fs::write(dir.join(VAULT_FILE), b"not json").expect("corrupt");
        let error = match Vault::unlock(&dir, "old") {
            Ok(_) => panic!("old password should fail"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("expected") || error.contains("key must be a string"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
