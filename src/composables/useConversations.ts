import { ref, computed } from "vue";
import type { Conversation } from "../types";

const STORAGE_KEY = "chatwhale-conversations";

// Singleton state
const conversations = ref<Conversation[]>(loadFromStorage());

function loadFromStorage(): Conversation[] {
  try {
    const data = localStorage.getItem(STORAGE_KEY);
    return data ? JSON.parse(data) : [];
  } catch {
    return [];
  }
}

function saveToStorage() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(conversations.value));
}

export function useConversations() {
  function createConversation(title: string, model: string): Conversation {
    const conv: Conversation = {
      id: crypto.randomUUID(),
      title,
      model,
      created_at: Date.now(),
      updated_at: Date.now(),
      messages: "[]",
      workspace_id: "default",
    };
    conversations.value.unshift(conv);
    saveToStorage();
    return conv;
  }

  function updateConversation(id: string, updates: { title?: string; messages?: string }) {
    const conv = conversations.value.find((c) => c.id === id);
    if (!conv) return;
    if (updates.title !== undefined) conv.title = updates.title;
    if (updates.messages !== undefined) conv.messages = updates.messages;
    conv.updated_at = Date.now();
    saveToStorage();
  }

  function deleteConversation(id: string) {
    conversations.value = conversations.value.filter((c) => c.id !== id);
    saveToStorage();
  }

  function getConversation(id: string): Conversation | undefined {
    return conversations.value.find((c) => c.id === id);
  }

  const groupedConversations = computed(() => {
    const now = Date.now();
    const groups: { label: string; items: Conversation[] }[] = [];
    const today: Conversation[] = [];
    const thisWeek: Conversation[] = [];
    const earlier: Conversation[] = [];

    for (const c of conversations.value) {
      const age = now - c.updated_at;
      if (age < 86400000) {
        today.push(c);
      } else if (age < 604800000) {
        thisWeek.push(c);
      } else {
        earlier.push(c);
      }
    }

    if (today.length) groups.push({ label: "今天", items: today });
    if (thisWeek.length) groups.push({ label: "本周", items: thisWeek });
    if (earlier.length) groups.push({ label: "更早", items: earlier });
    return groups;
  });

  return {
    conversations,
    groupedConversations,
    createConversation,
    updateConversation,
    deleteConversation,
    getConversation,
  };
}
