<script setup>
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useQuasar } from "quasar";
import { useI18n } from "../i18n";

const $q = useQuasar();
const { t } = useI18n();

const emit = defineEmits(["close"]);

const props = defineProps({
  mobile: { type: Boolean, default: false },
});

const peers = ref([]);
const input = ref("");
const adding = ref(false);
const writeCount = ref(0);
const savedWriteCount = ref(0);
const savingCount = ref(false);
const pinging = ref("");
const health = ref({});

onMounted(async () => {
  try {
    peers.value = await invoke("get_mailbox_peers");
  } catch (e) {
    $q.notify({ type: "negative", message: t("mailbox.loaded", { error: e }) });
  }
  try {
    writeCount.value = await invoke("get_mailbox_write_count");
    savedWriteCount.value = writeCount.value;
  } catch (e) {
    $q.notify({ type: "negative", message: t("mailbox.countLoaded", { error: e }) });
  }
});

const countOptions = computed(() => {
  const n = Math.max(peers.value.length, writeCount.value);
   const opts = [{ label: t("mailbox.all"), value: 0 }];
  for (let i = 1; i <= n; i++) opts.push({ label: `${i}`, value: i });
  return opts;
});

async function onCountChange(value) {
  if (savingCount.value) return;
  savingCount.value = true;
  try {
    await invoke("set_mailbox_write_count", { count: value });
    savedWriteCount.value = value;
     $q.notify({ type: "positive", message: t("mailbox.countSaved"), position: "bottom" });
  } catch (e) {
    writeCount.value = savedWriteCount.value;
     $q.notify({ type: "negative", message: t("mailbox.saveFailed", { error: e }), position: "bottom" });
  } finally {
    savingCount.value = false;
  }
}

async function addPeer() {
  const v = input.value.trim();
  if (!v) return;
  if (!/^[0-9a-f]{64}$/.test(v)) {
    $q.notify({
      type: "negative",
       message: t("mailbox.nodeId"),
      position: "bottom",
    });
    return;
  }
  adding.value = true;
  try {
    await invoke("add_mailbox_peer", { peerId: v });
    input.value = "";
    peers.value = await invoke("get_mailbox_peers");
     $q.notify({ type: "positive", message: t("mailbox.added"), position: "bottom" });
  } catch (e) {
     $q.notify({ type: "negative", message: t("mailbox.addFailed", { error: e }), position: "bottom" });
  } finally {
    adding.value = false;
  }
}

async function removePeer(peerId) {
  const ok = await $q.dialog({ title: t("mailbox.removeTitle"), message: t("mailbox.removeMessage"), cancel: true, persistent: true });
  if (!ok) return;
  try {
    await invoke("remove_mailbox_peer", { peerId });
    peers.value = peers.value.filter((p) => p !== peerId);
     $q.notify({ type: "positive", message: t("mailbox.removed"), position: "bottom" });
  } catch (e) {
     $q.notify({ type: "negative", message: t("mailbox.removeFailed", { error: e }), position: "bottom" });
  }
}

async function pingPeer(peerId) {
  pinging.value = peerId;
  try {
    const ms = await invoke("ping_mailbox", { peerId });
    health.value[peerId] = { ok: true, ms };
  } catch (e) {
    health.value[peerId] = { ok: false, error: String(e) };
  } finally {
    pinging.value = "";
  }
}
</script>

<template>
  <div class="mailbox-settings">
    <div class="mailbox-header">
      <q-btn flat round dense icon="arrow_back" color="grey-4" @click="emit('close')" />
       <span class="text-subtitle2">{{ t("mailbox.title") }}</span>
    </div>

    <div class="mailbox-scroll">
      <div class="mailbox-tip">
         {{ t("mailbox.tip") }}
      </div>

      <div class="count-row">
         <span class="count-label">{{ t("mailbox.copies") }}</span>
        <q-select
          v-model="writeCount"
          :options="countOptions"
          emit-value
          map-options
          dense
          outlined
          :dark="$q.dark.isActive"
          :disable="savingCount"
          :loading="savingCount"
          class="count-select"
          @update:model-value="onCountChange"
        />
      </div>

      <q-input
        v-model="input"
        dense
        outlined
        :dark="$q.dark.isActive"
        class="q-px-md q-mt-sm"
         :placeholder="t('mailbox.placeholder')"
        @keydown.enter="addPeer"
      >
        <template v-slot:append>
          <q-btn
            unelevated
            color="primary"
             :label="t('common.add')"
            :loading="adding"
            :disable="adding || !input.trim()"
            @click="addPeer"
          />
        </template>
      </q-input>

      <q-list :dark="$q.dark.isActive" bordered separator class="mailbox-list">
        <q-item v-for="p in peers" :key="p">
          <q-item-section>
            <q-item-label class="mailbox-id mono">{{ p.slice(0, 18) }}…{{ p.slice(-6) }}</q-item-label>
            <q-item-label caption class="mailbox-id-full mono">{{ p }}</q-item-label>
          </q-item-section>
           <q-item-section side>
             <q-btn flat round dense icon="network_check" color="primary" :loading="pinging === p" @click="pingPeer(p)" />
             <q-btn flat round dense color="negative" icon="delete" @click="removePeer(p)" />
           </q-item-section>
           <q-item-label caption v-if="health[p]" :class="health[p].ok ? 'text-positive' : 'text-negative'">
             {{ health[p].ok ? `${t("mailbox.online")} · ${health[p].ms} ms` : `${t("mailbox.offline")} · ${health[p].error}` }}
           </q-item-label>
        </q-item>
        <q-item v-if="peers.length === 0">
          <q-item-section>
             <q-item-label caption class="mailbox-empty">{{ t("mailbox.noMailboxes") }}</q-item-label>
          </q-item-section>
        </q-item>
      </q-list>
    </div>
  </div>
</template>

<style scoped>
.mailbox-settings {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  overflow: hidden;
  background: var(--app-bg);
}

.mailbox-header {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  height: calc(56px + env(safe-area-inset-top, 0px));
  padding: 0 8px;
  padding-top: env(safe-area-inset-top, 0px);
  background: var(--sidebar-bg);
}

.mailbox-scroll {
  padding-top: 8px;
  flex: 1;
  overflow-y: auto;
}

.mailbox-tip {
  font-size: 12px;
  color: var(--text-secondary);
  padding: 0 16px;
  line-height: 1.5;
}

.count-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 16px;
  margin-top: 12px;
}

.count-label {
  font-size: 12px;
  color: var(--text-secondary);
  flex-shrink: 0;
}

.count-select {
  flex: 1;
  max-width: 140px;
}

.mailbox-list {
  margin: 12px;
  border-radius: 12px;
  overflow: hidden;
}

.mailbox-id {
  color: var(--text-primary);
  font-size: 13px;
}

.mailbox-id-full {
  word-break: break-all;
  font-size: 10px;
}

.mailbox-empty {
  color: var(--text-secondary);
}

.mono {
  font-family: "SF Mono", "Cascadia Code", Consolas, monospace;
}
</style>
