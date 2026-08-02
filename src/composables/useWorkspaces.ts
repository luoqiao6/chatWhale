import { computed, ref } from "vue";
import type { Workspace, WorkspaceSummary } from "../types";

const STORAGE_KEY = "chatwhale-workspaces";
const ACTIVE_KEY = "chatwhale-active-workspace";
export const DEFAULT_WORKSPACE_ID = "default";

export function buildDefaultWorkspace(): Workspace {
  return {
    id: DEFAULT_WORKSPACE_ID,
    name: "默认工作空间",
    path: "",
    archived: false,
    created_at: Date.now(),
    updated_at: Date.now(),
  };
}

const workspaces = ref<Workspace[]>([]);
const currentWorkspaceId = ref<string>(DEFAULT_WORKSPACE_ID);

export function useWorkspaces() {
  const activeWorkspaces = computed(() => workspaces.value.filter((w) => !w.archived));
  const archivedWorkspaces = computed(() => workspaces.value.filter((w) => w.archived));
  const currentWorkspace = computed(
    () => workspaces.value.find((w) => w.id === currentWorkspaceId.value) ?? null,
  );

  function persistActive() {
    localStorage.setItem(ACTIVE_KEY, currentWorkspaceId.value);
  }

  function saveLocal() {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(workspaces.value));
  }

  function ensureDefault() {
    if (!workspaces.value.some((w) => w.id === DEFAULT_WORKSPACE_ID)) {
      workspaces.value.unshift(buildDefaultWorkspace());
      saveLocal();
    }
  }

  function restoreActive() {
    const saved = localStorage.getItem(ACTIVE_KEY);
    if (saved && workspaces.value.some((w) => w.id === saved)) {
      currentWorkspaceId.value = saved;
    } else {
      currentWorkspaceId.value = DEFAULT_WORKSPACE_ID;
    }
  }

  async function initWorkspaces() {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const list = await invoke<WorkspaceSummary[]>("list_workspaces");
      workspaces.value = list;
      ensureDefault();
      restoreActive();
    } catch {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        try {
          workspaces.value = JSON.parse(raw);
        } catch {
          workspaces.value = [];
        }
      }
      ensureDefault();
      restoreActive();
    }
  }

  function switchWorkspace(id: string) {
    if (!workspaces.value.some((w) => w.id === id)) return;
    currentWorkspaceId.value = id;
    persistActive();
  }

  async function createWorkspace(input: {
    name: string;
    path: string;
    copyFrom: string | null;
  }): Promise<Workspace | null> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const ws = await invoke<Workspace>("create_workspace", {
        name: input.name,
        path: input.path,
        copyFrom: input.copyFrom,
      });
      workspaces.value.push(ws);
      return ws;
    } catch {
      const ws: Workspace = {
        id: crypto.randomUUID(),
        name: input.name,
        path: input.path,
        archived: false,
        created_at: Date.now(),
        updated_at: Date.now(),
      };
      workspaces.value.push(ws);
      saveLocal();
      return ws;
    }
  }

  async function renameWorkspace(id: string, name: string) {
    const ws = workspaces.value.find((w) => w.id === id);
    if (!ws) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("update_workspace", { id, name });
    } catch {
      // 浏览器降级
    }
    ws.name = name;
    ws.updated_at = Date.now();
    saveLocal();
  }

  async function setArchived(id: string, archived: boolean) {
    if (id === DEFAULT_WORKSPACE_ID) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("set_workspace_archived", { id, archived });
    } catch {
      // 浏览器降级
    }
    const ws = workspaces.value.find((w) => w.id === id);
    if (ws) {
      ws.archived = archived;
      ws.updated_at = Date.now();
      saveLocal();
    }
    if (archived && currentWorkspaceId.value === id) {
      switchWorkspace(DEFAULT_WORKSPACE_ID);
    }
  }

  async function deleteWorkspace(id: string) {
    if (id === DEFAULT_WORKSPACE_ID) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("delete_workspace", { id });
    } catch {
      // 浏览器降级
    }
    workspaces.value = workspaces.value.filter((w) => w.id !== id);
    saveLocal();
    if (currentWorkspaceId.value === id) {
      switchWorkspace(DEFAULT_WORKSPACE_ID);
    }
  }

  return {
    workspaces,
    currentWorkspaceId,
    currentWorkspace,
    activeWorkspaces,
    archivedWorkspaces,
    initWorkspaces,
    switchWorkspace,
    createWorkspace,
    renameWorkspace,
    setArchived,
    deleteWorkspace,
  };
}
