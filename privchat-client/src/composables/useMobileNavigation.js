import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export function useMobileNavigation() {
  const view = ref("list");
  const activeId = ref(null);
  let depth = 0;

  function apply(nextView, nextActiveId = null) {
    view.value = nextView;
    activeId.value = nextView === "list" ? null : nextActiveId;
  }

  function push(nextView, nextActiveId = activeId.value) {
    history.pushState({ privchatView: nextView, activeId: nextActiveId }, "", window.location.pathname);
    depth += 1;
    apply(nextView, nextActiveId);
  }

  function onPopState(event) {
    depth = Math.max(0, depth - 1);
    const state = event.state || {};
    apply(state.privchatView || "list", state.activeId || null);
  }

  async function back() {
    if (depth > 0) history.back();
    else await invoke("exit_app");
  }

  function replace(nextView, nextActiveId = null) {
    history.replaceState({ privchatView: nextView, activeId: nextActiveId }, "", window.location.pathname);
    apply(nextView, nextActiveId);
  }

  return { view, activeId, push, back, replace, onPopState, canGoBack: () => depth > 0 };
}
