<script setup>
import { computed, onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useQuasar } from "quasar";
import ContactsPanel from "./components/ContactsPanel.vue";
import ChatWindow from "./components/ChatWindow.vue";
import ChatInfoPanel from "./components/ChatInfoPanel.vue";
import Settings from "./components/Settings.vue";
import MailboxSettings from "./components/MailboxSettings.vue";
import MyId from "./components/MyId.vue";

const $q = useQuasar();

const conversations = ref([]);
const activeId = ref(null);
const ready = ref(false);
// 保险箱解锁门：unlocked=false 时只显示密码界面。
const unlocked = ref(false);
const vaultInitialized = ref(false);
const vaultBusy = ref(false);
const vaultPassword = ref("");
const vaultError = ref("");
const vaultMode = computed(() =>
  vaultInitialized.value ? "unlock" : "create"
);
// 移动端单屏视图：list | chat | settings | add | mailbox | info
const view = ref("list");
const showInfo = ref(false); // desktop: right-hand chat info panel

const messagesByConv = reactive({});
// peer_id → 本端为该联系人使用的专属身份（判定消息归属 me/peer）。
const localIdByPeer = reactive({});

const isMobile = computed(() => $q.screen.lt.md);

// 桌面端 Settings / My ID / Mailbox 浮层的开关。
const overlayOpen = computed({
  get: () =>
    !isMobile.value &&
    (view.value === "settings" ||
      view.value === "add" ||
      view.value === "mailbox"),
  set: (v) => {
    if (!v) closeOverlay();
  },
});

const activeConv = computed(
  () => conversations.value.find((c) => c.id === activeId.value) ?? null
);

function ensureConversation(peerId, name, localId) {
  let conv = conversations.value.find((c) => c.nodeId === peerId);
  if (!conv) {
    conv = reactive({
      id: `conv-${peerId}`,
      name: name || peerId.slice(0, 10),
      nodeId: peerId,
      localId: localId || "",
      lastMessage: "",
      timestamp: 0,
      unread: 0,
      online: true,
      lastStatus: "",
    });
    conversations.value.push(conv);
  } else if (localId) {
    conv.localId = localId;
  }
  return conv;
}

function getMessages(peerId) {
  if (!messagesByConv[peerId]) messagesByConv[peerId] = [];
  return messagesByConv[peerId];
}

async function selectConversation(id) {
  activeId.value = id;
  const conv = conversations.value.find((c) => c.id === id);
  if (conv) conv.unread = 0;
  if (conv && !messagesByConv[conv.nodeId]) {
    try {
      const hist = await invoke("get_history", { peerId: conv.nodeId });
      messagesByConv[conv.nodeId] = (hist || []).map((h) => ({
        id: h.id ? `h-${h.id}` : `h-${h.time}-${h.from === conv.localId ? "me" : "peer"}`,
        from: h.from === conv.localId ? "me" : "peer",
        text: h.text,
        ts: h.time,
        status: h.from === conv.localId ? "sent" : "received",
      }));
    } catch (e) {
      console.error("load history failed", e);
    }
  }
  if (isMobile.value) view.value = "chat";
}

function goToList() {
  view.value = "list";
  activeId.value = null;
}

function openSettings() {
  view.value = "settings";
}

function openAdd() {
  view.value = "add";
}

function openMailbox() {
  view.value = "mailbox";
}

function closeOverlay() {
  view.value = activeId.value ? "chat" : "list";
}

function openInfo() {
  if (isMobile.value) {
    view.value = "info";
  } else {
    showInfo.value = !showInfo.value;
  }
}

function closeInfo() {
  if (isMobile.value) view.value = "chat";
}

async function onAdded(peerId) {
  // 添加联系人成功：把新联系人同步进内存列表，避免重启才显示。
  try {
    const contacts = await invoke("list_contacts");
    const added = (contacts || []).find((c) => c.peer_id === peerId);
    localIdByPeer[peerId] = added?.local_id || "";
    const conv = ensureConversation(
      peerId,
      added?.name || peerId.slice(0, 10),
      added?.local_id || ""
    );
    conv.lastMessage = conv.lastMessage || "Peer connected";
    if (!conv.timestamp) conv.timestamp = Date.now();
    activeId.value = conv.id;
    if (isMobile.value) view.value = "chat";
    else closeOverlay();
  } catch (e) {
    console.error("refresh contacts after add failed", e);
  }
}

async function deleteContact() {
  if (!activeConv.value) return;
  await deleteContactById(activeConv.value.nodeId);
}

async function deleteContactById(peerId) {
  try {
    await invoke("delete_contact", { peerId });
    // 清理内存态：会话列表、消息、选择与详情面板。
    delete messagesByConv[peerId];
    conversations.value = conversations.value.filter(
      (c) => c.nodeId !== peerId
    );
    if (activeId.value === `conv-${peerId}`) {
      activeId.value = null;
      showInfo.value = false;
      if (isMobile.value) view.value = "list";
    }
    $q.notify({ type: "positive", message: "Contact deleted", position: "bottom" });
  } catch (e) {
    $q.notify({ type: "negative", message: `Delete failed: ${e}`, position: "bottom" });
  }
}

// 重命名联系人：调用后端持久化，并同步内存会话名。
async function renameContact(peerId, name) {
  try {
    const updated = await invoke("rename_contact", { peerId, name });
    const conv = conversations.value.find((c) => c.nodeId === peerId);
    if (conv && updated) conv.name = updated.name;
    if (updated) localIdByPeer[peerId] = updated.local_id || localIdByPeer[peerId];
    $q.notify({ type: "positive", message: "Name updated", position: "bottom" });
    return true;
  } catch (e) {
    $q.notify({ type: "negative", message: `Rename failed: ${e}`, position: "bottom" });
    return false;
  }
}

async function doSend(msg, peerId) {
  try {
    const result = await invoke("send_message", { peerId, text: msg.text });
    msg.id = result.id;
    msg.ts = result.time;
    msg.status = "sent";
    const conv = conversations.value.find((c) => c.nodeId === peerId);
    if (conv) conv.lastStatus = "sent";
    return true;
  } catch (e) {
    msg.status = "failed";
    const conv = conversations.value.find((c) => c.nodeId === peerId);
    if (conv) conv.lastStatus = "failed";
    $q.notify({ type: "negative", message: `Send failed: ${e}`, position: "bottom" });
    return false;
  }
}

function sendMessage(text) {
  if (!text.trim() || !activeConv.value) return;
  const peerId = activeConv.value.nodeId;
  // 乐观更新：先立即上屏为「发送中」，成功/失败再更新状态。
  const tempId = `m-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
  const msg = reactive({
    id: tempId,
    from: "me",
    text,
    ts: Date.now(),
    status: "sending",
  });
  getMessages(peerId).push(msg);
  activeConv.value.lastMessage = text;
  activeConv.value.timestamp = Date.now();
  activeConv.value.lastStatus = "sending";
  doSend(msg, peerId);
}

// 失败消息点击重发：把该条消息置底（与接收方送达顺序一致）并置回
// 「发送中」后重新投递。
function resendMessage(msg) {
  if (!activeConv.value || !msg) return;
  const msgs = getMessages(activeConv.value.nodeId);
  const idx = msgs.indexOf(msg);
  if (idx !== -1) {
    msgs.splice(idx, 1);
    msgs.push(msg);
  }
  msg.status = "sending";
  activeConv.value.lastStatus = "sending";
  activeConv.value.lastMessage = msg.text;
  activeConv.value.timestamp = Date.now();
  doSend(msg, activeConv.value.nodeId);
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    $q.notify({ type: "positive", message: "Copied to clipboard", position: "bottom" });
  } catch (e) {
    $q.notify({ type: "negative", message: `Copy failed: ${e}`, position: "bottom" });
  }
}

// 保险箱：首次创建密码 / 之后输入密码解锁。
async function submitVault() {
  if (!vaultPassword.value || vaultBusy.value) return;
  vaultBusy.value = true;
  vaultError.value = "";
  try {
    const st = await invoke(
      vaultInitialized.value ? "unlock_vault" : "create_vault",
      { password: vaultPassword.value }
    );
    if (st && st.unlocked) {
      unlocked.value = true;
      await loadLocalState();
      await listenForMessages();
    }
  } catch (e) {
    vaultError.value = String(e);
  } finally {
    vaultBusy.value = false;
  }
}

function closeVaultInput() {
  // 密码框按 Enter 或点按钮提交；这里仅为移除焦点。
}

watch(
  () => $q.screen.lt.md,
  (mobile) => {
    if (!mobile) view.value = "list";
  }
);

// 主题：跟随系统或手动（localStorage 持久化，设置面板中切换）。
const THEME_KEY = "privchat_theme";
const savedTheme = JSON.parse(
  localStorage.getItem(THEME_KEY) ?? JSON.stringify("auto")
);
$q.dark.set(savedTheme);

function syncThemeClass() {
  document.documentElement.classList.toggle("light", !$q.dark.isActive);
}

watch(
  () => $q.dark.isActive,
  (dark) => {
    document.documentElement.classList.toggle("light", !dark);
  }
);

onMounted(async () => {
  syncThemeClass();

  // 保险箱门：先查状态，未解锁时不加载任何本地数据。
  try {
    const st = await invoke("vault_status");
    vaultInitialized.value = !!st?.initialized;
    if (st?.unlocked) {
      unlocked.value = true;
      await loadLocalState();
      await listenForMessages();
    }
  } catch (e) {
    console.error("vault_status failed", e);
  }
  ready.value = true;
});

// 解锁成功后加载本地联系人、历史与入站消息监听。
async function loadLocalState() {
  try {
    const contacts = await invoke("list_contacts");
    for (const c of contacts || []) {
      localIdByPeer[c.peer_id] = c.local_id;
      const conv = ensureConversation(c.peer_id, c.name, c.local_id);
      const hist = await invoke("get_history", { peerId: c.peer_id });
      conv.lastMessage = hist?.length
        ? hist[hist.length - 1].text
        : "";
      conv.timestamp = hist?.length ? hist[hist.length - 1].time : 0;
      conv.lastStatus = hist?.length
        ? (hist[hist.length - 1].from === c.local_id ? "sent" : "received")
        : "";
      if (hist?.length) {
        messagesByConv[c.peer_id] = hist.map((h) => ({
          id: h.id ? `h-${h.id}` : `h-${h.time}-${h.from === c.local_id ? "me" : "peer"}`,
          from: h.from === c.local_id ? "me" : "peer",
          text: h.text,
          ts: h.time,
          status: h.from === c.local_id ? "sent" : "received",
        }));
      }
    }
  } catch (e) {
    console.error("load contacts failed", e);
  }
}

let unlistenMessages = null;

async function listenForMessages() {
  if (unlistenMessages) return;
  unlistenMessages = await listen("chat://message", (event) => {
    const { from, text, time } = event.payload;
    const conv = ensureConversation(from, undefined, localIdByPeer[from]);
    conv.lastMessage = text;
    conv.timestamp = time;
    conv.lastStatus = "received";
    const msg = {
      id: `m-${time}-${Math.random().toString(36).slice(2, 7)}`,
      from: "peer",
      text,
      ts: time,
      status: "received",
    };
    getMessages(from).push(msg);
    if (activeId.value !== conv.id) {
      conv.unread += 1;
    }
  });
}
</script>

<template>
  <!-- 保险箱解锁门：未解锁时只显示密码界面，不加载任何本地数据 -->
  <div v-if="!unlocked" class="vault-gate">
    <div class="vault-card">
      <div class="vault-logo">P</div>
      <h2 class="vault-title">
        {{ vaultMode === "create" ? "Create your password" : "Enter password" }}
      </h2>
      <p class="vault-hint">
        {{
          vaultMode === "create"
            ? "First launch: choose a password to encrypt all local data. It cannot be recovered if forgotten."
            : "All local data is encrypted. Enter your password to unlock."
        }}
      </p>
      <q-input
        v-model="vaultPassword"
        type="password"
        filled
        dense
        autofocus
        placeholder="Password"
        :dark="$q.dark.isActive"
        :error="!!vaultError"
        :error-message="vaultError || undefined"
        @keyup.enter="submitVault"
      />
      <q-btn
        class="vault-submit full-width"
        color="primary"
        label="Unlock"
        :loading="vaultBusy"
        :disable="!vaultPassword"
        @click="submitVault"
      />
    </div>
  </div>

  <div v-else class="app-root" :class="{ mobile: isMobile }">
    <!-- 桌面：Telegram 双栏 + 可选详情面板 -->
    <template v-if="!isMobile">
      <aside class="app-sidebar">
        <ContactsPanel
          :conversations="conversations"
          :active-id="activeId"
          :connecting="!ready"
          @select="selectConversation"
          @add="openAdd"
          @settings="openSettings"
          @delete="deleteContactById"
        />
      </aside>
      <main class="app-main">
        <ChatWindow
          :conversation="activeConv"
          :messages="getMessages(activeConv?.nodeId ?? '')"
          @send="sendMessage"
          @resend="resendMessage"
          @info="openInfo"
        />
      </main>
      <ChatInfoPanel
        v-if="showInfo"
        :conversation="activeConv"
        @close="showInfo = false"
        @copy="copyText"
        @delete="deleteContact"
        @rename="renameContact"
      />

      <!-- 桌面端 Settings / Add / Mailbox 以右侧浮层展示 -->
      <q-dialog v-model="overlayOpen" :maximized="false" position="right">
        <div class="desktop-overlay">
          <Settings
            v-if="view === 'settings'"
            @close="closeOverlay"
            @open-mailbox="openMailbox"
          />
          <MyId v-else-if="view === 'add'" @close="closeOverlay" @added="onAdded" />
          <MailboxSettings v-else @close="closeOverlay" />
        </div>
      </q-dialog>
    </template>

    <!-- 移动端：单屏视图切换 -->
    <template v-else>
      <section v-show="view === 'list'" class="app-sidebar app-mobile-screen">
        <ContactsPanel
          :conversations="conversations"
          :active-id="activeId"
          :connecting="!ready"
          @select="selectConversation"
          @add="openAdd"
          @settings="openSettings"
          @delete="deleteContactById"
        />
      </section>
      <section v-show="view === 'chat'" class="app-main app-mobile-screen">
        <ChatWindow
          :conversation="activeConv"
          :messages="getMessages(activeConv?.nodeId ?? '')"
          :mobile="true"
          @send="sendMessage"
          @resend="resendMessage"
          @back="goToList"
          @info="openInfo"
        />
      </section>
      <section v-show="view === 'info'" class="app-mobile-screen">
        <ChatInfoPanel mobile :conversation="activeConv" @close="closeInfo" @copy="copyText" @delete="deleteContact" @rename="renameContact" />
      </section>
      <section v-show="view === 'settings'" class="app-mobile-screen">
        <Settings
          mobile
          @close="goToList"
          @open-mailbox="openMailbox"
        />
      </section>
      <section v-show="view === 'mailbox'" class="app-mobile-screen">
        <MailboxSettings mobile @close="goToList" />
      </section>
      <section v-show="view === 'add'" class="app-mobile-screen">
        <MyId mobile @close="goToList" @added="onAdded" />
      </section>
    </template>
  </div>
</template>

<style>
html,
body,
#app {
  height: 100%;
  margin: 0;
  overflow: hidden;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,
    "Helvetica Neue", Arial, sans-serif;
  -webkit-tap-highlight-color: transparent;
  user-select: none;
}

.app-root {
  display: flex;
  height: 100%;
  width: 100%;
  overflow: hidden;
  background: var(--app-bg);
}

/* 扫码时：让整条 DOM 链透明，露出 barcode-scanner 插件置于 webview 底下的相机预览 */
body.scanning-active,
body.scanning-active html,
body.scanning-active #app,
body.scanning-active .app-root,
body.scanning-active .app-root .app-mobile-screen,
body.scanning-active .app-root .app-main,
body.scanning-active .app-root section,
body.scanning-active .q-layout,
body.scanning-active .q-page-container,
body.scanning-active .q-page {
  background: transparent !important;
}

.app-root:not(.mobile) .app-sidebar {
  width: clamp(300px, 28vw, 420px);
  min-width: 300px;
  flex-shrink: 0;
  overflow: hidden;
  border-right: 1px solid rgba(255, 255, 255, 0.06);
}

.app-root:not(.mobile) .app-main {
  flex: 1;
  min-width: 0;
}

.app-root.mobile .app-sidebar,
.app-root.mobile .app-main,
.app-root.mobile .app-mobile-screen {
  position: absolute;
  top: 0;
  right: 0;
  bottom: env(safe-area-inset-bottom, 0px);
  left: 0;
  width: 100%;
  overflow: hidden;
}

.app-root.mobile .app-main {
  background: var(--app-bg);
}

/* Quasar 默认深色主题（跟随系统/深色） */
:root {
  --app-bg: #121212;
  --sidebar-bg: #1d1d1d;
  --rail-bg: #1a1a1a;
  --input-bg: #2c2c2c;
  --bubble-in: #242424;
  --bubble-out: #1976d2;
  --text-primary: #e0e0e0;
  --text-secondary: #9e9e9e;
  --accent: #1976d2;
  --row-active: rgba(255, 255, 255, 0.08);
}

/* Quasar 默认浅色主题 */
:root.light {
  --app-bg: #f5f5f5;
  --sidebar-bg: #ffffff;
  --rail-bg: #e8e8e8;
  --input-bg: #e0e0e0;
  --bubble-in: #ffffff;
  --bubble-out: #bbdefb;
  --text-primary: #212121;
  --text-secondary: #757575;
  --accent: #1976d2;
  --row-active: rgba(0, 0, 0, 0.06);
}

.desktop-overlay {
  width: 380px;
  height: 100vh;
  max-height: 100vh;
  overflow: hidden;
  background: var(--app-bg);
}

.desktop-overlay .settings,
.desktop-overlay .my-id,
.desktop-overlay .mailbox-settings {
  height: 100%;
  width: 100%;
}

.q-dialog__inner--right {
  padding: 0;
}

/* 保险箱解锁门 */
.vault-gate {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--app-bg);
  z-index: 1000;
}

.vault-card {
  width: min(360px, 90vw);
  padding: 32px 28px;
  border-radius: 14px;
  background: var(--sidebar-bg);
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.vault-logo {
  width: 56px;
  height: 56px;
  border-radius: 50%;
  background: var(--accent);
  color: #fff;
  font-size: 28px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  align-self: center;
}

.vault-title {
  margin: 0;
  text-align: center;
  font-size: 20px;
  color: var(--text-primary);
}

.vault-hint {
  margin: 0;
  text-align: center;
  font-size: 13px;
  line-height: 1.5;
  color: var(--text-secondary);
}

.vault-submit {
  margin-top: 4px;
}
</style>