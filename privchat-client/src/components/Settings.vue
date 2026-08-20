<script setup>
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useQuasar } from "quasar";
import { useI18n } from "../i18n";

const $q = useQuasar();
const { locale, t, setLocale } = useI18n();

const props = defineProps({
  mobile: { type: Boolean, default: false },
});

const emit = defineEmits(["close", "open-mailbox", "lock", "auto-lock-changed", "diagnostics"]);
const passwordDialog = ref(false);
const oldPassword = ref("");
const newPassword = ref("");
const confirmPassword = ref("");
const changingPassword = ref(false);
const autoLockMinutes = ref("0");
const autoLockOptions = [
  { label: t("settings.never"), value: "0" },
  { label: t("settings.oneMinute"), value: "1" },
  { label: t("settings.fiveMinutes"), value: "5" },
];

const THEME_KEY = "privchat_theme";
const themeMode = ref(
  JSON.parse(localStorage.getItem(THEME_KEY) ?? JSON.stringify("auto"))
);
const themeOptions = [
  { label: t("settings.system"), value: "auto" },
  { label: t("settings.dark"), value: true },
  { label: t("settings.light"), value: false },
];

const languageOptions = [
  { label: t("settings.chinese"), value: "zh-CN" },
  { label: t("settings.english"), value: "en" },
];

function setTheme(v) {
  themeMode.value = v;
  localStorage.setItem(THEME_KEY, JSON.stringify(v));
  $q.dark.set(v);
}

function setLanguage(v) {
  setLocale(v);
  window.location.reload();
}

function setAutoLock(value) {
  autoLockMinutes.value = value;
  emit("auto-lock-changed", Number(value));
  invoke("set_auto_lock_minutes", { minutes: Number(value) }).catch(() => {
    $q.notify({ type: "negative", message: t("settings.autoLockSaveFailed"), position: "bottom" });
  });
}

onMounted(async () => {
  try {
    autoLockMinutes.value = String(await invoke("get_auto_lock_minutes"));
  } catch {
    autoLockMinutes.value = "0";
  }
});

async function changePassword() {
  if (!oldPassword.value || !newPassword.value || newPassword.value !== confirmPassword.value) {
    $q.notify({ type: "negative", message: t("settings.passwordMismatch"), position: "bottom" });
    return;
  }
  changingPassword.value = true;
  try {
    await invoke("change_vault_password", {
      oldPassword: oldPassword.value,
      newPassword: newPassword.value,
    });
    passwordDialog.value = false;
    oldPassword.value = "";
    newPassword.value = "";
    confirmPassword.value = "";
    $q.notify({ type: "positive", message: t("settings.passwordChanged"), position: "bottom" });
  } catch (error) {
    $q.notify({ type: "negative", message: String(error), position: "bottom" });
  } finally {
    changingPassword.value = false;
  }
}
</script>

<template>
  <div class="settings">
    <div class="settings-header">
      <q-btn flat round dense icon="arrow_back" color="grey-4" @click="emit('close')" />
      <span class="text-subtitle2">{{ t("settings.title") }}</span>
    </div>

    <div class="settings-scroll">
      <!-- Mailbox 入口（独立页面管理多个节点） -->
      <q-list class="settings-list" :dark="$q.dark.isActive" bordered separator>
        <q-item clickable v-ripple @click="emit('open-mailbox')">
          <q-item-section avatar>
            <q-icon name="mark_email_unread" color="primary" />
          </q-item-section>
          <q-item-section>
            <q-item-label>{{ t("settings.mailbox") }}</q-item-label>
            <q-item-label caption>{{ t("settings.mailboxHint") }}</q-item-label>
          </q-item-section>
          <q-item-section side>
            <q-icon name="chevron_right" color="grey-5" />
          </q-item-section>
        </q-item>
      </q-list>

      <q-list class="settings-list" :dark="$q.dark.isActive" bordered separator>
        <q-item clickable v-ripple @click="emit('diagnostics')">
          <q-item-section avatar><q-icon name="bug_report" color="primary" /></q-item-section>
          <q-item-section>
            <q-item-label>{{ t("settings.diagnostics") }}</q-item-label>
            <q-item-label caption>{{ t("settings.diagnosticsHint") }}</q-item-label>
          </q-item-section>
        </q-item>
      </q-list>

      <q-list class="settings-list" :dark="$q.dark.isActive" bordered separator>
        <q-item>
          <q-item-section avatar><q-icon name="timer" color="primary" /></q-item-section>
          <q-item-section>
            <q-item-label>{{ t("settings.autoLock") }}</q-item-label>
            <q-item-label caption>{{ t("settings.autoLockHint") }}</q-item-label>
          </q-item-section>
          <q-item-section side>
            <q-select :model-value="autoLockMinutes" :options="autoLockOptions" emit-value map-options dense outlined options-dense color="primary" style="min-width: 120px" @update:model-value="setAutoLock" />
          </q-item-section>
        </q-item>
        <q-item clickable v-ripple @click="passwordDialog = true">
          <q-item-section avatar><q-icon name="password" color="primary" /></q-item-section>
          <q-item-section>
            <q-item-label>{{ t("settings.changePassword") }}</q-item-label>
            <q-item-label caption>{{ t("settings.changePasswordHint") }}</q-item-label>
          </q-item-section>
        </q-item>
      </q-list>

      <q-list class="settings-list" :dark="$q.dark.isActive" bordered separator>
        <q-item clickable v-ripple @click="emit('lock')">
          <q-item-section avatar>
            <q-icon name="lock" color="primary" />
          </q-item-section>
          <q-item-section>
            <q-item-label>{{ t("settings.lock") }}</q-item-label>
            <q-item-label caption>{{ t("settings.lockHint") }}</q-item-label>
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
            <q-item-label>{{ t("settings.theme") }}</q-item-label>
            <q-item-label caption>{{ t("settings.themeHint") }}</q-item-label>
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

      <q-list class="settings-list" :dark="$q.dark.isActive" bordered separator>
        <q-item>
          <q-item-section avatar>
            <q-icon name="translate" color="primary" />
          </q-item-section>
          <q-item-section>
            <q-item-label>{{ t("settings.language") }}</q-item-label>
            <q-item-label caption>{{ t("settings.languageHint") }}</q-item-label>
          </q-item-section>
          <q-item-section side>
            <q-select :model-value="locale" :options="languageOptions" emit-value map-options dense outlined options-dense color="primary" style="min-width: 110px" @update:model-value="setLanguage" />
          </q-item-section>
        </q-item>
      </q-list>
    </div>

    <q-dialog v-model="passwordDialog">
      <q-card class="password-card" :dark="$q.dark.isActive">
        <q-card-section><div class="text-h6">{{ t("settings.changePassword") }}</div></q-card-section>
        <q-card-section class="q-gutter-md">
          <q-input v-model="oldPassword" type="password" filled :label="t('settings.oldPassword')" />
          <q-input v-model="newPassword" type="password" filled :label="t('settings.newPassword')" />
          <q-input v-model="confirmPassword" type="password" filled :label="t('settings.confirmPassword')" />
          <div class="text-caption text-grey-6">{{ t("settings.passwordWarning") }}</div>
        </q-card-section>
        <q-card-actions align="right">
          <q-btn flat :label="t('common.cancel')" v-close-popup />
          <q-btn color="primary" :label="t('common.save')" :loading="changingPassword" @click="changePassword" />
        </q-card-actions>
      </q-card>
    </q-dialog>
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
