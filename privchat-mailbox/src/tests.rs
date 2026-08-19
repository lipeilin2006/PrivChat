use super::*;

fn temp_db(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("privchat-mailbox-db-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("mailbox.db")
}

#[tokio::test]
async fn put_fetch_roundtrip() {
    let path = temp_db("roundtrip");
    let store = MailboxStore::open(path.clone(), 0).expect("open");
    let msg = StoredMessage {
        msg_id: "m1".into(),
        to_peer_id: "peer-a".into(),
        msg: b"ciphertext".to_vec(),
    };
    assert!(store.put(msg.clone(), 100).await.expect("put"));
    // msg_id 幂等：重复 put 不再新增。
    assert!(!store.put(msg.clone(), 100).await.expect("dup put"));

    // fetch 取回即删：返回队列（按 msg_id 排序）。
    let fetched = store.fetch("peer-a").await.expect("fetch");
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].msg_id, "m1");
    assert_eq!(fetched[0].msg, b"ciphertext");
    // 取回后立即删除：再 fetch 为空。
    assert!(store.fetch("peer-a").await.expect("fetch after").is_empty());
    assert!(store.fetch("peer-b").await.expect("fetch b").is_empty());

    // 重开库（模拟重启）持久化已删除状态。
    drop(store);
    let reopened = MailboxStore::open(path.clone(), 0).expect("reopen");
    assert!(reopened.fetch("peer-a").await.expect("fetch again").is_empty());
    // 重新入库后 fetch 再次取回即删。
    reopened.put(msg.clone(), 100).await.expect("put again");
    assert_eq!(reopened.fetch("peer-a").await.expect("fetch again 2").len(), 1);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// 队列归属认证：fetch 只能拉取自己身份的队列，他人拉取被拒（防
/// 「取回即删」式 DoS）；本人正常拉取。
#[tokio::test]
async fn fetch_forbidden_for_other_identity() {
    let dir = std::env::temp_dir().join(format!(
        "privchat-mailbox-authz-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let store = MailboxStore::open(dir.join("mb.db"), 0).expect("open");
    let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::N0)
        .secret_key(iroh::SecretKey::generate())
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .expect("bind");
    let handler = MailboxHandler {
        store,
        endpoint,
        peers: Arc::new(Vec::new()),
    };

    let victim = "victim-peer".to_string();
    handler
        .store
        .put(
            StoredMessage {
                msg_id: "m1".into(),
                to_peer_id: victim.clone(),
                msg: vec![1, 2, 3],
            },
            100,
        )
        .await
        .expect("seed queue");

    // 其他身份 fetch → 拒绝，队列不动。
    let req = MailboxRequest::fetch(&victim);
    let resp = handle_request(
        &handler,
        "attacker-peer",
        &serde_json::to_vec(&req).expect("ser"),
    )
    .await;
    assert!(!resp.ok, "attacker fetch must fail");
    assert!(
        resp.error.as_deref().unwrap_or("").contains("forbidden"),
        "error: {:?}",
        resp.error
    );
    // 队列仍在（未被取回即删）：非破坏性计数检查。
    {
        let conn = handler.store.conn.lock().await;
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM messages WHERE to_peer_id = ?1",
                params![victim],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(count, 1, "queue must be intact after forbidden fetch");
    }

    // 本人 fetch → 成功并取回即删。
    let req = MailboxRequest::fetch(&victim);
    let resp = handle_request(
        &handler,
        &victim,
        &serde_json::to_vec(&req).expect("ser"),
    )
    .await;
    assert!(resp.ok, "owner fetch must succeed: {:?}", resp.error);
    assert_eq!(resp.messages.expect("messages").len(), 1);
    assert!(handler.store.fetch(&victim).await.expect("drained").is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// 链式拓扑下的转发不变量：`put` 只有「真正新增」返回 true（触发转发），
/// 重复插入返回 false（切断环路）。沿 mb0→mb1→mb2 依次放入同一条消息，
/// 必须恰好各节点新增一次；环回（mb2 再发回 mb0）必须返回 false。
#[tokio::test]
async fn chain_forwarding_stops_on_duplicate() {
    let dir = std::env::temp_dir().join(format!(
        "privchat-mailbox-chain-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mb0 = MailboxStore::open(dir.join("mb0.db"), 0).expect("mb0");
    let mb1 = MailboxStore::open(dir.join("mb1.db"), 0).expect("mb1");
    let mb2 = MailboxStore::open(dir.join("mb2.db"), 0).expect("mb2");
    let msg = StoredMessage {
        msg_id: "chain-msg".into(),
        to_peer_id: "recipient-x".into(),
        msg: b"ciphertext".to_vec(),
    };

    // client0 -> mb0 新增（转发）-> mb1 新增（转发）-> mb2 新增。
    assert!(mb0.put(msg.clone(), 100).await.expect("mb0 put"));
    assert!(mb1.put(msg.clone(), 100).await.expect("mb1 put"));
    assert!(mb2.put(msg.clone(), 100).await.expect("mb2 put"));
    // 环回 mb2 -> mb0：已存在，不得再新增（否则会无限循环）。
    assert!(!mb0.put(msg.clone(), 100).await.expect("mb0 dup"));
    // 环回 mb2 -> mb1：同样截断。
    assert!(!mb1.put(msg.clone(), 100).await.expect("mb1 dup"));

    // 每个节点都持有一份，client1 从 mb2 fetch 能拿到。
    assert_eq!(mb2.fetch("recipient-x").await.expect("fetch").len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 网格删除（sync_ack）转发不变量：首次删除返回行数 1（触发转发），
/// 重复删除返回 0（不再转发），切断环路。
#[tokio::test]
async fn sync_ack_returns_deleted_count_for_loop_cut() {
    let dir = std::env::temp_dir().join(format!(
        "privchat-mailbox-ackcount-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let store = MailboxStore::open(dir.join("ack.db"), 0).expect("open");
    store
        .put(StoredMessage {
            msg_id: "m".into(),
            to_peer_id: "a".into(),
            msg: vec![],
        }, 100)
        .await
        .expect("put");
    // 首次 ack 删除 1 行 → 可继续转发。
    assert_eq!(store.ack("a", &["m".into()]).await.expect("ack"), 1);
    // 再次 ack 删除 0 行 → 不再转发，环路截断。
    assert_eq!(store.ack("a", &["m".into()]).await.expect("dup ack"), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 取回即删 + 排序：fetch 按 msg_id（发送方 (gen,n) 派生前缀）升序返回，
/// 返回后立即从本节点删除。
#[tokio::test]
async fn fetch_returns_sorted_and_deletes() {
    let path = temp_db("sorted");
    let store = MailboxStore::open(path.clone(), 0).expect("open");
    for id in ["z", "a", "m"] {
        store
            .put(
                StoredMessage {
                    msg_id: id.into(),
                    to_peer_id: "q".into(),
                    msg: vec![],
                },
                100,
            )
            .await
            .expect("put");
    }
    let fetched = store.fetch("q").await.expect("fetch");
    let ids: Vec<&str> = fetched.iter().map(|m| m.msg_id.as_str()).collect();
    assert_eq!(ids, ["a", "m", "z"]);
    // 已全部删除。
    assert!(store.fetch("q").await.expect("fetch again").is_empty());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// TTL：到期消息被清除，未过期保留；ttl=0 关闭。
#[tokio::test]
async fn ttl_purges_expired_rows() {
    let path = temp_db("ttl");
    let store = MailboxStore::open(path.clone(), 1).expect("open"); // 1 秒 TTL
    let now = now_ms();
    store
        .put(
            StoredMessage {
                msg_id: "old".into(),
                to_peer_id: "q".into(),
                msg: vec![],
            },
            now.saturating_sub(5_000),
        )
        .await
        .expect("put old");
    store
        .put(
            StoredMessage {
                msg_id: "fresh".into(),
                to_peer_id: "q".into(),
                msg: vec![],
            },
            now,
        )
        .await
        .expect("put fresh");

    let purged = store.purge_expired(now).await.expect("purge");
    assert_eq!(purged, 1);
    let fetched = store.fetch("q").await.expect("fetch");
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].msg_id, "fresh");

    // ttl=0：永不过期。
    let store2 = MailboxStore::open(path.clone(), 0).expect("open off");
    store2
        .put(
            StoredMessage {
                msg_id: "forever".into(),
                to_peer_id: "q".into(),
                msg: vec![],
            },
            now.saturating_sub(9_999_999),
        )
        .await
        .expect("put forever");
    assert_eq!(store2.purge_expired(now).await.expect("purge off"), 0);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// 存量库迁移：旧表无 time 列 → 补列，历史行 time=0（视为立即过期，
/// 随首次 TTL 清理删除）；新消息正常留存。
#[tokio::test]
async fn legacy_db_migrates_time_column() {
    let path = temp_db("legacy");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE messages (
             msg_id      TEXT PRIMARY KEY,
             to_peer_id  TEXT NOT NULL,
             msg         BLOB NOT NULL
         );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages (msg_id, to_peer_id, msg) VALUES ('legacy', 'q', x'00')",
        [],
    )
    .unwrap();
    drop(conn);

    let store = MailboxStore::open(path.clone(), 1).expect("open legacy");
    store
        .put(
            StoredMessage {
                msg_id: "new".into(),
                to_peer_id: "q".into(),
                msg: vec![],
            },
            now_ms(),
        )
        .await
        .expect("put new");
    // legacy(time=0) 过期清除；new 保留。
    assert_eq!(store.purge_expired(now_ms()).await.expect("purge"), 1);
    let fetched = store.fetch("q").await.expect("fetch");
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].msg_id, "new");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
