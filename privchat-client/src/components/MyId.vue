<script setup>
import { nextTick, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { Format, scan, cancel, requestPermissions } from "@tauri-apps/plugin-barcode-scanner";
import QRCode from "qrcode";
import { useQuasar } from "quasar";

const $q = useQuasar();

const emit = defineEmits(["close", "added"]);

const props = defineProps({
  mobile: { type: Boolean, default: false },
});

// —— 我的 ID / 邀请码 ——
const loadingId = ref(true);
const myId = ref("");
const qrCanvas = ref(null);
const copied = ref("");

// —— 添加联系人 ——
const name = ref("");
const ticket = ref("");
const loadingAdd = ref(false);
const scanning = ref(false);

async function loadMyId() {
  loadingId.value = true;
  try {
    const id = await invoke("get_self_ticket");
    myId.value = id;
    // 先切走 loading，让 v-else 分支的 canvas 真正挂载到 DOM，
    // 再 nextTick 后绘制二维码（否则 qrCanvas 为 null，QR 不显示）。
    loadingId.value = false;
    await nextTick();
    if (qrCanvas.value) {
      await QRCode.toCanvas(qrCanvas.value, id, {
        width: 220,
        margin: 1,
        errorCorrectionLevel: "L",
        color: { dark: "#000000", light: "#ffffff" },
      });
    }
  } catch (e) {
    loadingId.value = false;
    $q.notify({ type: "negative", message: `Failed to create ID: ${e}` });
  }
}

async function refreshMyId() {
  await loadMyId();
}

async function copyMyId() {
  try {
    await navigator.clipboard.writeText(myId.value);
    copied.value = "ID";
    $q.notify({ type: "positive", message: "ID copied", position: "bottom" });
  } catch (e) {
    $q.notify({ type: "negative", message: `Copy failed: ${e}`, position: "bottom" });
  } finally {
    setTimeout(() => {
      copied.value = "";
    }, 800);
  }
}

async function startScan() {
  scanning.value = true;
  document.body.classList.add("scanning-active");
  try {
    console.log("[scan] requesting permissions...");
    const perm = await requestPermissions();
    const cameraState = typeof perm === "string" ? perm : (perm && perm.camera);
    console.log("[scan] permission state:", cameraState);
    if (cameraState !== "granted") {
      $q.notify({
        type: "warning",
        message: "Camera permission required to scan a QR code",
        position: "bottom",
      });
      return;
    }
    console.log("[scan] starting scan");
    const { content } = await scan({
      formats: [Format.QRCode],
      windowed: true,
    });
    console.log("[scan] result:", content);
    ticket.value = content.trim();
  } catch (e) {
    console.error("[scan] error:", e, e && e.message);
    if (scanning.value) {
      $q.notify({
        type: "warning",
        message: `Camera scan unavailable: ${e}`,
        position: "bottom",
      });
    }
  } finally {
    scanning.value = false;
    document.body.classList.remove("scanning-active");
  }
}

async function stopScan() {
  scanning.value = false;
  try {
    await cancel();
  } catch (e) {
    console.error("[scan] cancel error:", e);
  }
}

async function submit() {
  if (!ticket.value.trim()) {
    $q.notify({
      type: "warning",
      message: "Please paste a connection ticket",
      position: "bottom",
    });
    return;
  }
  loadingAdd.value = true;
  try {
    const peerId = await invoke("connect_peer", {
      ticket: ticket.value.trim(),
      name: name.value.trim() || null,
    });
    $q.notify({
      type: "positive",
      message: "Contact added",
      position: "bottom",
    });
    // 清空添加联系人输入框，并刷新生成一个新的 ID（邀请码）。
    name.value = "";
    ticket.value = "";
    await loadMyId();
    emit("added", peerId);
  } catch (e) {
    $q.notify({
      type: "negative",
      message: `Connect failed: ${e}`,
      position: "bottom",
    });
  } finally {
    loadingAdd.value = false;
  }
}

onMounted(loadMyId);
</script>

<template>
  <div class="my-id" :class="{ 'is-scanning': scanning }">
    <div class="myid-header">
      <q-btn flat round dense icon="arrow_back" color="grey-4" @click="emit('close')" />
      <span class="text-subtitle2">My ID</span>
    </div>

    <div class="myid-scroll">
      <!-- 我的 ID：进入先加载，成功后展示 id + 二维码，可手动刷新 -->
      <div v-if="loadingId" class="id-loading">
        <q-spinner color="primary" size="48px" />
        <div class="text-grey-6 q-mt-md">Creating your ID…</div>
      </div>

      <div v-else class="id-card">
        <div class="id-title">Scan to add me</div>
        <canvas ref="qrCanvas" class="qr-canvas" />
        <div class="id-value mono">{{ myId }}</div>
        <div class="id-actions">
          <q-btn
            unelevated
            outline
            no-caps
            color="primary"
            label="Copy ID"
            class="full-width q-mb-sm"
            :loading="copied === 'ID'"
            @click="copyMyId"
          />
          <q-btn
            unelevated
            no-caps
            color="primary"
            label="Refresh ID"
            class="full-width"
            @click="refreshMyId"
          />
        </div>
        <div class="id-hint text-grey-6">
          Each refresh creates a new ID. Share it with one person to connect.
        </div>
      </div>

      <!-- 添加联系人 -->
      <div class="add-section">
        <div class="add-title">Add contact</div>
        <div class="form q-gutter-md">
          <q-input
            v-model="name"
            :dark="$q.dark.isActive"
            filled
            color="primary"
            label="Display name"
            placeholder="Alice"
            autocomplete="off"
          />
          <q-input
            v-model="ticket"
            :dark="$q.dark.isActive"
            filled
            color="primary"
            type="textarea"
            label="Peer ID"
            placeholder="Paste the peer's ID…"
            :rules="[(v) => v.trim().length > 0 || 'Peer ID is required']"
            autogrow
          />
          <q-btn
            v-if="props.mobile"
            outline
            no-caps
            color="primary"
            icon="qr_code_scanner"
            label="Scan QR code"
            class="form-btn"
            :loading="scanning"
            @click="startScan"
          />
          <q-btn
            unelevated
            no-caps
            color="primary"
            label="Add"
            icon="person_add"
            class="form-btn"
            :loading="loadingAdd"
            :disable="!ticket.trim()"
            @click="submit"
          />
        </div>
      </div>
    </div>

    <!-- 扫码覆盖层：中间镂空让下方的 CameraX 预览可见，四周遮罩 + 取消按钮 -->
    <div v-if="scanning" class="scan-overlay">
      <div class="scan-mask">
        <div class="scan-frame" />
      </div>
      <div class="scan-actions">
        <q-btn
          unelevated
          no-caps
          color="white"
          text-color="black"
          icon="close"
          label="Cancel"
          class="scan-cancel"
          @click="stopScan"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.my-id {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  overflow: hidden;
  background: var(--app-bg);
}

.my-id.is-scanning {
  background: transparent;
}

.my-id.is-scanning .myid-header,
.my-id.is-scanning .myid-scroll {
  display: none;
}

.myid-header {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  height: calc(56px + env(safe-area-inset-top, 0px));
  padding: 0 8px;
  padding-top: env(safe-area-inset-top, 0px);
  background: var(--sidebar-bg);
}

.myid-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.id-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 48px 0;
}

.id-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}

.id-title {
  color: var(--text-primary);
  font-size: 16px;
  font-weight: 600;
}

.qr-canvas {
  background: #ffffff;
  border-radius: 8px;
  padding: 6px;
}

.id-value {
  color: var(--text-secondary);
  font-size: 11px;
  word-break: break-all;
  text-align: center;
  max-width: 100%;
}

.mono {
  font-family: "SF Mono", "Cascadia Code", Consolas, monospace;
}

.id-actions {
  width: 80%;
}

.id-hint {
  font-size: 12px;
  text-align: center;
}

.add-section {
  margin-top: 24px;
}

.add-title {
  color: var(--text-primary);
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 12px;
}

/* q-gutter-md 给每个子元素加 16px 左边距；按钮去掉 full-width 后
   与输入框同宽（100% 容器宽减 16px），自然与上方输入框对齐 */
.form .form-btn {
  width: calc(100% - 16px);
}

.scan-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  background: rgba(0, 0, 0, 0.35);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: 32px;
}

.scan-mask {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.scan-frame {
  width: 70vw;
  max-width: 320px;
  aspect-ratio: 1;
  border: 2px solid rgba(255, 255, 255, 0.9);
  border-radius: 12px;
  background: transparent;
  box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.35);
}

.scan-actions {
  display: flex;
  justify-content: center;
  padding-top: 16px;
}

.scan-cancel {
  min-width: 180px;
}
</style>
