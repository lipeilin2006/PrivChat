<script setup>
import { computed, ref } from "vue";
import { useQuasar } from "quasar";
import { avatarColor, initials } from "../utils/avatar";
import { useI18n } from "../i18n";

const $q = useQuasar();
const { t } = useI18n();

const props = defineProps({
  conversations: { type: Array, required: true },
  activeId: { type: String, default: null },
  connecting: { type: Boolean, default: false },
});

const emit = defineEmits(["select", "add", "settings", "delete", "search-messages"]);

const search = ref("");
const contextConv = ref(null);
const contextMenu = ref(false);

function openContext(conv) {
  contextConv.value = conv;
  contextMenu.value = true;
}

async function confirmDelete() {
  if (!contextConv.value) return;
  const { value: ok } = await $q.dialog({
    title: t("contacts.deleteTitle"),
    message: t("contacts.deleteMessage", { name: contextConv.value.name }),
    cancel: true,
    persistent: true,
    color: "negative",
  });
  if (ok) emit("delete", contextConv.value.nodeId);
}

const filtered = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return props.conversations;
  return props.conversations.filter(
    (c) => c.name.toLowerCase().includes(q) || c.nodeId.toLowerCase().includes(q)
  );
});

function fmtTime(ms) {
  if (!ms) return "";
  return new Date(ms).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function submitSearch() {
  const query = search.value.trim();
  if (query) emit("search-messages", query);
}
</script>

<template>
  <div class="panel">
    <div class="panel-header">
      <div class="panel-title q-px-md q-pt-md q-pb-xs">
        <div class="row items-center">
          <q-btn
            flat
            round
            dense
            color="grey-4"
            icon="menu"
            :title="t('contacts.settings')"
            @click="emit('settings')"
          />
          <div class="text-subtitle1 text-weight-medium q-ml-sm">{{ t("contacts.messages") }}</div>
        </div>
        <q-btn
          flat
          round
          dense
          color="grey-4"
          icon="person_add"
          :title="t('contacts.add')"
          :disable="connecting"
          @click="emit('add')"
        />
      </div>
      <div class="row items-center q-px-md q-py-sm">
        <q-input
          v-model="search"
          :dark="$q.dark.isActive"
          dense
          rounded
          outlined
          color="primary"
          class="col"
          :placeholder="t('contacts.search')"
          @keyup.enter="submitSearch"
        >
          <template #prepend>
            <q-icon name="search" size="18px" color="grey-5" />
          </template>
          <template v-if="search" #append>
            <q-icon name="cancel" size="16px" color="grey-5" class="cursor-pointer q-mr-sm" @click="search = ''" />
          </template>
        </q-input>
      </div>
    </div>

    <div class="list">
      <q-scroll-area class="scroll" style="height: 0">
        <q-list padding class="q-py-xs">
          <template v-if="filtered.length">
            <q-item
              v-for="c in filtered"
              :key="c.id"
              clickable
              v-ripple
              :active="c.id === activeId"
              active-class="row-selected"
              class="conv-item"
              @click="emit('select', c.id)"
              @click.right.prevent="openContext(c)"
              @contextmenu.prevent="openContext(c)"
            >
              <q-item-section avatar>
                <q-avatar
                  :style="{ backgroundColor: avatarColor(c.name) }"
                  text-color="white"
                  size="54px"
                  font-size="20px"
                >
                  {{ initials(c.name) }}
                </q-avatar>
              </q-item-section>

              <q-item-section class="conv-main">
                <q-item-label class="row items-center justify-between no-wrap">
                  <span class="conv-name ellipsis">{{ c.name }}</span>
                  <span class="conv-time">{{ fmtTime(c.timestamp) }}</span>
                </q-item-label>
                <q-item-label class="conv-preview row items-center no-wrap">
                  <q-icon
                    v-if="c.lastStatus === 'sending'"
                    name="schedule"
                    size="14px"
                    color="grey-6"
                    class="q-mr-xs"
                  />
                  <q-icon
                    v-else-if="c.lastStatus === 'sent'"
                    name="done_all"
                    size="14px"
                    color="grey-6"
                    class="q-mr-xs"
                  />
                  <q-icon
                    v-else-if="c.lastStatus === 'failed'"
                    name="error"
                    size="14px"
                    color="negative"
                    class="q-mr-xs"
                  />
                  <span class="ellipsis text-grey-5">{{ c.lastMessage }}</span>
                </q-item-label>
              </q-item-section>

              <q-item-section v-if="c.unread" side>
                <q-badge class="unread-badge" rounded>
                  {{ c.unread > 99 ? "99+" : c.unread }}
                </q-badge>
              </q-item-section>
            </q-item>
          </template>

          <div v-else class="empty-state text-center q-py-lg">
            <q-icon name="forum" size="48px" color="grey-7" />
            <div class="text-grey-6 q-mt-sm">
              {{ search ? t("contacts.noMatches") : t("contacts.empty") }}
            </div>
            <q-btn
              v-if="!search"
              unelevated
              color="primary"
              icon="person_add"
              :label="t('contacts.add')"
              class="q-mt-md"
              :disable="connecting"
              @click="emit('add')"
            />
          </div>
        </q-list>
      </q-scroll-area>
    </div>

    <q-menu v-model="contextMenu" context-menu fit>
      <q-list dense style="min-width: 180px">
        <q-item
          clickable
          v-ripple
          @click="contextMenu = false; emit('select', contextConv.id)"
        >
          <q-item-section avatar>
            <q-icon name="chat" size="18px" color="grey-5" />
          </q-item-section>
          <q-item-section>{{ t("contacts.open") }}</q-item-section>
        </q-item>
        <q-separator />
        <q-item clickable v-ripple class="text-negative" @click="confirmDelete">
          <q-item-section avatar>
            <q-icon name="person_remove" size="18px" color="negative" />
          </q-item-section>
          <q-item-section>{{ t("contacts.delete") }}</q-item-section>
        </q-item>
      </q-list>
    </q-menu>
  </div>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background: var(--sidebar-bg);
}

.panel-header {
  flex-shrink: 0;
  padding-bottom: 2px;
  padding-top: env(safe-area-inset-top, 0px);
}

.panel-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--text-primary);
}

.list {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.scroll {
  flex: 1;
  min-height: 0;
}

/* 内容宽度始终等于滚动容器，禁止横向溢出与横向滚动条 */
.scroll :deep(.q-scroll-area__content) {
  width: 100%;
  min-width: 100%;
  max-width: 100%;
  overflow-x: hidden;
}

/* 彻底隐藏横向滚动条轨道/滑块 */
.scroll :deep(.q-scrollarea__bar--h),
.scroll :deep(.q-scrollarea__thumb--h) {
  display: none !important;
}

.conv-item {
  border-radius: 10px;
  margin: 0 8px;
  overflow: hidden;
}

.conv-item :deep(.q-item__section--main) {
  min-width: 0;
}

.conv-main {
  min-width: 0;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px 16px;
}

.row-selected {
  background: var(--row-active) !important;
}

.row-selected .conv-name {
  color: var(--text-primary);
}

.row-selected .conv-preview .text-grey-5,
.row-selected .conv-time {
  color: var(--text-secondary) !important;
}

.conv-name {
  font-size: 15px;
  font-weight: 500;
  color: var(--text-primary);
  min-width: 0;
  flex: 1 1 auto;
}

.conv-time {
  font-size: 12px;
  color: var(--text-secondary);
  margin-left: 8px;
  flex-shrink: 0;
}

.conv-preview {
  font-size: 13px;
  color: var(--text-secondary);
  margin-top: 2px;
  min-width: 0;
}

.conv-preview .ellipsis {
  min-width: 0;
}

.unread-badge {
  background: var(--accent);
  color: #fff;
  font-size: 12px;
  min-width: 20px;
  padding: 2px 6px;
}
</style>
