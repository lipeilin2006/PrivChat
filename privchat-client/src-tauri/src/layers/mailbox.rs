//! Layer 1.5 · Mailbox 客户端 (离线消息代理)
//!
//! 复用聊天身份的 iroh Endpoint（拨号时指定 mailbox ALPN），负责与 mailbox
//! 节点通信：把加密后的离线消息 PUT 上去、FETCH 本端待收密文、ACK 确认删除。
//!
//! mailbox 节点与聊天直连是两套独立的 ALPN，互不干扰。每个请求一条
//! QUIC bi-stream：写入 JSON 请求，读回 JSON 响应。
//!
//! 所有操作都以「身份 Endpoint」为参数：调用方传入某联系人的专属身份
//! Endpoint，令 mailbox 节点看到的 TLS 身份是**该联系人的专属身份**而非
//! 全局主身份，避免 mailbox 节点跨联系人关联本端。
//!
//! wire 协议（请求/响应/常量/PoW）统一由 `privchat-common` 提供，本层只
//! 保留客户端操作封装，避免与 mailbox 节点重复维护协议。

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use iroh::endpoint::presets;
use iroh::protocol::Router;
use iroh::{Endpoint, SecretKey};
use privchat_common::{
    find_pow, send_request, MailboxRequest, MailboxResponse, ALPN,
};

pub use privchat_common::StoredMessage;

/// Mailbox 客户端：无状态辅助函数，所有操作显式接收「身份 Endpoint」。
pub struct MailboxClient;

impl MailboxClient {
    /// 创建独立端点（仅测试/独立进程使用）：复用 data_dir/secret.key 身份。
    #[allow(dead_code)]
    pub async fn start(data_dir: PathBuf) -> Result<Endpoint> {
        let secret_path = data_dir.join("secret.key");
        let secret_key = if let Ok(bytes) = std::fs::read(&secret_path) {
            SecretKey::try_from(bytes.as_slice())?
        } else {
            let key = SecretKey::generate();
            std::fs::write(&secret_path, key.to_bytes())?;
            key
        };
        let endpoint = Endpoint::builder(presets::N0)
            .secret_key(secret_key)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await?;
        let _router = Router::builder(endpoint.clone()).spawn();
        // 等待上线（联系上 relay），确保可被拨号/可拨号。加限时防 relay
        // 不可达时挂死（与 transport::build_identity 一致）。
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(8),
            endpoint.online(),
        )
        .await;
        Ok(endpoint)
    }

    /// 向 mailbox 节点发一个请求，返回响应。用传入的身份 Endpoint 拨号，
    /// 让 mailbox 节点看到的 TLS 身份为该联系人专属身份。
    async fn call(endpoint: &Endpoint, mailbox: &str, req: &MailboxRequest) -> Result<MailboxResponse> {
        send_request(endpoint, mailbox, req).await
    }

    /// 上传一条密文邮件。先计算 PoW nonce（防垃圾），随请求提交。
    /// 发送方由拨号 Endpoint 的 TLS 身份决定，无需显式传 from。
    pub async fn put(
        endpoint: &Endpoint,
        mailbox: &str,
        to: &str,
        id: &str,
        payload: Vec<u8>,
    ) -> Result<()> {
        // 在独立线程算 PoW，避免阻塞 IO 任务。
        let to_owned = to.to_string();
        let payload_owned = payload.clone();
        let nonce = tokio::task::spawn_blocking(move || find_pow(&to_owned, &payload_owned))
            .await
            .map_err(|e| anyhow!("pow task failed: {e}"))?;
        let resp = Self::call(endpoint, mailbox, &MailboxRequest::put(to, id, payload, nonce)).await?;
        if resp.ok {
            Ok(())
        } else {
            Err(anyhow!("mailbox put failed: {}", resp.error.unwrap_or_default()))
        }
    }

    /// 拉取本端全部待收密文（节点侧「取回即删」并广播删除同步）。
    pub async fn fetch(endpoint: &Endpoint, mailbox: &str, me: &str) -> Result<Vec<StoredMessage>> {
        let resp = Self::call(endpoint, mailbox, &MailboxRequest::fetch(me)).await?;
        if resp.ok {
            Ok(resp.messages.unwrap_or_default())
        } else {
            Err(anyhow!("mailbox fetch failed: {}", resp.error.unwrap_or_default()))
        }
    }
}
