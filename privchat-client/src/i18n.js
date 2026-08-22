import { computed, ref } from "vue";

const LOCALE_KEY = "privchat_locale";

function detectLocale() {
  const saved = localStorage.getItem(LOCALE_KEY);
  if (saved === "zh-CN" || saved === "en") return saved;
  const systemLocale = navigator.language || navigator.languages?.[0] || "en";
  return systemLocale.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}

const locale = ref(detectLocale());

const messages = {
  "zh-CN": {
    vault: {
      createTitle: "创建密码",
      unlockTitle: "输入密码",
      createHint: "首次启动：请选择一个密码来加密所有本地数据。密码遗失后无法找回。",
      unlockHint: "所有本地数据均已加密。请输入密码解锁。",
      password: "密码",
      unlock: "解锁",
      locking: "正在锁定，请稍候…",
    },
    common: {
      cancel: "取消",
      save: "保存",
      delete: "删除",
      add: "添加",
      copy: "复制",
      refresh: "刷新",
      close: "关闭",
      failed: "失败",
      copied: "已复制",
    },
    contacts: {
      settings: "设置",
      messages: "消息",
      add: "添加联系人",
      search: "搜索联系人或消息",
      noMatches: "没有匹配的聊天",
      empty: "还没有会话",
      open: "打开聊天",
      delete: "删除联系人",
      deleteTitle: "删除联系人？",
      deleteMessage: "确定移除“{name}”及其聊天记录吗？",
      searchMessages: "搜索消息",
      searchMessagesPlaceholder: "输入消息内容",
      fromDate: "开始日期",
      toDate: "结束日期",
      noSearchResults: "没有找到消息",
    },
    chat: {
      message: "消息",
      resend: "重试发送",
      copy: "复制消息",
      copied: "消息已复制",
      copyFailed: "复制失败：{error}",
      today: "今天",
      yesterday: "昨天",
      encrypted: "已加密 · 点对点",
      resending: "重发中",
      cancelled: "已取消",
    },
    info: {
      title: "聊天信息",
      cancel: "取消",
      save: "保存",
      rename: "重命名联系人",
      online: "在线",
      offline: "离线",
      nodeId: "节点 ID",
      copy: "复制",
      delete: "删除联系人",
      deleteTitle: "删除联系人？",
      deleteMessage: "这将移除“{name}”并清除本地聊天记录。消息无法恢复。",
      empty: "选择一个会话查看详情",
    },
    settings: {
      title: "设置",
      mailbox: "Mailbox",
      mailboxHint: "管理离线消息中继节点",
      theme: "主题",
      themeHint: "跟随系统或手动选择",
      language: "语言",
      languageHint: "选择界面语言",
      system: "跟随系统",
      dark: "深色",
      light: "浅色",
      chinese: "中文",
      english: "English",
      lock: "立即锁定",
      lockHint: "清除当前会话并返回密码界面",
      never: "永不",
      oneMinute: "1 分钟",
      fiveMinutes: "5 分钟",
      autoLock: "自动锁定",
      autoLockHint: "无操作后自动锁定保险箱",
      changePassword: "修改密码",
      changePasswordHint: "验证旧密码并重新加密本地数据",
      oldPassword: "旧密码",
      newPassword: "新密码",
      confirmPassword: "确认新密码",
      passwordWarning: "密码无法恢复，请务必妥善保存。",
      passwordMismatch: "密码为空或两次输入的新密码不一致",
      passwordChanged: "密码已修改",
      autoLockSaveFailed: "自动锁定设置保存失败",
    },
    mailbox: {
      title: "Mailbox",
      tip: "离线消息中继。发送消息时，消息会先加密，然后上传到配置的 mailbox 节点。你可以选择写入副本数量以提高冗余，收件人上线后即可收到。",
      copies: "每条消息的副本数",
      all: "全部已配置节点",
      placeholder: "粘贴 mailbox 节点 ID（64 位十六进制）",
      noMailboxes: "尚未配置 mailbox 节点。",
      nodeId: "节点 ID 必须是 64 位十六进制字符",
      loaded: "Mailbox 列表加载失败：{error}",
      countLoaded: "投放数量加载失败：{error}",
      countSaved: "投放数量已保存",
      added: "Mailbox 已添加",
      removed: "Mailbox 已移除",
      addFailed: "添加失败：{error}",
      removeFailed: "移除失败：{error}",
      saveFailed: "保存失败：{error}",
      removeTitle: "删除 Mailbox？",
      removeMessage: "删除后将停止向该节点投放消息。",
      online: "在线",
      offline: "离线",
    },
    id: {
      title: "我的 ID",
      creating: "正在创建 ID…",
      scanToAdd: "扫描二维码添加我",
      copy: "复制 ID",
      refresh: "刷新 ID",
      hint: "每次刷新都会生成新的 ID。将它分享给一个人即可建立连接。",
      addTitle: "添加联系人",
      displayName: "显示名称",
      peerId: "对方 ID",
      peerPlaceholder: "粘贴对方的 ID…",
      required: "对方 ID 不能为空",
      scan: "扫描二维码",
      cameraPermission: "需要相机权限才能扫描二维码",
      scanUnavailable: "无法使用相机扫描：{error}",
      pasteTicket: "请粘贴连接凭证",
      added: "联系人已添加",
      connectFailed: "连接失败：{error}",
      processing: "正在添加联系人，请稍候…",
      idCreated: "ID 创建失败：{error}",
      copied: "ID 已复制",
      copyFailed: "复制失败：{error}",
    },
    notify: {
      deleted: "联系人已删除",
      deleteFailed: "删除失败：{error}",
      renamed: "名称已更新",
      renameFailed: "重命名失败：{error}",
      sendFailed: "发送失败：{error}",
      operationFailed: "操作失败，请稍后重试",
      diagnostics: "导出诊断信息",
      diagnosticsHint: "只包含版本、平台和状态，不包含私密数据",
    },
  },
  en: {
    vault: { createTitle: "Create your password", unlockTitle: "Enter password", createHint: "First launch: choose a password to encrypt all local data. It cannot be recovered if forgotten.", unlockHint: "All local data is encrypted. Enter your password to unlock.", password: "Password", unlock: "Unlock", locking: "Locking, please wait…" },
    common: { cancel: "Cancel", save: "Save", delete: "Delete", add: "Add", copy: "Copy", refresh: "Refresh", close: "Close", failed: "Failed", copied: "Copied" },
    contacts: { settings: "Settings", messages: "Messages", add: "Add contact", search: "Search contacts or messages", noMatches: "No matches", empty: "No conversations yet", open: "Open chat", delete: "Delete contact", deleteTitle: "Delete contact?", deleteMessage: 'Remove "{name}" and its chat history?', searchMessages: "Search messages", searchMessagesPlaceholder: "Enter message text", fromDate: "From", toDate: "To", noSearchResults: "No messages found" },
    chat: { message: "Message", resend: "resend", copied: "Message copied", copyFailed: "Copy failed: {error}", today: "Today", yesterday: "Yesterday", encrypted: "Encrypted · Peer to peer", resending: "Resending", cancelled: "Cancelled" },
    info: { title: "Chat info", cancel: "Cancel", save: "Save", rename: "Rename contact", online: "Online", offline: "Offline", nodeId: "Node ID", copy: "Copy", delete: "Delete contact", deleteTitle: "Delete contact?", deleteMessage: 'This removes "{name}" and clears the local chat history. Messages can\'t be recovered.', empty: "Select a conversation to see details" },
    settings: { title: "Settings", mailbox: "Mailbox", mailboxHint: "Manage offline message relays", theme: "Theme", themeHint: "Follow system or choose manually", language: "Language", languageHint: "Choose interface language", system: "System", dark: "Dark", light: "Light", chinese: "中文", english: "English", lock: "Lock now", lockHint: "Clear session data and return to the password screen", never: "Never", oneMinute: "1 minute", fiveMinutes: "5 minutes", autoLock: "Auto-lock", autoLockHint: "Lock the vault after inactivity", changePassword: "Change password", changePasswordHint: "Verify the old password and re-encrypt local data", oldPassword: "Old password", newPassword: "New password", confirmPassword: "Confirm new password", passwordWarning: "The password cannot be recovered. Store it safely.", passwordMismatch: "Password is empty or the new passwords do not match", passwordChanged: "Password changed", autoLockSaveFailed: "Failed to save auto-lock setting" },
    mailbox: { title: "Mailbox", tip: "Offline message relays. Messages are encrypted before upload. Choose how many copies are written for redundancy, then they are delivered when the recipient comes online.", copies: "Copies per message", all: "All configured", placeholder: "Paste a mailbox Node ID (64 hex chars)", noMailboxes: "No mailboxes configured.", nodeId: "Node ID must be 64 hex chars", loaded: "Failed to load mailbox list: {error}", countLoaded: "Failed to load write count: {error}", countSaved: "Write count saved", added: "Mailbox added", removed: "Mailbox removed", addFailed: "Add failed: {error}", removeFailed: "Remove failed: {error}", saveFailed: "Save failed: {error}" },
    id: { title: "My ID", creating: "Creating your ID…", scanToAdd: "Scan to add me", copy: "Copy ID", refresh: "Refresh ID", hint: "Each refresh creates a new ID. Share it with one person to connect.", addTitle: "Add contact", displayName: "Display name", peerId: "Peer ID", peerPlaceholder: "Paste the peer's ID…", required: "Peer ID is required", scan: "Scan QR code", cameraPermission: "Camera permission required to scan a QR code", scanUnavailable: "Camera scan unavailable: {error}", pasteTicket: "Please paste a connection ticket", added: "Contact added", connectFailed: "Connect failed: {error}", processing: "Adding contact, please wait…", idCreated: "Failed to create ID: {error}", copied: "ID copied", copyFailed: "Copy failed: {error}" },
    notify: { deleted: "Contact deleted", deleteFailed: "Delete failed: {error}", renamed: "Name updated", renameFailed: "Rename failed: {error}", sendFailed: "Send failed: {error}", operationFailed: "Operation failed. Please try again later." },
  },
};

function translate(key, params = {}) {
  const value = key.split(".").reduce((node, part) => node?.[part], messages[locale.value]) ?? key;
  return String(value).replace(/\{(\w+)\}/g, (_, name) => params[name] ?? `{${name}}`);
}

export function useI18n() {
  const t = (key, params) => translate(key, params);
  const setLocale = (value) => {
    locale.value = value;
    localStorage.setItem(LOCALE_KEY, value);
  };
  return { locale: computed(() => locale.value), t, setLocale };
}
