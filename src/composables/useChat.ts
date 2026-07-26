import { ref } from "vue";
import type { Message, Conversation } from "../types";

export function useChat() {
  const messages = ref<Message[]>([]);
  const conversations = ref<Conversation[]>([]);
  const currentConvId = ref<string | null>(null);
  const isLoading = ref(false);

  function addMessage(msg: Message) {
    messages.value.push(msg);
  }

  function updateLastMessage(content: string) {
    const last = messages.value[messages.value.length - 1];
    if (last && last.role === "assistant") {
      last.content = (last.content ?? "") + content;
    }
  }

  function appendReasoning(content: string) {
    const last = messages.value[messages.value.length - 1];
    if (last && last.role === "assistant") {
      last.reasoning_content = (last.reasoning_content ?? "") + content;
    }
  }

  function clearMessages() {
    messages.value = [];
    currentConvId.value = null;
  }

  async function loadConversations() {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      conversations.value = await invoke<Conversation[]>("get_conversations");
    } catch {
      // Tauri not available, use mock
    }
  }

  async function createConversation(title: string, model: string): Promise<Conversation | null> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const conv = await invoke<Conversation>("create_conversation", { title, model });
      conversations.value.unshift(conv);
      currentConvId.value = conv.id;
      return conv;
    } catch {
      return null;
    }
  }

  async function saveConversation(messagesJson: string) {
    if (!currentConvId.value) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("update_conversation", {
        id: currentConvId.value,
        messages: messagesJson,
      });
    } catch {
      // ignore
    }
  }

  return {
    messages,
    conversations,
    currentConvId,
    isLoading,
    addMessage,
    updateLastMessage,
    appendReasoning,
    clearMessages,
    loadConversations,
    createConversation,
    saveConversation,
  };
}
