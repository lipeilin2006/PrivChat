<script setup>
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useQuasar } from "quasar";

const $q = useQuasar();

const emit = defineEmits(["close"]);

const props = defineProps({
  mobile: { type: Boolean, default: false },
});

const peers = ref([]);
const input = ref("");
const adding = ref(false);

onMounted(async () => {
  try {
    peers.value = await invoke("get_mailbox_peers");
  } catch (e) {
    $q.notify({ type: "negative", message: `Failed to load mailbox list: ${e}` });
  }
});

async function addPeer() {
  const v = input.value.trim();
  if (!v) return;
  if (!/^[0-9a-f]{64}$/.test(v)) {
    $q.notify({
      type: "negative",
      message: "Node ID must be 64 hex chars",
      position: "bottom",
    });
    return;
  }
  adding.value = true;
  try {
    await invoke("add_mailbox_peer", { peerId: v });
    input.value = "";
    peers.value = await invoke("get_mailbox_peers");
    $q.notify({ type: "positive", message: "Mailbox added", position: "bottom" });
  } catch (e) {
    $q.notify({ type: "negative", message: `Add failed: ${e}`, position: "bottom" });
  } finally {
    adding.value = false;
  }
}

async function removePeer(peerId) {
  try {
    await invoke("remove_mailbox_peer", { peerId });
    peers.value = peers.value.filter((p) => p !== peerId);
    $q.notify({ type: "positive", message: "Mailbox removed", position: "bottom" });
  } catch (e) {
    $q.notify({ type: "negative", message: `Remove failed: ${e}`, position: "bottom" });
  }
}
</script>

<template>
  <div class="mailbox-settings">
    <div class="mailbox-header">
      <q-btn flat round dense icon="arrow_back" color="grey-4" @click="emit('close')" />
      <span class="text-subtitle2">Mailbox</span>
    </div>

    <div class="mailbox-scroll">
      <div class="mailbox-tip">
        Offline message relays. When you send a message it is encrypted to
        the recipient and uploaded to every configured mailbox, then delivered
        when the recipient comes online.
      </div>

      <q-input
        v-model="input"
        dense
        outlined
        :dark="$q.dark.isActive"
        class="q-px-md q-mt-sm"
        placeholder="Paste a mailbox Node ID (64 hex chars)"
        @keydown.enter="addPeer"
      >
        <template v-slot:append>
          <q-btn
            unelevated
            color="primary"
            label="Add"
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
            <q-btn flat round dense color="negative" icon="delete" @click="removePeer(p)" />
          </q-item-section>
        </q-item>
        <q-item v-if="peers.length === 0">
          <q-item-section>
            <q-item-label caption class="mailbox-empty">No mailboxes configured.</q-item-label>
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