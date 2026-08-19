//! 命令行测试客户端：验证 mailbox 的 put/fetch 及节点间同步。
//!
//! 用法：
//!   cargo run --example client -- <mailbox_peer_id> put  <to> <id> <text>
//!   cargo run --example client -- <mailbox_peer_id> fetch <recipient>

use anyhow::{anyhow, Result};
use privchat_mailbox::{MailboxRequest, find_pow, make_endpoint, send_request};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        return Err(anyhow!(
            "usage: client <mailbox_peer_id> put <to> <id> <text> | fetch <recipient>"
        ));
    }
    let peer_id = &args[1];
    let op = &args[2];

    let endpoint = make_endpoint(iroh::SecretKey::generate()).await?;
    eprintln!("[client] connecting to {peer_id} op={op}");

    let req = match op.as_str() {
        "put" => {
            if args.len() < 6 {
                return Err(anyhow!("put needs <to> <id> <text>"));
            }
            let payload = args[5].as_bytes().to_vec();
            // 计算工作量证明（防垃圾）。
            let nonce = find_pow(&args[3], &payload);
            eprintln!("[client] pow done: nonce={nonce}");
            MailboxRequest::put(&args[3], &args[4], payload, nonce)
        }
        "fetch" => {
            if args.len() < 4 {
                return Err(anyhow!("fetch needs <recipient>"));
            }
            MailboxRequest::fetch(&args[3])
        }
        other => return Err(anyhow!("unknown op: {other}")),
    };

    match send_request(&endpoint, peer_id, &req).await {
        Ok(resp) => {
            println!("{op}  => ok={} error={:?}", resp.ok, resp.error);
            if let Some(msgs) = resp.messages {
                println!("  {} message(s):", msgs.len());
                for m in msgs {
                    println!(
                        "    msg_id={} to={} msg_len={}",
                        m.msg_id,
                        m.to_peer_id,
                        m.msg.len()
                    );
                }
            }
        }
        Err(e) => println!("{op}  => ERROR: {e:#}"),
    }
    Ok(())
}