<script setup>
import { computed, ref } from "vue";
import { useQuasar } from "quasar";
import { avatarColor, initials } from "../utils/avatar";
import { useI18n } from "../i18n";

const $q = useQuasar();
const { t } = useI18n();

const props = defineProps({
  conversation: { type: Object, default: null },
  mobile: { type: Boolean, default: false },
});

const emit = defineEmits(["close", "copy", "delete", "rename"]);

const confirmDelete = ref(false);
const editingName = ref(false);
const draftName = ref("");

function startEdit() {
  draftName.value = props.conversation?.name || "";
  editingName.value = true;
}

async function saveName() {
  const name = draftName.value.trim();
  if (!name) return;
  emit("rename", props.conversation.nodeId, name);
  editingName.value = false;
}

function cancelEdit() {
  editingName.value = false;
}

const displayId = computed(() =>
  props.conversation ? props.conversation.nodeId : ""
);

const shortId = computed(() => {
  const id = displayId.value;
  if (!id) return "";
  return id.slice(0, 16) + "…" + id.slice(-8);
});

</script>

<template>
  <div class="info-panel" :class="{ 'is-mobile': mobile }">
    <div class="info-header">
      <div class="row items-center">
        <q-btn
          v-if="mobile"
          flat
          round
          dense
          icon="arrow_back"
          color="grey-4"
          class="q-mr-sm"
          @click="emit('close')"
        />
        <span class="text-subtitle2">{{ t("info.title") }}</span>
      </div>
      <q-btn v-if="!mobile" flat round dense icon="close" color="grey-5" @click="emit('close')" />
    </div>

    <template v-if="conversation">
      <div class="info-body">
      <div class="info-profile">
        <q-avatar
          :style="{ backgroundColor: avatarColor(conversation.name) }"
          text-color="white"
          size="92px"
          font-size="38px"
        >
          {{ initials(conversation.name) }}
        </q-avatar>
        <div class="info-name">
          <template v-if="editingName">
            <q-input
              v-model="draftName"
              filled
              dense
              autofocus
              :dark="$q.dark.isActive"
              class="rename-input"
              @keyup.enter="saveName"
              @keyup.esc="cancelEdit"
            />
            <div class="row justify-center q-gutter-sm q-mt-sm">
              <q-btn flat dense no-caps size="sm" color="grey-5" :label="t('info.cancel')" @click="cancelEdit" />
              <q-btn flat dense no-caps size="sm" color="primary" :label="t('info.save')" @click="saveName" />
            </div>
          </template>
          <template v-else>
            {{ conversation.name }}
            <q-btn
              flat
              round
              dense
              size="sm"
              icon="edit"
              color="grey-5"
              class="rename-btn"
               :aria-label="t('info.rename')"
              @click="startEdit"
            />
          </template>
        </div>
        <div class="info-status">
          <span class="dot" :class="conversation.online ? 'online' : 'offline'" />
           {{ conversation.online ? t("info.online") : t("info.offline") }}
        </div>
      </div>

      <q-separator :dark="$q.dark.isActive" class="q-my-sm" />

      <div class="info-section">
         <div class="info-label">{{ t("info.nodeId") }}</div>
       <div class="info-value mono" :title="displayId">{{ shortId }}</div>
        <q-btn
          flat
          dense
          no-caps
          size="sm"
          color="primary"
          icon="content_copy"
           :label="t('info.copy')"
          class="q-mt-xs"
          @click="emit('copy', displayId)"
       />
      </div>

      <div class="info-section">
        <q-btn flat dense no-caps :color="verified ? 'positive' : 'primary'" :icon="verified ? 'verified' : 'verified_user'" :label="verified ? t('info.verified') : t('info.markVerified')" @click="toggleVerified" />
      </div>

      <q-separator :dark="$q.dark.isActive" class="q-my-sm" />

      <div class="danger-zone">
        <q-btn
          flat
          no-caps
          color="negative"
          icon="person_remove"
           :label="t('info.delete')"
          class="full-width"
          @click="confirmDelete = true"
        />
      </div>

      <q-dialog v-model="confirmDelete">
        <q-card class="bg-grey-9 text-white" style="min-width: 320px">
          <q-card-section>
             <div class="text-h6">{{ t("info.deleteTitle") }}</div>
            <div class="text-grey-5 q-mt-sm">
               {{ t("info.deleteMessage", { name: conversation.name }) }}
            </div>
          </q-card-section>
          <q-card-actions align="right">
             <q-btn flat :label="t('info.cancel')" v-close-popup />
            <q-btn
              flat
              color="negative"
               :label="t('common.delete')"
              v-close-popup
              @click="emit('delete')"
            />
          </q-card-actions>
        </q-card>
      </q-dialog>
      </div>
    </template>

    <div v-else class="info-empty text-grey-6">
       {{ t("info.empty") }}
    </div>
  </div>
</template>

<style scoped>
.info-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 300px;
  min-width: 300px;
  background: var(--sidebar-bg);
  border-left: 1px solid rgba(255, 255, 255, 0.06);
}

.info-panel.is-mobile {
  width: 100%;
  min-width: 0;
  border-left: none;
}

.info-body {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.info-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 14px;
  padding-top: calc(env(safe-area-inset-top, 0px) + 12px);
  min-height: calc(56px + env(safe-area-inset-top, 0px));
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.info-profile {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 24px 16px 16px;
  gap: 6px;
}

.info-name {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 18px;
  font-weight: 600;
  color: var(--text-primary);
  margin-top: 8px;
}

.rename-btn {
  flex-shrink: 0;
}

.rename-input {
  width: 200px;
}

.info-status {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--text-secondary);
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.dot.online {
  background: var(--positive);
}

.dot.offline {
  background: var(--text-secondary);
}

.info-section {
  padding: 10px 16px;
}

.info-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.4px;
  margin-bottom: 8px;
}

.info-value {
  font-size: 13px;
  color: var(--text-primary);
  word-break: break-all;
}

.mono {
  font-family: "SF Mono", "Cascadia Code", Consolas, monospace;
}

.info-empty {
  padding: 32px 16px;
  text-align: center;
  font-size: 13px;
}

.danger-zone {
  padding: 16px;
  margin-top: auto;
}
</style>
