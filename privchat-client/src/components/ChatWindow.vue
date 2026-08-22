<script setup>
import { computed, nextTick, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useQuasar } from "quasar";
import { avatarColor, initials } from "../utils/avatar";
import { useI18n } from "../i18n";

const $q = useQuasar();
const { t } = useI18n();

const props = defineProps({
  conversation: { type: Object, default: null },
  messages: { type: Array, required: true },
  mobile: { type: Boolean, default: false },
});

const emit = defineEmits(["send", "back", "info", "resend"]);

const draft = ref("");
const scrollArea = ref(null);
const contextMsg = ref(null);
const contextMenu = ref(false);
const showScrollFab = ref(false);

function onScroll(info) {
  const pos = info?.position;
  if (!pos) return;
  showScrollFab.value = pos.bottom > 80;
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
      $q.notify({ type: "positive", message: t("chat.copied"), position: "bottom" });
  } catch (e) {
      $q.notify({ type: "negative", message: t("chat.copyFailed", { error: e }), position: "bottom" });
  }
}

function openContext(msg) {
  contextMsg.value = msg;
  contextMenu.value = true;
}

function scrollToBottom() {
  scrollArea.value?.setScrollPercentage(1, 1);
}

const grouped = computed(() => {
  const out = [];
  let lastDay = null;
  for (const m of props.messages) {
    const day = new Date(m.ts).toDateString();
    if (day !== lastDay) {
      out.push({ type: "date", ts: m.ts });
      lastDay = day;
    }
    out.push({ type: "msg", msg: m });
  }
  return out;
});

watch(
  () => props.conversation?.nodeId,
  async (peerId) => {
    draft.value = peerId ? await invoke("get_draft", { peerId }).catch(() => "") : "";
  },
  { immediate: true }
);

watch(
  draft,
  (text) => {
    const peerId = props.conversation?.nodeId;
    if (peerId) invoke("save_draft", { peerId, text }).catch(() => {});
  }
);

watch(
  () => props.messages.length,
  async () => {
    await nextTick();
    scrollArea.value?.setScrollPercentage(1, 1);
    showScrollFab.value = false;
  },
  { immediate: true }
);

function fmtTime(ms) {
  return new Date(ms).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function fmtDay(ms) {
  const d = new Date(ms);
  const today = new Date();
  const yesterday = new Date();
  yesterday.setDate(today.getDate() - 1);
  const same = (a, b) =>
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate();
  if (same(d, today)) return t("chat.today");
  if (same(d, yesterday)) return t("chat.yesterday");
  return d.toLocaleDateString([], {
    month: "long",
    day: "numeric",
    year: d.getFullYear() !== today.getFullYear() ? "numeric" : undefined,
  });
}

function submit() {
  const text = draft.value;
  if (!text.trim()) return;
  emit("send", text);
  draft.value = "";
}

function resend(msg) {
  if (msg.status === "failed") emit("resend", msg);
}
</script>

<template>
  <div v-if="conversation" class="chat">
    <div class="chat-header">
      <q-btn
        flat
        round
        dense
        icon="arrow_back"
        color="grey-4"
        class="mobile-back"
        :class="{ 'is-mobile': mobile }"
        @click="emit('back')"
      />
      <q-avatar
        :style="{ backgroundColor: avatarColor(conversation.name) }"
        text-color="white"
        size="38px"
        font-size="16px"
        class="q-mr-sm"
      >
        {{ initials(conversation.name) }}
      </q-avatar>
      <div class="chat-title">
        <div class="chat-name ellipsis">{{ conversation.name }}</div>
        <div class="chat-sub ellipsis">
          <span class="dot" :class="conversation.online ? 'online' : 'offline'" />
          <q-icon name="lock" size="12px" color="grey-5" />
          {{ conversation.nodeId.slice(0, 24) }}
        </div>
      </div>
      <q-btn
        flat
        round
        dense
        icon="more_vert"
        color="grey-4"
        @click="emit('info')"
      />
    </div>

    <q-scroll-area ref="scrollArea" class="chat-scroll" style="height: 0" @scroll="onScroll">
      <div class="chat-bg">
        <div class="chat-inner">
          <template v-for="(g, i) in grouped" :key="i">
            <div v-if="g.type === 'date'" class="date-sep">
              <span>{{ fmtDay(g.ts) }}</span>
            </div>
            <div
              v-else
              class="bubble-row"
              :class="g.msg.from === 'me' ? 'mine' : 'theirs'"
            >
              <div
                class="bubble"
                :class="[
                  g.msg.from === 'me' ? 'bubble-out' : 'bubble-in',
                  g.msg.status === 'failed' ? 'bubble-failed' : '',
                ]"
                @click.right.prevent="openContext(g.msg)"
                @contextmenu.prevent="openContext(g.msg)"
                @click="resend(g.msg)"
              >
                <div class="bubble-text">{{ g.msg.text }}</div>
                <div class="bubble-meta">
                  <span>{{ fmtTime(g.msg.ts) }}</span>
                  <template v-if="g.msg.from === 'me'">
                     <q-icon
                       v-if="['queued', 'sending', 'resending'].includes(g.msg.status)"
                       :name="g.msg.status === 'queued' ? 'schedule' : 'sync'"
                       size="14px"
                       color="grey-5"
                       :class="{ 'sending-icon': ['sending', 'resending'].includes(g.msg.status) }"
                     />
                    <q-icon
                      v-else-if="g.msg.status === 'failed'"
                      name="error"
                      size="14px"
                      color="negative"
                    />
                    <q-icon v-else name="done_all" size="14px" />
                  </template>
                </div>
                  <div v-if="g.msg.status === 'failed'" class="bubble-retry">
                   {{ g.msg.error || t("chat.resend") }} · {{ t("chat.resend") }}
                  </div>
                  <div v-else-if="g.msg.status === 'resending'" class="bubble-retry">
                   {{ t("chat.resending") }}
                  </div>
                  <div v-else-if="g.msg.status === 'cancelled'" class="bubble-retry">
                   {{ t("chat.cancelled") }}
                  </div>
              </div>
            </div>
          </template>
        </div>
      </div>
    </q-scroll-area>

    <q-btn
      v-show="showScrollFab"
      class="scroll-fab"
      round
      unelevated
      color="primary"
      icon="keyboard_arrow_down"
      @click="scrollToBottom"
    />

    <q-menu v-model="contextMenu" context-menu fit>
      <q-list dense style="min-width: 180px">
        <q-item clickable v-ripple @click="copyText(contextMsg?.text)">
          <q-item-section avatar>
            <q-icon name="content_copy" size="18px" color="grey-5" />
          </q-item-section>
          <q-item-section>{{ t("chat.copy") }}</q-item-section>
        </q-item>
      </q-list>
    </q-menu>

    <div class="composer">
      <q-input
        v-model="draft"
        :dark="$q.dark.isActive"
        rounded
        filled
        class="composer-input"
         :placeholder="t('chat.message')"
        autogrow
        :maxlength="4000"
        @keyup.enter="submit"
        @keydown.enter.exact.prevent="submit"
      />
      <q-btn
        v-if="draft.trim()"
        unelevated
        round
        color="primary"
        icon="send"
        class="send-btn"
        @click="submit"
      />
    </div>
  </div>

  <div v-else class="empty">
    <div class="empty-inner">
      <div class="empty-icon">
        <q-icon name="lock" size="64px" color="grey-6" />
      </div>
      <div class="text-h6 text-grey-5 q-mt-md">PrivChat</div>
      <div class="text-grey-6 q-mt-xs">
         {{ t("chat.encrypted") }}
      </div>
    </div>
  </div>
</template>

<style scoped>
.chat {
  position: relative;
  display: flex;
  flex-direction: column;
   height: 100%;
   min-height: 0;
  background: var(--app-bg);
}

.chat-header {
  display: flex;
  align-items: center;
  padding: 6px 12px;
  padding-top: calc(env(safe-area-inset-top, 0px) + 6px);
  min-height: calc(56px + env(safe-area-inset-top, 0px));
  background: var(--sidebar-bg);
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  flex-shrink: 0;
}

@media (max-width: 599px) {
  .chat {
    height: var(--visual-viewport-height, 100dvh);
    max-height: var(--visual-viewport-height, 100dvh);
    min-height: 0;
  }

  .chat-header {
    position: relative;
    z-index: 2;
    flex: 0 0 auto;
  }

  .chat-scroll {
    min-height: 0;
  }
}

.mobile-back {
  display: none;
}

.mobile-back.is-mobile {
  display: inline-flex;
}

.chat-title {
  flex: 1;
  min-width: 0;
}

.chat-name {
  font-size: 15px;
  font-weight: 500;
  color: var(--text-primary);
}

.chat-sub {
  font-size: 12px;
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  gap: 6px;
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}

.dot.online {
  background: var(--positive);
}

.dot.offline {
  background: var(--text-secondary);
}

.chat-scroll {
  flex: 1;
}

.chat-bg {
  height: 100%;
  background-image: radial-gradient(
    rgba(255, 255, 255, 0.02) 1px,
    transparent 1px
  );
  background-size: 24px 24px;
}

.chat-inner {
  max-width: 720px;
  margin: 0 auto;
  padding: 14px 16px 8px;
  min-height: 100%;
}

.date-sep {
  text-align: center;
  margin: 12px 0;
}

.date-sep span {
  background: var(--sidebar-bg);
  color: var(--text-secondary);
  font-size: 12px;
  padding: 4px 12px;
  border-radius: 12px;
}

.bubble-row {
  display: flex;
  margin: 2px 0;
}

.bubble-row.mine {
  justify-content: flex-end;
}

.bubble-row.theirs {
  justify-content: flex-start;
}

.bubble {
  position: relative;
  max-width: 78%;
  padding: 7px 12px 6px;
  border-radius: 12px;
  word-break: break-word;
}

.bubble-out {
  background: var(--bubble-out);
  border-top-right-radius: 4px;
}

.bubble-in {
  background: var(--bubble-in);
  border-top-left-radius: 4px;
}

.bubble-failed {
  border: 1px solid var(--negative);
  cursor: pointer;
}

.bubble-retry {
  font-size: 11px;
  color: var(--negative);
  text-align: right;
  margin-top: 2px;
}

.bubble-text {
  font-size: 15px;
  line-height: 1.35;
  color: var(--text-primary);
  white-space: pre-wrap;
}

.bubble-meta {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 4px;
  font-size: 11px;
   color: var(--bubble-meta);
  margin-top: 3px;
  float: right;
  margin-left: 10px;
}

.sending-icon {
  animation: sending-icon-spin 1s linear infinite;
}

@keyframes sending-icon-spin {
  to {
    transform: rotate(360deg);
  }
}

.composer {
  display: flex;
  align-items: flex-end;
  gap: 8px;
  padding: 10px 12px;
  padding-bottom: calc(10px + env(safe-area-inset-bottom, 0px));
  background: var(--sidebar-bg);
  flex-shrink: 0;
}

.composer-input {
  flex: 1;
}

.composer-input :deep(.q-field__control) {
  border-radius: 24px;
  background: var(--input-bg);
  min-height: 44px;
}

.composer-input :deep(.q-field__native) {
  max-height: 120px;
}

.send-btn {
  margin-bottom: 2px;
}

.scroll-fab {
  position: absolute;
  right: 16px;
  bottom: 96px;
  z-index: 5;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.35);
}

.empty {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--app-bg);
}

.empty-inner {
  text-align: center;
  padding: 24px;
}

.empty-icon {
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>
