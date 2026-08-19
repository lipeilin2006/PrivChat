# PrivChat

端到端加密点对点聊天。基于 iroh QUIC 实现直连与离线投递，双棘轮（Double Ratchet）提供前向保密，SQLCipher 加密全部本地数据。

## 仓库结构

```
privchat-client/       Tauri 客户端（Android / Windows / macOS）
  src/                 Vue 3 + Quasar 前端
  src-tauri/           Rust 后端（L2 应用层 / L2.5 加密 / L3 传输 / L4 封装）
privchat-common/       共享 crate：mailbox wire 协议与 PoW
privchat-mailbox/      离线消息缓存节点（可网格组网同步）
```

## 安全模型

- **无主身份**：每个联系人使用独立 SecretKey + 独立 iroh Endpoint，跨联系人不关联。
- **双棘轮加密**：每条消息独立密钥，发送失败不推进链；接收方跳过窗口可自愈乱序。
- **本地加密存储**：SQLCipher 全库加密，密码解锁保险箱（vault）。
- **离线投递**：mailbox 节点缓存密文（可组网同步，取回即删），不接触明文。

## 开发

### 桌面端（Windows）

```powershell
cd privchat-client
npm install
npm run tauri build -- --no-bundle
```

### Android（arm64）

需要 NDK r28c 与 Android SDK，见 `wsl_apk_build_arm64.sh` 的构建流程（WSL 内）。

### mailbox 节点

```bash
cd privchat-mailbox
cargo run --release
```

首次启动自动生成 `config.json` / `mailboxes.json` / `secret.key`（可配置 `PRIVCHAT_MAILBOX_DATA_DIR` 等环境变量）。

## 测试

```bash
cargo test                      # 根工作区（client lib + mailbox）
```

## License

[MIT](LICENSE)