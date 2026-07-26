<script setup lang="ts">
import { ref, nextTick, watch } from "vue";
import { useConversations } from "../composables/useConversations";
import ChatInput from "./ChatInput.vue";
import MessageBubble from "./MessageBubble.vue";
import type { Message, ToolCall } from "../types";

const props = defineProps<{
  convId: string | null;
  model?: string;
}>();

const { getConversation, updateConversation } = useConversations();

interface SendParams {
  content: string;
  thinkingEnabled: boolean;
  effort: "high" | "max";
  temperature: number;
  maxTokens: number;
}

const messages = ref<Message[]>([]);
const isLoading = ref(false);
const chatContainer = ref<HTMLElement | null>(null);

function scrollToBottom() {
  nextTick(() => {
    if (chatContainer.value) {
      chatContainer.value.scrollTop = chatContainer.value.scrollHeight;
    }
  });
}

function loadMessagesFromConv(convId: string) {
  const conv = getConversation(convId);
  if (conv) {
    try {
      messages.value = JSON.parse(conv.messages);
    } catch {
      messages.value = [];
    }
  } else {
    messages.value = [];
  }
}

function saveMessages() {
  if (!props.convId) return;
  const conv = getConversation(props.convId);
  if (conv) {
    // Auto-title from first user message
    const firstUser = messages.value.find((m) => m.role === "user");
    const title = conv.title === "新对话" && firstUser
      ? (firstUser.content ?? "").slice(0, 30)
      : conv.title;
    updateConversation(props.convId, {
      title,
      messages: JSON.stringify(messages.value),
    });
  }
}

// Watch convId to load messages when switching conversations
watch(
  () => props.convId,
  (newId) => {
    if (newId) {
      loadMessagesFromConv(newId);
    } else {
      messages.value = [];
    }
  },
  { immediate: true },
);

function getApiConfig() {
  const baseUrl = localStorage.getItem("chatwhale-base-url") || "https://api.deepseek.com";
  const apiKey = localStorage.getItem("chatwhale-api-key") || "";
  return { baseUrl, apiKey };
}

function buildMessages(): Message[] {
  const systemMsg: Message = {
    role: "system",
    content: "你是一个有帮助的助手。",
  };
  const filtered = messages.value.filter((m) => {
    if (m.role === "system") return false;
    if (m.role === "assistant") {
      return m.content != null || (m.tool_calls && m.tool_calls.length > 0);
    }
    return true;
  });
  return [systemMsg, ...filtered];
}

async function handleSend(params: SendParams) {
  const { content, thinkingEnabled, effort, temperature, maxTokens } = params;
  if (!content.trim() || isLoading.value) return;

  const { baseUrl, apiKey } = getApiConfig();
  if (!apiKey) {
    alert("请先在设置中配置 API Key");
    return;
  }

  const userMsg: Message = { role: "user", content };
  messages.value.push(userMsg);
  scrollToBottom();

  const assistantMsg: Message = { role: "assistant", content: null, reasoning_content: null };
  messages.value.push(assistantMsg);
  const assistantIndex = messages.value.length - 1;

  isLoading.value = true;

  try {
    const body: Record<string, unknown> = {
      model: "deepseek-v4-pro",
      messages: buildMessages(),
      stream: true,
      stream_options: { include_usage: true },
      temperature,
      max_tokens: maxTokens,
    };

    if (thinkingEnabled) {
      body.thinking = { type: "enabled" };
      body.reasoning_effort = effort;
    } else {
      body.thinking = { type: "disabled" };
    }

    const resp = await fetch(`${baseUrl.replace(/\/$/, "")}/chat/completions`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${apiKey}`,
      },
      body: JSON.stringify(body),
    });

    if (!resp.ok) {
      const errText = await resp.text();
      messages.value[assistantIndex].content = `API 错误 (${resp.status}): ${errText}`;
      isLoading.value = false;
      return;
    }

    const reader = resp.body?.getReader();
    if (!reader) throw new Error("No response body");

    const decoder = new TextDecoder();
    let buffer = "";
    let contentAccum = "";
    let reasoningAccum = "";
    const toolCallsAccum = new Map<number, { id: string; name: string; arguments: string }>();

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });

      let lineEnd: number;
      while ((lineEnd = buffer.indexOf("\n")) !== -1) {
        const line = buffer.substring(0, lineEnd).trim();
        buffer = buffer.substring(lineEnd + 1);

        if (!line || line.startsWith(":")) continue;
        if (!line.startsWith("data: ")) continue;

        const data = line.substring(6);
        if (data === "[DONE]") {
          if (toolCallsAccum.size > 0) {
            messages.value[assistantIndex].tool_calls = Array.from(toolCallsAccum.values()).map(
              (tc) =>
                ({
                  id: tc.id,
                  type: "function",
                  function: { name: tc.name, arguments: tc.arguments },
                }) as ToolCall,
            );
          }
          saveMessages();
          continue;
        }

        try {
          const chunk = JSON.parse(data);
          const delta = chunk.choices?.[0]?.delta;
          if (!delta) continue;

          if (delta.reasoning_content) {
            reasoningAccum += delta.reasoning_content;
            messages.value[assistantIndex].reasoning_content = reasoningAccum;
          }

          if (delta.content) {
            contentAccum += delta.content;
            messages.value[assistantIndex].content = contentAccum;
          }

          if (delta.tool_calls) {
            for (const tc of delta.tool_calls) {
              const idx = tc.index ?? 0;
              if (!toolCallsAccum.has(idx)) {
                toolCallsAccum.set(idx, { id: "", name: "", arguments: "" });
              }
              const entry = toolCallsAccum.get(idx)!;
              if (tc.id) entry.id = tc.id;
              if (tc.function?.name) entry.name += tc.function.name;
              if (tc.function?.arguments) entry.arguments += tc.function.arguments;
            }
          }

          scrollToBottom();
        } catch {
          // Skip invalid JSON lines
        }
      }
    }
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    if (!messages.value[assistantIndex].content) {
      messages.value[assistantIndex].content = `连接失败: ${msg}`;
    }
  } finally {
    isLoading.value = false;
    scrollToBottom();
  }
}

watch(
  () => messages.value.length,
  () => scrollToBottom(),
);
</script>

<template>
  <main class="main">
    <header class="header">
      <div class="header-title">
        {{ convId ? (getConversation(convId)?.title ?? "对话中...") : "chatWhale" }}
      </div>
      <span class="header-model-badge">deepseek-v4-pro</span>
      <div class="header-actions">
        <button class="header-btn" title="分享对话">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <circle cx="18" cy="5" r="3"/><circle cx="6" cy="12" r="3"/><circle cx="18" cy="19" r="3"/>
            <line x1="8.59" y1="13.51" x2="15.42" y2="17.49"/><line x1="15.41" y1="6.51" x2="8.59" y2="10.49"/>
          </svg>
        </button>
        <button class="header-btn" title="导出对话">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
          </svg>
        </button>
      </div>
    </header>

    <div class="chat-area" ref="chatContainer">
      <div class="chat-inner">
        <MessageBubble
          v-for="(msg, idx) in messages"
          :key="idx"
          :message="msg"
          :is-last="idx === messages.length - 1"
        />
        <div v-if="messages.length === 0" class="empty-state">
          <div class="empty-icon">🐋</div>
          <div class="empty-title">开始与 chatWhale 对话</div>
          <div class="empty-desc">先在设置中配置 API Key，然后输入问题开始对话</div>
        </div>
      </div>
    </div>

    <ChatInput :is-loading="isLoading" @send="handleSend" />
  </main>
</template>

<style scoped>
.main { flex: 1; display: flex; flex-direction: column; min-width: 0; }

.header {
  height: var(--header-height); min-height: var(--header-height);
  border-bottom: 1px solid var(--border); display: flex; align-items: center;
  padding: 0 24px; gap: 12px;
}
.header-title { font-size: 14px; font-weight: 600; flex: 1; }
.header-model-badge {
  font-size: 11px; padding: 3px 10px; border-radius: 100px;
  background: var(--accent-bg); color: var(--accent); font-weight: 500;
}
.header-actions { display: flex; gap: 4px; }
.header-btn {
  width: 32px; height: 32px; border-radius: var(--radius-sm); border: none;
  background: transparent; color: var(--text-secondary); cursor: pointer;
  display: flex; align-items: center; justify-content: center;
}
.header-btn:hover { background: var(--bg-hover); color: var(--text-primary); }

.chat-area { flex: 1; overflow-y: auto; padding: 24px 0; }
.chat-inner { max-width: 780px; margin: 0 auto; padding: 0 24px; display: flex; flex-direction: column; gap: 24px; }

.empty-state {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  padding: 80px 20px; color: var(--text-muted);
}
.empty-icon { font-size: 48px; margin-bottom: 16px; opacity: 0.6; }
.empty-title { font-size: 16px; font-weight: 600; margin-bottom: 8px; color: var(--text-secondary); }
.empty-desc { font-size: 13px; }
</style>
