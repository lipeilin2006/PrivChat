//! Layer 0.5 · 本地持久化 (SQLite + SQLCipher 全库加密)
//!
//! 整个数据库文件由 SQLCipher 加密（AES-256，页级加密，表结构、行数、
//! 索引全部密文），文件头为随机 salt，不再是明文 "SQLite format 3"。
//! 加密密钥 = 密码经 PBKDF2 派生主密钥后，再经 HKDF-Expand 域分离得到的
//! `db_key`（见 vault.rs），通过 `PRAGMA key = "x'<hex>'"` 传入（raw key，
//! 不经 SQLCipher 二次 KDF）。
//!
//! 应用层在加密库内直接读写明文，由 SQLCipher 负责落盘加密，因此不再
//! 需要字段级加密（此前为逐字段 ChaCha20，已移除）。
//!
//! 结构：
//! ```text
//! contacts      (peer_id PK, local_id, name, ticket)
//! messages      (peer_id, msg_id, sender, text, time)
//! mailbox_peers (peer_id PK)
//! identities    (local_id PK, secret BLOB)
//! ratchets      (local_id, peer_id, state BLOB)  —— 双棘轮会话状态
//! ```
//!
//! 兼容性：不再迁移旧库。若已存在旧版明文 SQLite 文件（文件头为
//! "SQLite format 3"），直接删除重建全新加密库；密钥错误的加密库
//! 则返回错误，绝不误删有效数据。

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};

use super::app::{ChatMessage, Contact};

/// 明文 SQLite 文件头（识别旧版明文旧库，予以删除重建）。
const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";

/// SQLite 持久化门面。连接用 `Mutex` 包裹以保证线程安全，且所有操作
/// 都很小（几十到几百行），同步执行即可。
pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// 打开（或创建）SQLCipher 加密数据库。旧版明文库会被删除重建。
    pub fn open(data_dir: &Path, key: [u8; 32]) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        let db_path = data_dir.join("privchat.db");
        let conn = Self::open_with_key(&db_path, &key)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS contacts (
                 peer_id TEXT PRIMARY KEY,
                 local_id TEXT NOT NULL,
                 name TEXT,
                 ticket TEXT
             );
             CREATE TABLE IF NOT EXISTS messages (
                 peer_id TEXT NOT NULL,
                 msg_id TEXT NOT NULL,
                 sender TEXT NOT NULL,
                 text TEXT NOT NULL,
                 time INTEGER NOT NULL,
                 PRIMARY KEY (peer_id, msg_id)
             );
             CREATE TABLE IF NOT EXISTS mailbox_peers (
                 peer_id TEXT PRIMARY KEY
             );
             CREATE TABLE IF NOT EXISTS identities (
                 local_id TEXT PRIMARY KEY,
                 secret BLOB NOT NULL
             );
             CREATE TABLE IF NOT EXISTS ratchets (
                 local_id TEXT NOT NULL,
                 peer_id TEXT NOT NULL,
                 state BLOB NOT NULL,
                 PRIMARY KEY (local_id, peer_id)
             );",
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// 以派生密钥打开加密库。`PRAGMA key` 必须是连接后的首个操作。
    /// 旧版明文库（文件头 "SQLite format 3"）删除重建；密钥错误的加密
    /// 库返回错误（不误删有效数据）。
    fn open_with_key(db_path: &Path, key: &[u8; 32]) -> Result<Connection> {
        if is_legacy_plaintext(db_path) {
            for suffix in ["", "-wal", "-shm"] {
                let _ = std::fs::remove_file(db_path.with_extension(format!("db{suffix}")));
            }
        }
        let conn = Connection::open(db_path)?;
        let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
        conn.execute_batch(&format!("PRAGMA key = \"x'{hex}'\";"))?;
        // 触发首页读取：密钥错误或非加密库会在此报错。
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        })
        .map_err(|e| anyhow!("cannot open database with key: {e}"))?;
        Ok(conn)
    }

    /// 联系人表整表读入内存（peer_id → Contact）。
    pub fn load_contacts(&self) -> Result<HashMap<String, Contact>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT peer_id, local_id, name, ticket FROM contacts")?;
        let rows = stmt.query_map([], |row| {
            Ok(Contact {
                peer_id: row.get(0)?,
                local_id: row.get(1)?,
                name: row.get(2)?,
                ticket: row.get(3)?,
            })
        })?;
        let mut out = HashMap::new();
        for r in rows {
            let c = r?;
            out.insert(c.peer_id.clone(), c);
        }
        Ok(out)
    }

    /// 整表写回联系人（先删后插，数据量小，保证与内存态一致）。
    pub fn save_contacts(&self, contacts: &HashMap<String, Contact>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM contacts", [])?;
        let mut stmt = conn.prepare(
            "INSERT INTO contacts (peer_id, local_id, name, ticket) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for c in contacts.values() {
            stmt.execute(params![c.peer_id, c.local_id, c.name, c.ticket])?;
        }
        Ok(())
    }

    /// 消息历史整表读入内存（peer_id → 消息列表，按 time 升序）。
    pub fn load_history(&self) -> Result<HashMap<String, Vec<ChatMessage>>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT peer_id, msg_id, sender, text, time FROM messages",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ChatMessage {
                    id: row.get(1)?,
                    from: row.get(2)?,
                    text: row.get(3)?,
                    time: row.get::<_, i64>(4)? as u64,
                },
            ))
        })?;
        let mut out: HashMap<String, Vec<ChatMessage>> = HashMap::new();
        for r in rows {
            let (peer_id, msg) = r?;
            out.entry(peer_id).or_default().push(msg);
        }
        for msgs in out.values_mut() {
            msgs.sort_by_key(|m| m.time);
        }
        Ok(out)
    }

    /// 整表写回历史（先删后插，保证幂等去重）。
    pub fn save_history(&self, history: &HashMap<String, Vec<ChatMessage>>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM messages", [])?;
        let mut stmt = conn.prepare(
            "INSERT INTO messages (peer_id, msg_id, sender, text, time) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for (peer_id, msgs) in history {
            for m in msgs {
                stmt.execute(params![peer_id, m.id, m.from, m.text, m.time as i64])?;
            }
        }
        Ok(())
    }

    /// mailbox 节点列表。
    pub fn load_mailbox_peers(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT peer_id FROM mailbox_peers")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// 写回 mailbox 节点列表（先删后插）。
    pub fn save_mailbox_peers(&self, peers: &[String]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM mailbox_peers", [])?;
        let mut stmt = conn.prepare("INSERT INTO mailbox_peers (peer_id) VALUES (?1)")?;
        for p in peers {
            stmt.execute(params![p])?;
        }
        Ok(())
    }

    /// 保存一个专属身份的私钥（SecretKey 字节）。整库已加密，直接存 BLOB。
    pub fn save_identity(&self, local_id: &str, secret: &[u8]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO identities (local_id, secret) VALUES (?1, ?2)",
            params![local_id, secret],
        )?;
        Ok(())
    }

    /// 读取全部专属身份私钥（local_id → SecretKey 字节）。
    pub fn load_identities(&self) -> Result<HashMap<String, Vec<u8>>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT local_id, secret FROM identities")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;
        let mut out = HashMap::new();
        for r in rows {
            let (local_id, secret) = r?;
            out.insert(local_id, secret);
        }
        Ok(out)
    }

    /// 删除一个专属身份私钥（删除联系人时调用）。
    pub fn delete_identity(&self, local_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM identities WHERE local_id = ?1",
            params![local_id],
        )?;
        Ok(())
    }

    /// 保存一个双棘轮会话状态（UPSERT）。`state` 为 `crypto::Ratchet::to_bytes`。
    pub fn save_ratchet(&self, local_id: &str, peer_id: &str, state: &[u8]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO ratchets (local_id, peer_id, state) VALUES (?1, ?2, ?3)",
            params![local_id, peer_id, state],
        )?;
        Ok(())
    }

    /// 读取全部双棘轮会话状态 `(local_id, peer_id, state)`。
    pub fn load_ratchets(&self) -> Result<Vec<(String, String, Vec<u8>)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT local_id, peer_id, state FROM ratchets")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Vec<u8>>(2)?))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// 删除一个专属身份的全部双棘轮会话（删除联系人时调用）。
    pub fn delete_ratchet(&self, local_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM ratchets WHERE local_id = ?1",
            params![local_id],
        )?;
        Ok(())
    }
}

/// 判断文件是否为旧版明文 SQLite（文件头 "SQLite format 3\0"）。
fn is_legacy_plaintext(db_path: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(db_path) else {
        return false;
    };
    let mut hdr = [0u8; 16];
    if f.read_exact(&mut hdr).is_err() {
        return false;
    }
    &hdr == SQLITE_HEADER
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [0x42; 32];
    const WRONG_KEY: [u8; 32] = [0x99; 32];

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("privchat-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn read_header(path: &std::path::Path) -> [u8; 16] {
        let mut f = std::fs::File::open(path).unwrap();
        let mut hdr = [0u8; 16];
        f.read_exact(&mut hdr).unwrap();
        hdr
    }

    fn sample_contacts() -> HashMap<String, Contact> {
        let mut contacts = HashMap::new();
        contacts.insert(
            "peer1".to_string(),
            Contact {
                peer_id: "peer1".to_string(),
                local_id: "local1".to_string(),
                name: Some("Alice".to_string()),
                ticket: Some("ticket1".to_string()),
            },
        );
        contacts
    }

    fn sample_history() -> HashMap<String, Vec<ChatMessage>> {
        let mut history = HashMap::new();
        history.insert(
            "peer1".to_string(),
            vec![ChatMessage {
                id: "m1".to_string(),
                from: "local1".to_string(),
                text: "hi".to_string(),
                time: 1700000000000,
            }],
        );
        history
    }

    #[test]
    fn sqlite_roundtrip_encrypted_file() {
        let dir = temp_dir("enc");
        let store = Store::open(&dir, KEY).expect("open");

        // 新库文件头应为随机 salt，而非明文 SQLite 头。
        assert_ne!(&read_header(&dir.join("privchat.db")), b"SQLite format 3\0");

        let contacts = sample_contacts();
        store.save_contacts(&contacts).expect("save contacts");
        let loaded = store.load_contacts().expect("load contacts");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded["peer1"].name.as_deref(), Some("Alice"));

        let history = sample_history();
        store.save_history(&history).expect("save history");
        let h = store.load_history().expect("load history");
        assert_eq!(h["peer1"].len(), 1);
        assert_eq!(h["peer1"][0].text, "hi");
        assert_eq!(h["peer1"][0].time, 1700000000000);

        store
            .save_mailbox_peers(&["mb1".to_string(), "mb2".to_string()])
            .expect("save mailbox");
        assert_eq!(store.load_mailbox_peers().unwrap(), vec!["mb1", "mb2"]);

        // 落盘文件（含 WAL 后主库）不得含明文（搜索原始字符串应失败）。
        let raw = std::fs::read(dir.join("privchat.db")).unwrap();
        assert!(
            !raw.windows(5).any(|w| w == b"Alice"),
            "plaintext 'Alice' must not appear in db file"
        );

        // 重开：数据仍在（持久化生效）。
        drop(store);
        let store2 = Store::open(&dir, KEY).expect("reopen");
        assert_eq!(store2.load_contacts().unwrap().len(), 1);
        assert_eq!(store2.load_history().unwrap()["peer1"].len(), 1);
        assert_eq!(store2.load_mailbox_peers().unwrap().len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn wrong_key_cannot_open() {
        let dir = temp_dir("wrongkey");
        let store = Store::open(&dir, KEY).expect("open");
        store.save_contacts(&sample_contacts()).expect("save");
        drop(store);

        // 错误密钥打开加密库 → 打开即报错（SQLCipher 首页解密失败）。
        assert!(
            Store::open(&dir, WRONG_KEY).is_err(),
            "wrong key must fail open"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_plaintext_deleted_recreated() {
        let dir = temp_dir("legacy");
        // 构造旧版明文库（标准 SQLite 文件头 + 明文数据）。
        let conn = rusqlite::Connection::open(dir.join("privchat.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE contacts (peer_id TEXT PRIMARY KEY, local_id TEXT NOT NULL, name TEXT, ticket TEXT);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO contacts (peer_id, local_id, name, ticket) VALUES ('peerX','localX','Bob','tkX')",
            [],
        )
        .unwrap();
        drop(conn);

        // 旧版明文库不做迁移，直接删除重建为加密库。
        let store = Store::open(&dir, KEY).expect("open fresh");
        assert!(store.load_contacts().unwrap().is_empty(), "legacy data not migrated");
        assert_ne!(&read_header(&dir.join("privchat.db")), b"SQLite format 3\0");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_ok() {
        let dir = temp_dir("empty");
        let store = Store::open(&dir, KEY).expect("open empty");
        assert!(store.load_contacts().unwrap().is_empty());
        assert!(store.load_history().unwrap().is_empty());
        assert!(store.load_mailbox_peers().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ratchets_roundtrip() {
        let dir = temp_dir("ratchets");
        let store = Store::open(&dir, KEY).expect("open");

        let state1: [u8; 16] = [0x01; 16];
        let state2: [u8; 16] = [0x02; 16];
        store
            .save_ratchet("local1", "peerA", &state1)
            .expect("save r1");
        store
            .save_ratchet("local1", "peerB", &state2)
            .expect("save r2");

        let all = store.load_ratchets().expect("load");
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|(l, p, s)| l == "local1" && p == "peerA" && s == &state1));
        assert!(all.iter().any(|(l, p, s)| l == "local1" && p == "peerB" && s == &state2));

        // UPSERT：同一键覆盖。
        store
            .save_ratchet("local1", "peerA", &state2)
            .expect("upsert");
        let all = store.load_ratchets().expect("reload");
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|(l, p, s)| l == "local1" && p == "peerA" && s == &state2));

        // 删除按 local_id 清除全部会话。
        store.delete_ratchet("local1").expect("delete");
        assert!(store.load_ratchets().unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn identities_roundtrip_encrypted_file() {
        let dir = temp_dir("idents");
        let store = Store::open(&dir, KEY).expect("open");

        let secret: [u8; 32] = [0x77; 32];
        store.save_identity("local1", &secret).expect("save identity");
        store
            .save_identity("local2", &[0x11; 32])
            .expect("save identity2");

        let loaded = store.load_identities().expect("load identities");
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded["local1"].as_slice(), &secret);
        assert_eq!(loaded["local2"].as_slice(), &[0x11; 32]);

        store.delete_identity("local1").expect("delete identity");
        let after = store.load_identities().expect("reload");
        assert_eq!(after.len(), 1);
        assert!(after.contains_key("local2"));

        // 落盘不含明文私钥字节（整库加密）。
        let raw = std::fs::read(dir.join("privchat.db")).unwrap();
        assert!(
            !raw.windows(32).any(|w| w == &secret[..]),
            "plaintext secret must not appear in db file"
        );

        // 重开后持久化仍在。
        drop(store);
        let store2 = Store::open(&dir, KEY).expect("reopen");
        assert_eq!(store2.load_identities().unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}