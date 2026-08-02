import { computed, ref } from "vue";
import type { Conversation } from "../types";
import { DEFAULT_WORKSPACE_ID } from "./useWorkspaces";

const STORAGE_KEY = "chatwhale-conversations";

// Singleton state
const conversations = ref<Conversation[]>([]);

function loadFromStorage(): Conversation[] {
  try {
    const data = localStorage.getItem(STORAGE_KEY);
    const list: Conversation[] = data ? JSON.parse(data) : [];
    let migrated = false;
    for (const c of list) {
      if (!c.workspace_id) {
        c.workspace_id = DEFAULT_WORKSPACE_ID;
        migrated = true;
      }
    }
    if (migrated) localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
    return list;
  } catch {
    return [];
  }
}

function saveToStorage() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(conversations.value));
}

export function groupConversationsByTime(
  convs: Conversation[],
): { label: string; items: Conversation[] }[] {
  const now = Date.now();
  const groups: { label: string; items: Conversation[] }[] = [];
  const today: Conversation[] = [];
  const thisWeek: Conversation[] = [];
  const earlier: Conversation[] = [];
  for (const c of convs) {
    const age = now - c.updated_at;
    if (age < 86400000) today.push(c);
    else if (age < 604800000) thisWeek.push(c);
    else earlier.push(c);
  }
  if (today.length) groups.push({ label: "今天", items: today });
  if (thisWeek.length) groups.push({ label: "本周", items: thisWeek });
  if (earlier.length) groups.push({ label: "更早", items: earlier });
  return groups;
}

export function useConversations() {
  async function loadConversations(workspaceId: string) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      conversations.value = await invoke<Conversation[]>("get_conversations", {
        workspaceId,
      });
    } catch {
      conversations.value = loadFromStorage().filter(
        (c) => c.workspace_id === workspaceId,
      );
    }
  }

  async function createConversation(
    title: string,
    model: string,
    workspaceId: string,
  ): Promise<Conversation> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const conv = await invoke<Conversation>("create_conversation", {
        workspaceId,
        title,
        model,
      });
      conversations.value.unshift(conv);
      return conv;
    } catch {
      const conv: Conversation = {
        id: crypto.randomUUID(),
        title,
        model,
        created_at: Date.now(),
        updated_at: Date.now(),
        messages: "[]",
        workspace_id: workspaceId,
      };
      conversations.value.unshift(conv);
      saveToStorage();
      return conv;
    }
  }

  function updateConversation(
    id: string,
    updates: { title?: string; messages?: string },
  ) {
    const conv = conversations.value.find((c) => c.id === id);
    if (!conv) return;
    if (updates.title !== undefined) conv.title = updates.title;
    if (updates.messages !== undefined) conv.messages = updates.messages;
    conv.updated_at = Date.now();
    saveToStorage();
    // Tauri 模式同步到 SQLite（fire-and-forget，兼容 ChatView 的同步保存回调）
    import("@tauri-apps/api/core")
      .then(({ invoke }) =>
        invoke("update_conversation", {
          id,
          title: updates.title ?? null,
          messages: updates.messages ?? null,
        }),
      )
      .catch(() => {});
  }

  async function deleteConversation(id: string) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("delete_conversation", { id });
    } catch {
      // 浏览器降级
    }
    conversations.value = conversations.value.filter((c) => c.id !== id);
    saveToStorage();
  }

  async function moveConversation(id: string, targetWorkspaceId: string) {
    const conv = conversations.value.find((c) => c.id === id);
    if (!conv) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("move_conversation", { id, workspaceId: targetWorkspaceId });
    } catch {
      // 浏览器降级：必须基于完整存储列表更新，避免只保存当前空间子集导致数据丢失
      const all = loadFromStorage();
      const target = all.find((c) => c.id === id);
      if (target) {
        target.workspace_id = targetWorkspaceId;
        localStorage.setItem(STORAGE_KEY, JSON.stringify(all));
      }
    }
    conv.workspace_id = targetWorkspaceId;
    conversations.value = conversations.value.filter((c) => c.id !== id);
  }

  function getConversation(id: string): Conversation | undefined {
    return conversations.value.find((c) => c.id === id);
  }

  const groupedConversations = computed(() =>
    groupConversationsByTime(conversations.value),
  );

  return {
    conversations,
    groupedConversations,
    loadConversations,
    createConversation,
    updateConversation,
    deleteConversation,
    moveConversation,
    getConversation,
  };
}
