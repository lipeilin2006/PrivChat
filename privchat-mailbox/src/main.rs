//! PrivChat Mailbox —— 离线消息缓存节点（可组网同步）
//!
//! 纯存储节点：不参与任何会话加密。客户端先做应用层加密（双棘轮，见
//! client 的 `crypto::Ratchet`），再把密文 `payload` 交给本节点暂存。
//! 本节点只按「接收方 peer_id」分类存取、按消息 ID 排序与删除，永不接触明文。
//!
//! ## 取回即删 + TTL
//! `fetch` 按 `msg_id` 升序返回该接收方队列的全部密文，返回后立即从本节点
//! 删除，并向 peer 广播 `sync_ack` 让网格同步删除其他副本。msg_id 由发送方
//! 从加密头的 `(gen, n)`（逻辑发送序）派生，故按 msg_id 排序即等于棘轮
//! 消息顺序；不含时间戳，不泄露收发时间。
//! 消息最长保留时间（TTL）可配置，到期由后台任务清除：
//! - 环境变量 `PRIVCHAT_MAILBOX_TTL_SECS`（秒，0 = 永不过期），或
//! - `data_dir/config.json` 的 `{"ttl_secs": <秒>}`；缺省 7 天。
//!
//! ## 组网同步（多跳、无环路）
//! 每个 mailbox 通过配置知道若干其他 mailbox 节点（可全连通、可链式/环状）。
//! 收到客户端消息后向所有 peer 广播 `sync`；收到 `ack`/`fetch` 删除后广播
//! `sync_ack`。广播消息携带完整消息，接收方按 msg_id 幂等去重，**仅在真正
//! 新增/删除时继续转发**（`INSERT OR IGNORE` 主键幂等保证每条消息每节点
//! 至多处理一次），因此多跳链路（client0→mb0→mb1→mb2→client1）可传播到底，
//! 而环状拓扑中节点第二次收到同一条消息时不再转发，环路/风暴从结构上被截断。
//! 客户端连任意一个 mailbox 节点都能 fetch 到全部待收消息。
//!
//! peer 配置（二选一，环境变量优先）：
//! - `PRIVCHAT_MAILBOX_PEERS="<peer_id>,<peer_id>,..."`，或
//! - `data_dir/mailboxes.json` = `["<peer_id>", ...]`

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use iroh::endpoint::Connection;
use iroh::protocol::{AcceptError, ProtocolHandler, Router};
use iroh::SecretKey;
use rusqlite::{params, Connection as SqlConn};
use tokio::sync::Mutex;

use privchat_mailbox::{MailboxRequest, MailboxResponse, Op, StoredMessage, ALPN, MAX_PAYLOAD};

/// 内存邮箱 + SQLite 持久化。
///
/// 单表 `messages`：
/// - `msg_id`      TEXT —— 发送方派生 ID，与 to_peer_id 组成主键，去重 + 排序
/// - `to_peer_id`  TEXT NOT NULL —— 接收方身份（按此分类队列）
/// - `msg`         BLOB NOT NULL —— 应用层密文
/// - `time`        INTEGER NOT NULL —— 入库时间（毫秒），TTL 到期清除依据
///
/// 另建 `idx_messages_to` 索引加速按接收方拉取。仅存密文与路由元数据，
/// 节点不接触明文。
#[derive(Clone)]
struct MailboxStore {
    conn: Arc<Mutex<SqlConn>>,
    path: PathBuf,
    /// 消息最长保留秒数；0 = 永不过期。
    ttl_secs: u64,
}

impl std::fmt::Debug for MailboxStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MailboxStore")
            .field("path", &self.path)
            .finish()
    }
}

impl MailboxStore {
    /// 打开（或创建）当前版本 SQLite 库并建立表结构。
    fn open(path: PathBuf, ttl_secs: u64) -> Result<Self> {
        let conn = SqlConn::open(&path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS messages (
                 msg_id      TEXT NOT NULL,
                 to_peer_id  TEXT NOT NULL,
                 msg         BLOB NOT NULL,
                 time        INTEGER NOT NULL DEFAULT 0,
                 PRIMARY KEY (to_peer_id, msg_id)
             );
             CREATE INDEX IF NOT EXISTS idx_messages_to ON messages(to_peer_id);",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path,
            ttl_secs,
        })
    }

    /// 存入一条消息；同一 (接收方,msg_id,payload) 幂等，不同 payload 冲突。
    async fn put(&self, msg: StoredMessage, time: u64) -> Result<bool> {
        let conn = self.conn.lock().await;
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO messages (msg_id, to_peer_id, msg, time) VALUES (?1, ?2, ?3, ?4)",
            params![msg.msg_id, msg.to_peer_id, msg.msg, time as i64],
        )?;
        if inserted == 1 {
            return Ok(true);
        }
        let same_payload: bool = conn.query_row(
            "SELECT msg = ?3 FROM messages WHERE to_peer_id = ?1 AND msg_id = ?2",
            params![msg.to_peer_id, msg.msg_id, msg.msg],
            |row| row.get(0),
        )?;
        if same_payload {
            Ok(false)
        } else {
            Err(anyhow!("msg_id conflict with different payload"))
        }
    }

    /// 拉取某接收方队列的全部密文（按 `msg_id` 升序），返回后**立即删除**
    /// （取回即删）。调用方负责向 peer 广播删除意图。
    async fn fetch(&self, recipient: &str) -> Result<Vec<StoredMessage>> {
        let conn = self.conn.lock().await;
        let mut out = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT msg_id, to_peer_id, msg FROM messages WHERE to_peer_id = ?1 ORDER BY msg_id",
            )?;
            let rows = stmt.query_map(params![recipient], |row| {
                Ok(StoredMessage {
                    msg_id: row.get(0)?,
                    to_peer_id: row.get(1)?,
                    msg: row.get(2)?,
                })
            })?;
            for r in rows {
                out.push(r?);
            }
        }
        if !out.is_empty() {
            let ids: Vec<String> = out.iter().map(|m| m.msg_id.clone()).collect();
            let placeholders = vec!["?"; ids.len()].join(",");
            let sql = format!(
                "DELETE FROM messages WHERE to_peer_id = ?1 AND msg_id IN ({placeholders})"
            );
            let mut stmt = conn.prepare(&sql)?;
            stmt.raw_bind_parameter(1, recipient)?;
            for (i, id) in ids.iter().enumerate() {
                stmt.raw_bind_parameter(i + 2, id)?;
            }
            stmt.raw_execute()?;
        }
        Ok(out)
    }

    /// 删除已超过 TTL 的消息，返回删除行数。
    async fn purge_expired(&self, now_ms: u64) -> Result<usize> {
        if self.ttl_secs == 0 {
            return Ok(0);
        }
        let cutoff = now_ms.saturating_sub(self.ttl_secs * 1000);
        let conn = self.conn.lock().await;
        let deleted = conn.execute(
            "DELETE FROM messages WHERE time < ?1",
            params![cutoff as i64],
        )?;
        Ok(deleted)
    }

    /// 删除对应 msg_id（仅限该接收方的队列），返回删除行数。由网格 `sync_ack`
    /// 触发（fetch 取回即删 / 节点间删除同步）；返回 0 表示本节点本就没有
    /// 这些消息（幂等，无需再转发，切断环路）。
    async fn ack(&self, recipient: &str, ids: &[String]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock().await;
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql =
            format!("DELETE FROM messages WHERE to_peer_id = ?1 AND msg_id IN ({placeholders})");
        let mut stmt = conn.prepare(&sql)?;
        stmt.raw_bind_parameter(1, recipient)?;
        for (i, id) in ids.iter().enumerate() {
            stmt.raw_bind_parameter(i + 2, id)?;
        }
        let deleted = stmt.raw_execute()?;
        Ok(deleted)
    }
}

/// 连接处理循环：读一个 bi-stream，解析请求，返回响应。
#[derive(Debug, Clone)]
struct MailboxHandler {
    store: MailboxStore,
    endpoint: iroh::Endpoint,
    /// 其他 mailbox 节点的 peer_id（全连通/链式/环状均可）。
    peers: Arc<Vec<String>>,
}

impl ProtocolHandler for MailboxHandler {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        // 拨号方的 TLS 身份：fetch 必须用自己身份拉自己的队列（防他人
        // 取回即删式 DoS）；put/sync 不校验（任意发送方 + PoW 防垃圾）。
        let remote_id = connection.remote_id().to_string();
        loop {
            let (mut send, mut recv) = match connection.accept_bi().await {
                Ok(bi) => bi,
                Err(_) => break,
            };

            let payload = match recv.read_to_end(MAX_PAYLOAD).await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("[mailbox] accept_bi read failed: {e:#}");
                    continue;
                }
            };

            let response = handle_request(self, &remote_id, &payload).await;
            let bytes = serde_json::to_vec(&response).unwrap_or_else(|_| b"{}".to_vec());
            let _ = send.write_all(&bytes).await;
            let _ = send.finish();
        }
        Ok(())
    }
}

async fn handle_request(
    handler: &MailboxHandler,
    remote_id: &str,
    payload: &[u8],
) -> MailboxResponse {
    let req: MailboxRequest = match serde_json::from_slice(payload) {
        Ok(r) => r,
        Err(e) => return err(&format!("bad request: {e}")),
    };
    if matches!(req.op, Op::Sync | Op::SyncAck)
        && !handler.peers.iter().any(|peer| peer == remote_id)
    {
        return err("mailbox sync forbidden: sender is not a configured peer");
    }

    match req.op {
        Op::Ping => ok(),
        Op::Put => match StoredMessage::from_request(&req) {
            Ok(msg) => {
                if msg.msg.len() > MAX_PAYLOAD {
                    return err("payload too large");
                }
                // 防垃圾：校验工作量证明，不满足直接拒绝（不存储、不广播）。
                let nonce = req.nonce.unwrap_or(0);
                if !privchat_mailbox::verify_pow(&msg.to_peer_id, &msg.msg, nonce) {
                    return err("pow verification failed");
                }
                match handler.store.put(msg.clone(), now_ms()).await {
                    Ok(true) => {
                        // 本地持久化后立即响应；网格同步转后台，避免慢 peer
                        // 让客户端误判 PUT 超时并回滚发送棘轮。
                        spawn_broadcast(
                            handler.endpoint.clone(),
                            handler.peers.clone(),
                            MailboxRequest {
                                op: Op::Sync,
                                msg: Some(msg),
                                to: None,
                                for_peer: None,
                                id: None,
                                ids: None,
                                payload: None,
                                nonce: None,
                            },
                        );
                        ok()
                    }
                    Ok(false) => ok(), // msg_id 已存在，幂等
                    Err(e) => err(&format!("store failed: {e}")),
                }
            }
            Err(e) => err(&e.to_string()),
        },
        Op::Fetch => {
            let recipient = match req.for_peer {
                Some(r) => r,
                None => return err("fetch missing for"),
            };
            // 队列归属认证：只能拉取自己身份的队列，防他人「取回即删」式 DoS。
            if recipient != remote_id {
                return err("fetch forbidden: queue belongs to another identity");
            }
            match handler.store.fetch(&recipient).await {
                Ok(messages) => {
                    // 取回即删：本节点已删除，向 peer 广播删除意图，
                    // 网格同步清掉其他节点的副本。
                    if !messages.is_empty() {
                        let ids: Vec<String> = messages.iter().map(|m| m.msg_id.clone()).collect();
                        spawn_broadcast(
                            handler.endpoint.clone(),
                            handler.peers.clone(),
                            MailboxRequest {
                                op: Op::SyncAck,
                                to: Some(recipient),
                                ids: Some(ids),
                                for_peer: None,
                                id: None,
                                payload: None,
                                msg: None,
                                nonce: None,
                            },
                        );
                    }
                    MailboxResponse {
                        ok: true,
                        error: None,
                        messages: Some(messages),
                    }
                }
                Err(e) => err(&format!("fetch failed: {e}")),
            }
        }
        Op::Sync => {
            // 来自其他 mailbox 的同步广播：仅在**真正新增**时继续转发，
            // msg_id 主键幂等保证每条消息每节点至多处理一次 → 多跳链路可通、
            // 环路被截断（新增过一次后必为 0，不再广播）。
            match req.msg {
                Some(msg) => match handler.store.put(msg.clone(), now_ms()).await {
                    Ok(true) => {
                        spawn_broadcast(
                            handler.endpoint.clone(),
                            handler.peers.clone(),
                            MailboxRequest {
                                op: Op::Sync,
                                msg: Some(msg),
                                to: None,
                                for_peer: None,
                                id: None,
                                ids: None,
                                payload: None,
                                nonce: None,
                            },
                        );
                        ok()
                    }
                    Ok(false) => ok(), // 已存在，幂等，不再转发
                    Err(e) => err(&format!("store failed: {e}")),
                },
                None => err("sync missing msg"),
            }
        }
        Op::SyncAck => {
            let recipient = match req.to {
                Some(t) => t,
                None => return err("sync_ack missing to"),
            };
            let ids = req.ids.unwrap_or_default();
            match handler.store.ack(&recipient, &ids).await {
                Ok(deleted) => {
                    // 本节点确实删除了消息才继续转发；否则说明已删除过，
                    // 不再广播，切断环路。
                    if deleted > 0 {
                        spawn_broadcast(
                            handler.endpoint.clone(),
                            handler.peers.clone(),
                            MailboxRequest {
                                op: Op::SyncAck,
                                to: Some(recipient),
                                ids: Some(ids),
                                for_peer: None,
                                id: None,
                                payload: None,
                                msg: None,
                                nonce: None,
                            },
                        );
                    }
                    ok()
                }
                Err(e) => err(&format!("ack failed: {e}")),
            }
        }
    }
}

/// 本地操作完成后在后台向所有 peer mailbox 并发广播，避免阻塞客户端响应。
fn spawn_broadcast(endpoint: iroh::Endpoint, peers: Arc<Vec<String>>, req: MailboxRequest) {
    tokio::spawn(async move {
        let mut requests = tokio::task::JoinSet::new();
        let req = Arc::new(req);
        for peer in peers.iter().cloned() {
            let endpoint = endpoint.clone();
            let req = Arc::clone(&req);
            requests.spawn(async move {
                let result = privchat_mailbox::send_request(&endpoint, &peer, req.as_ref()).await;
                (peer, result)
            });
        }
        while let Some(result) = requests.join_next().await {
            match result {
                Ok((peer, Err(e))) => eprintln!("[mailbox] sync to {peer} failed: {e}"),
                Err(e) => eprintln!("[mailbox] sync task failed: {e}"),
                Ok((_, Ok(_))) => {}
            }
        }
    });
}

fn ok() -> MailboxResponse {
    MailboxResponse {
        ok: true,
        error: None,
        messages: None,
    }
}

fn err(e: &str) -> MailboxResponse {
    MailboxResponse {
        ok: false,
        error: Some(e.to_string()),
        messages: None,
    }
}

/// 读取 peer 配置：环境变量优先，其次 data_dir/mailboxes.json。
fn load_peers(data_dir: &std::path::Path) -> Vec<String> {
    if let Ok(v) = std::env::var("PRIVCHAT_MAILBOX_PEERS") {
        let peers: Vec<String> = v
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !peers.is_empty() {
            return peers;
        }
    }
    if let Ok(bytes) = std::fs::read(data_dir.join("mailboxes.json")) {
        if let Ok(peers) = serde_json::from_slice::<Vec<String>>(&bytes) {
            return peers;
        }
    }
    Vec::new()
}

/// 当前 UTC 毫秒时间戳。
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 读取消息 TTL 配置（秒）。环境变量 `PRIVCHAT_MAILBOX_TTL_SECS` 优先，
/// 其次 `data_dir/config.json` 的 `ttl_secs`，缺省 7 天；0 = 永不过期。
fn load_ttl(data_dir: &std::path::Path) -> u64 {
    if let Ok(v) = std::env::var("PRIVCHAT_MAILBOX_TTL_SECS") {
        if let Ok(secs) = v.trim().parse::<u64>() {
            return secs;
        }
    }
    if let Ok(bytes) = std::fs::read(data_dir.join("config.json")) {
        #[derive(serde::Deserialize)]
        struct Cfg {
            ttl_secs: Option<u64>,
        }
        if let Ok(cfg) = serde_json::from_slice::<Cfg>(&bytes) {
            if let Some(secs) = cfg.ttl_secs {
                return secs;
            }
        }
    }
    7 * 24 * 3600
}

/// 首次启动兜底：若 `config.json` / `mailboxes.json` 不存在则按当前生效值
/// 落盘生成，便于用户直接编辑。已存在的文件绝不覆盖。
fn ensure_config_files(data_dir: &std::path::Path, ttl_secs: u64, peers: &[String]) -> Result<()> {
    let cfg_path = data_dir.join("config.json");
    if !cfg_path.exists() {
        std::fs::write(
            &cfg_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "ttl_secs": ttl_secs,
            }))?,
        )?;
        eprintln!("[mailbox] wrote {}", cfg_path.display());
    }
    let peers_path = data_dir.join("mailboxes.json");
    if !peers_path.exists() {
        std::fs::write(&peers_path, serde_json::to_string_pretty(&peers)?)?;
        eprintln!("[mailbox] wrote {}", peers_path.display());
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let data_dir = std::env::var("PRIVCHAT_MAILBOX_DATA_DIR")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|e| e.parent().map(PathBuf::from))
        })
        .unwrap_or_else(|| std::env::temp_dir().join("privchat-mailbox"));
    std::fs::create_dir_all(&data_dir)?;

    let secret_path = data_dir.join("secret.key");
    let secret_key = if let Ok(bytes) = std::fs::read(&secret_path) {
        SecretKey::try_from(bytes.as_slice())?
    } else {
        let key = SecretKey::generate();
        std::fs::write(&secret_path, key.to_bytes())?;
        key
    };

    let ttl_secs = load_ttl(&data_dir);
    let store = MailboxStore::open(data_dir.join("mailbox.db"), ttl_secs)?;
    let peers = Arc::new(load_peers(&data_dir));

    // 首次启动兜底：缺失的配置文件落盘，方便用户后续编辑。
    ensure_config_files(&data_dir, ttl_secs, &peers)?;

    let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::N0)
        .secret_key(secret_key.clone())
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await?;

    let router = Router::builder(endpoint.clone())
        .accept(
            ALPN,
            MailboxHandler {
                store: store.clone(),
                endpoint: endpoint.clone(),
                peers: peers.clone(),
            },
        )
        .spawn();

    // 到期消息清理任务：按 TTL/4 频率（夹在 1 分钟 ~ 1 小时）周期清除。
    if ttl_secs > 0 {
        let purge_store = store.clone();
        let interval_secs = (ttl_secs / 4).clamp(60, 3600);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            ticker.tick().await; // 跳过首拍
            loop {
                ticker.tick().await;
                match purge_store.purge_expired(now_ms()).await {
                    Ok(n) if n > 0 => eprintln!("[mailbox] purged {n} expired messages"),
                    Ok(_) => {}
                    Err(e) => eprintln!("[mailbox] purge failed: {e}"),
                }
            }
        });
    }

    // 等待上线（联系上 relay），确保客户端可拨号。
    // 加限时：relay 不可达（离线/被墙/内网环境）时不阻塞启动。
    let _ = tokio::time::timeout(std::time::Duration::from_secs(8), endpoint.online()).await;

    let id = endpoint.id();
    println!("PrivChat Mailbox online");
    println!("  peer_id : {id}");
    println!("  store   : {}", store.path.display());
    println!("  ttl     : {} secs (0 = never expire)", ttl_secs);
    println!("  data_dir: {}", data_dir.display());
    if peers.is_empty() {
        println!("  peers   : (none) — 单节点模式");
    } else {
        println!("  peers   : {}", peers.join(", "));
    }

    // 保持进程常驻。
    std::future::pending::<()>().await;
    let _ = router.shutdown().await;
    Ok(())
}

#[cfg(test)]
mod tests;
