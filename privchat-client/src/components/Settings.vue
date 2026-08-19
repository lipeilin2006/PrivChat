<script setup>
import { ref } from "vue";
import { useQuasar } from "quasar";

const $q = useQuasar();

const props = defineProps({
  mobile: { type: Boolean, default: false },
});

const emit = defineEmits(["close", "open-mailbox"]);

const THEME_KEY = "privchat_theme";
const themeMode = ref(
  JSON.parse(localStorage.getItem(THEME_KEY) ?? JSON.stringify("auto"))
);
const themeOptions = [
  { label: "System", value: "auto" },
  { label: "Dark", value: true },
  { label: "Light", value: false },
];

function setTheme(v) {
  themeMode.value = v;
  localStorage.setItem(THEME_KEY, JSON.stringify(v));
  $q.dark.set(v);
}
</script>

<template>
  <div class="settings">
    <div class="settings-header">
      <q-btn flat round dense icon="arrow_back" color="grey-4" @click="emit('close')" />
      <span class="text-subtitle2">Settings</span>
    </div>

    <div class="settings-scroll">
      <!-- Mailbox 入口（独立页面管理多个节点） -->
      <q-list class="settings-list" :dark="$q.dark.isActive" bordered separator>
        <q-item clickable v-ripple @click="emit('open-mailbox')">
          <q-item-section avatar>
            <q-icon name="mark_email_unread" color="primary" />
          </q-item-section>
          <q-item-section>
            <q-item-label>Mailbox</q-item-label>
            <q-item-label caption>Manage offline message relays</q-item-label>
          </q-item-section>
          <q-item-section side>
            <q-icon name="chevron_right" color="grey-5" />
          </q-item-section>
        </q-item>
      </q-list>

      <!-- 外观 -->
      <q-list class="settings-list" :dark="$q.dark.isActive" bordered separator>
        <q-item>
          <q-item-section avatar>
            <q-icon name="contrast" color="primary" />
          </q-item-section>
          <q-item-section>
            <q-item-label>Theme</q-item-label>
            <q-item-label caption>Follow system or choose manually</q-item-label>
          </q-item-section>
          <q-item-section side>
            <q-select
              v-model="themeMode"
              :options="themeOptions"
              :dark="$q.dark.isActive"
              emit-value
              map-options
              dense
              outlined
              options-dense
              color="primary"
              style="min-width: 110px"
              @update:model-value="setTheme"
            />
          </q-item-section>
        </q-item>
      </q-list>
    </div>
  </div>
</template>

<style scoped>
.settings {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  overflow: hidden;
  background: var(--app-bg);
}

.settings-header {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  height: calc(56px + env(safe-area-inset-top, 0px));
  padding: 0 8px;
  padding-top: env(safe-area-inset-top, 0px);
  background: var(--sidebar-bg);
}

.settings-scroll {
  padding-top: 8px;
  flex: 1;
  overflow-y: auto;
}

.settings-list {
  margin: 8px 12px;
  border-radius: 12px;
  overflow: hidden;
}
</style>