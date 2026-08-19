//! Layer 0 · 密码保险箱 (Vault)
//!
//! 客户端首次使用提示创建密码，此后所有落盘数据（SQLite 数据库）都用
//! 「密码派生的密钥」加密存储。
//!
//! - 密钥派生：PBKDF2-HMAC-SHA256(密码, 随机 salt, 迭代次数) → 32 字节
//!   主密钥 master；再经 HKDF-Expand(master, info) 域分离出子密钥：
//!   `db_key`（SQLCipher 数据库密钥，info = `privchat:sqlcipher-db:v1`）。
//!   校验串仍用 master 直接加密，解锁逻辑与旧 vault.json 保持兼容。
//! - 加解密：ChaCha20-Poly1305 AEAD，每次加密随机 12 字节 nonce，
//!   密文文件格式为 `nonce(12) || ciphertext || tag(16)` —— **nonce 直接
//!   记录在密文文件头部**，解密时从文件中读出。
//! - 持久化：`vault.json` 只存 salt 与迭代次数（不存密码、不存密钥），
//!   外加一个用派生密钥加密的固定明文校验串，用于登录时验证密码正确性。

use std::path::Path;

use anyhow::{anyhow, Result};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
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
/// HKDF-Expand 域分离标签：各用途子密钥互不相同。
const INFO_DB_KEY: &[u8] = b"privchat:sqlcipher-db:v1";

/// 密码保险箱：持有 PBKDF2 派生的主密钥，按用途经 HKDF 分化子密钥。
pub struct Vault {
    master: [u8; 32],
}

impl Vault {
    /// 首次使用：生成随机 salt，派生主密钥，写入 `vault.json` 并返回保险箱。
    pub fn create(data_dir: &Path, password: &str, iterations: u32) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        let mut salt = [0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        let master = derive_key(password, &salt, iterations);
        let verifier = encrypt_with_key(&master, VERIFIER)?;
        let meta = serde_json::json!({
            "salt": hex_encode(&salt),
            "iterations": iterations,
            "verifier": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &verifier),
        });
        std::fs::write(data_dir.join(VAULT_FILE), serde_json::to_vec(&meta)?)?;
        Ok(Self { master })
    }

    /// 后续使用：读取 `vault.json`，用输入密码派生主密钥并校验密码正确性。
    pub fn unlock(data_dir: &Path, password: &str) -> Result<Self> {
        let bytes = std::fs::read(data_dir.join(VAULT_FILE))
            .map_err(|_| anyhow!("vault not initialized"))?;
        let meta: serde_json::Value = serde_json::from_slice(&bytes)?;
        let salt = hex_decode(
            meta["salt"]
                .as_str()
                .ok_or_else(|| anyhow!("vault missing salt"))?,
        )?;
        let iterations = meta["iterations"]
            .as_u64()
            .ok_or_else(|| anyhow!("vault missing iterations"))? as u32;
        let master = derive_key(password, &salt, iterations);
        let verifier_b64 = meta["verifier"]
            .as_str()
            .ok_or_else(|| anyhow!("vault missing verifier"))?;
        let verifier =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, verifier_b64)?;
        let plain = decrypt_with_key(&master, &verifier).map_err(|_| anyhow!("wrong password"))?;
        if plain != VERIFIER {
            return Err(anyhow!("wrong password"));
        }
        Ok(Self { master })
    }

    /// 保险箱是否存在（= 是否已完成首次密码设置）。
    pub fn is_initialized(data_dir: &Path) -> bool {
        data_dir.join(VAULT_FILE).exists()
    }

    /// SQLCipher 数据库密钥：HKDF-Expand(master, "privchat:sqlcipher-db:v1")。
    pub fn db_key(&self) -> [u8; 32] {
        derive_subkey(&self.master, INFO_DB_KEY)
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

/// HKDF-Expand(master, info) 派生 32 字节域分离子密钥。
fn derive_subkey(master: &[u8; 32], info: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, master);
    let mut okm = [0u8; 32];
    hk.expand(info, &mut okm)
        .expect("HKDF-Expand 32 bytes must not fail");
    okm
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
        let dir = std::env::temp_dir().join(format!("privchat-vault-{name}-{}", std::process::id()));
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
    fn subkeys_domain_separated() {
        let dir = temp_dir("subkeys");
        let v = Vault::create(&dir, "pw", 1000).expect("create");
        // 不同 info 标签必须派生不同子密钥（域分离）。
        let other = derive_subkey(&v.master, b"privchat:other:v1");
        assert_ne!(v.db_key(), other);
        // 同一保险箱重复获取子密钥必须稳定。
        assert_eq!(v.db_key(), v.db_key());
        // 重新解锁后子密钥与创建时一致。
        let v2 = Vault::unlock(&dir, "pw").expect("unlock");
        assert_eq!(v2.db_key(), v.db_key());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
