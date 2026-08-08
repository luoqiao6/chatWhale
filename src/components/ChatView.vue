<script setup lang="ts">
import { ref, computed, nextTick, watch, onUnmounted } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useConversations } from "../composables/useConversations";
import { useAgent, watchAgentLoading } from "../composables/useAgent";
import ChatInput from "./ChatInput.vue";
import MessageBubble from "./MessageBubble.vue";
import type { Message, ToolCall } from "../types";

const props = defineProps<{
  convId: string | null;
  model?: string;
  workspaceId: string;
  workspaceArchived: boolean;
}>();

const emit = defineEmits<{
  agentRunningChange: [running: boolean];
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
const agentMode = ref(true);
const toolSources = ref<Record<string, string>>({});

const isTauriEnv =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const assetUrl = (p?: string) => (p ? convertFileSrc(p) : "");

const MAX_LOG_BODY_LENGTH = 500;

function generateRequestId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return Date.now().toString(36) + "-" + Math.random().toString(36).slice(2, 12);
}

function truncateLogBody(body: string): string {
  if (body.length <= MAX_LOG_BODY_LENGTH) return body;
  return body.slice(0, MAX_LOG_BODY_LENGTH) + "…[截断 " + (body.length - MAX_LOG_BODY_LENGTH) + " 字符]";
}

function logRequestFailure(requestId: string, status: number | null, body: string): void {
  console.error("[chat/completions] 请求失败", {
    requestId,
    status,
    responseBody: truncateLogBody(body),
  });
}

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

const {
  isAgentRunning,
  toolStates,
  pendingApproval,
  agentUsage,
  agentError,
  lastReason,
  startAgent,
  cancelAgent,
  approveCommand,
  cleanup,
} = useAgent(messages, saveMessages, (p) => {
  toolSources.value = { ...toolSources.value, [p.id]: p.source };
});
watchAgentLoading(isLoading, isAgentRunning);

watch(isAgentRunning, (running) => {
  emit("agentRunningChange", running);
});

watch(
  () => props.workspaceId,
  () => {
    if (props.convId && messages.value.length > 0) {
      saveMessages();
    }
  },
);

const toolResults = computed(() => {
  const map: Record<string, string> = {};
  for (let i = 0; i < messages.value.length; i++) {
    const m = messages.value[i];
    if (m.role === "assistant" && m.tool_calls) {
      for (const tc of m.tool_calls) {
        const next = messages.value[i + 1];
        if (next?.role === "tool" && next.tool_call_id === tc.id) {
          map[tc.id] = next.content ?? "";
        }
      }
    }
  }
  return map;
});

function toggleAgentMode() {
  if (isAgentRunning.value) return;
  if (props.workspaceArchived) return;
  if (!isTauriEnv) return;
  agentMode.value = !agentMode.value;
}

const shareDone = ref(false);

function shareConversation() {
  if (messages.value.length === 0) return;
  let text = "";
  for (const m of messages.value) {
    if (m.role === "user") {
      text += "\ud83d\udc64 \u7528\u6237:\n" + (m.content ?? "") + "\n\n";
    } else if (m.role === "assistant") {
      text += "\ud83d\udc0b chatWhale:\n" + (m.content ?? "") + "\n\n";
    }
  }
  const ta = document.createElement("textarea");
  ta.value = text;
  ta.style.position = "fixed";
  ta.style.left = "-9999px";
  document.body.appendChild(ta);
  ta.select();
  try {
    document.execCommand("copy");
    shareDone.value = true;
    setTimeout(() => { shareDone.value = false; }, 2000);
  } catch {
    navigator.clipboard?.writeText(text).then(() => {
      shareDone.value = true;
      setTimeout(() => { shareDone.value = false; }, 2000);
    });
  }
  document.body.removeChild(ta);
}

function exportConversation() {
  if (messages.value.length === 0) return;
  const conv = props.convId ? getConversation(props.convId) : null;
  const title = conv?.title ?? "\u5bf9\u8bdd\u5bfc\u51fa";
  let md = "# " + title + "\n\n> \u5bfc\u51fa\u65f6\u95f4: " + new Date().toLocaleString() + "\n\n---\n\n";
  for (const m of messages.value) {
    if (m.role === "user") {
      md += "### \ud83d\udc64 \u7528\u6237\n\n" + (m.content ?? "") + "\n\n";
    } else if (m.role === "assistant") {
      if (m.reasoning_content) {
        md += "> **\u6df1\u5ea6\u601d\u8003**\n> \n> " + (m.reasoning_content ?? "").replace(/\n/g, "\n> ") + "\n\n";
      }
      md += "### \ud83d\udc0b chatWhale\n\n" + (m.content ?? "") + "\n\n";
    }
  }
  const blob = new Blob([md], { type: "text/markdown;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = "chatwhale-" + new Date().toISOString().slice(0, 10) + ".md";
  a.click();
  URL.revokeObjectURL(url);
}

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
    content: "\u4f60\u662f\u4e00\u4e2a\u6709\u5e2e\u52a9\u7684\u52a9\u624b\u3002",
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
  if (props.workspaceArchived) return;

  const { baseUrl, apiKey } = getApiConfig();
  if (!apiKey) {
    alert("\u8bf7\u5148\u5728\u8bbe\u7f6e\u4e2d\u914d\u7f6e API Key");
    return;
  }

  if (agentMode.value) {
    const userMsg: Message = { role: "user", content };
    messages.value.push(userMsg);
    scrollToBottom();
    isLoading.value = true;
    await startAgent(
      {
        messages: buildMessages(),
        model: props.model || "deepseek-v4-pro",
        baseUrl,
        apiKey,
        temperature,
        maxTokens,
        thinking: thinkingEnabled ? { type: "enabled" } : { type: "disabled" },
        reasoningEffort: effort,
      },
      props.workspaceId,
    );
    isLoading.value = isAgentRunning.value;
    return;
  }

  const userMsg: Message = { role: "user", content };
  messages.value.push(userMsg);
  scrollToBottom();

  const requestId = generateRequestId();

  const assistantMsg: Message = { role: "assistant", content: null, reasoning_content: null };
  messages.value.push(assistantMsg);
  const assistantIndex = messages.value.length - 1;

  isLoading.value = true;

  try {
    const body: Record<string, unknown> = {
      model: props.model || "deepseek-v4-pro",
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

    const resp = await fetch(baseUrl.replace(/\/$/, "") + "/chat/completions", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: "Bearer " + apiKey,
      },
      body: JSON.stringify(body),
    });

    if (!resp.ok) {
      const errText = await resp.text();
      logRequestFailure(requestId, resp.status, errText);
      messages.value[assistantIndex].content =
        "API \u9519\u8bef (" + resp.status + ") [\u8bf7\u6c42ID: " + requestId + "]: " + errText;
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
        } catch (parseErr) {
          console.warn("[chat/completions] \u5ffd\u7565\u65e0\u6cd5\u89e3\u6790\u7684 SSE \u884c", {
            requestId,
            line: truncateLogBody(line),
          });
        }
      }
    }
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    logRequestFailure(requestId, null, msg);
    if (!messages.value[assistantIndex].content) {
      messages.value[assistantIndex].content = "\u8fde\u63a5\u5931\u8d25 [\u8bf7\u6c42ID: " + requestId + "]: " + msg;
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

onUnmounted(() => {
  cleanup();
});
</script>

<template>
  <main class="main">
    <header class="header">
      <div class="header-title">
        {{ convId ? (getConversation(convId)?.title ?? "对话中...") : "chatWhale" }}
      </div>
      <div class="header-actions">
        <button class="header-btn" :class="{ copied: shareDone }" :title="shareDone ? '已复制！' : '复制对话内容'" @click="shareConversation">
          <svg v-if="shareDone" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
            <polyline points="20 6 9 17 4 12"/>
          </svg>
          <svg v-else width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <circle cx="18" cy="5" r="3"/><circle cx="6" cy="12" r="3"/><circle cx="18" cy="19" r="3"/>
            <line x1="8.59" y1="13.51" x2="15.42" y2="17.49"/><line x1="15.41" y1="6.51" x2="8.59" y2="10.49"/>
          </svg>
        </button>
        <button class="header-btn" title="导出为 Markdown" @click="exportConversation">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
          </svg>
        </button>
      </div>
    </header>

    <div v-if="workspaceArchived" class="archived-banner">
      此工作空间已归档，可查看历史会话；继续对话请恢复工作空间
    </div>

    <div class="chat-area" ref="chatContainer">
      <div class="chat-inner">
        <MessageBubble
          v-for="(msg, idx) in messages"
          :key="idx"
          :message="msg"
          :is-last="idx === messages.length - 1"
          :tool-sources="toolSources"
          :tool-results="toolResults"
        />
        <div v-if="messages.length === 0" class="empty-state">
          <div class="empty-icon">🐋</div>
          <div class="empty-title">开始与 chatWhale 对话</div>
          <div class="empty-desc">先在设置中配置 API Key，然后输入问题开始对话</div>
        </div>
      </div>
    </div>

    <!-- Agent 工具活动面板 -->
    <div v-if="agentMode && Object.keys(toolStates).length" class="tool-activity">
      <div
        v-for="ts in Object.values(toolStates)"
        :key="ts.id"
        class="tool-card"
        :class="ts.status"
      >
        <span v-if="ts.status === 'running'" class="tool-spinner"></span>
        <span v-else class="tool-status-icon">{{ ts.status === "done" ? "✓" : "✕" }}</span>
        <span class="tool-name">{{ ts.name }}</span>
        <span class="tool-source">{{ ts.source }}</span>
        <span v-if="ts.status !== 'running'" class="tool-result-preview">{{ ts.result }}</span>
        <img
          v-if="ts.image_path && ts.status === 'done'"
          :src="assetUrl(ts.image_path)"
          class="tool-thumb"
          alt="browser screenshot"
        />
      </div>
    </div>

    <!-- 命令审批卡片 -->
    <div v-if="pendingApproval" class="approval-card">
      <div class="approval-title">
        命令审批 · {{ pendingApproval.policy }}
      </div>
      <pre class="approval-command">{{ pendingApproval.command }}</pre>
      <div class="approval-actions">
        <template v-if="pendingApproval.choices && pendingApproval.choices.length">
          <button class="btn-approve" @click="approveCommand(pendingApproval.id, true)">允许</button>
          <button
            v-for="c in pendingApproval.choices"
            :key="c.level"
            class="btn-approve"
            @click="approveCommand(pendingApproval.id, true, c.level)"
          >{{ c.label }}</button>
          <button class="btn-reject" @click="approveCommand(pendingApproval.id, false)">拒绝</button>
        </template>
        <template v-else>
          <button class="btn-approve" @click="approveCommand(pendingApproval.id, true)">批准</button>
          <button class="btn-reject" @click="approveCommand(pendingApproval.id, false)">拒绝</button>
        </template>
      </div>
    </div>

    <!-- Agent 状态条 -->
    <div v-if="agentMode" class="agent-status">
      <span v-if="isAgentRunning" class="agent-running">
        <span class="agent-dot"></span> Agent 运行中
        <button class="btn-cancel" @click="cancelAgent">取消</button>
      </span>
      <span v-else-if="lastReason" class="agent-reason">已结束：{{ lastReason }}</span>
      <span v-if="agentUsage" class="agent-usage">
        tokens: {{ agentUsage.total_tokens }}
      </span>
      <span v-if="agentError" class="agent-error">⚠ {{ agentError }}</span>
      <span v-if="!isTauriEnv" class="agent-env-hint">Agent 模式需要桌面运行环境</span>
    </div>

    <ChatInput
      :is-loading="isLoading"
      :agent-mode="agentMode"
      :disabled="workspaceArchived"
      @send="handleSend"
      @toggle-agent="toggleAgentMode"
    />
  </main>
</template>

<style scoped>
.main { flex: 1; display: flex; flex-direction: column; min-width: 0; }

.archived-banner { padding: 8px 24px; background: var(--accent-bg); color: var(--accent); font-size: 12px; }

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
.header-btn.copied { color: var(--accent); }

.chat-area { flex: 1; overflow-y: auto; padding: 24px 0; }
.chat-inner { max-width: 780px; margin: 0 auto; padding: 0 24px; display: flex; flex-direction: column; gap: 24px; }

.empty-state {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  padding: 80px 20px; color: var(--text-muted);
}
.empty-icon { font-size: 48px; margin-bottom: 16px; opacity: 0.6; }
.empty-title { font-size: 16px; font-weight: 600; margin-bottom: 8px; color: var(--text-secondary); }
.empty-desc { font-size: 13px; }

/* Agent 工具活动面板 */
.tool-activity {
  max-width: 780px; margin: 0 auto; padding: 0 24px 12px;
  display: flex; flex-direction: column; gap: 8px;
}
.tool-card {
  display: flex; align-items: center; gap: 8px;
  border: 1px solid var(--tool-border); border-radius: var(--radius);
  background: var(--bg-tool); padding: 8px 14px; font-size: 12px;
  color: var(--tool-color);
}
.tool-card.error { border-color: #e05b5b; }
.tool-spinner {
  width: 12px; height: 12px; border-radius: 50%;
  border: 2px solid var(--border-active); border-top-color: var(--accent);
  animation: spin 0.8s linear infinite;
}
.tool-status-icon { font-weight: 700; }
.tool-name { font-family: var(--font-mono); font-weight: 600; }
.tool-source {
  font-size: 10px; padding: 1px 6px; border-radius: 100px;
  background: var(--accent-bg); color: var(--accent);
}
.tool-result-preview {
  flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  color: var(--text-muted);
}
.tool-thumb {
  max-height: 96px; border-radius: var(--radius-sm);
  border: 1px solid var(--tool-border); object-fit: contain;
}
@keyframes spin { to { transform: rotate(360deg); } }

/* 审批卡片 */
.approval-card {
  max-width: 780px; margin: 0 auto 12px; padding: 12px 16px;
  border: 1px solid var(--border-active); border-radius: var(--radius);
  background: var(--bg-card);
}
.approval-title { font-size: 12px; font-weight: 600; color: var(--text-secondary); margin-bottom: 8px; }
.approval-command {
  font-family: var(--font-mono); font-size: 12px; background: var(--bg-code);
  border-radius: var(--radius-sm); padding: 8px 12px; overflow-x: auto;
  white-space: pre-wrap; word-break: break-all; margin-bottom: 10px;
}
.approval-actions { display: flex; gap: 8px; }
.btn-approve, .btn-reject {
  padding: 6px 18px; border-radius: var(--radius-sm); border: none;
  font-size: 13px; cursor: pointer;
}
.btn-approve { background: var(--accent); color: var(--bg-primary); }
.btn-reject { background: var(--bg-hover); color: var(--text-secondary); }

/* Agent 状态条 */
.agent-status {
  max-width: 780px; margin: 0 auto; padding: 0 24px 8px;
  display: flex; align-items: center; gap: 12px; flex-wrap: wrap;
  font-size: 12px; color: var(--text-muted);
}
.agent-running { display: flex; align-items: center; gap: 6px; color: var(--accent); }
.agent-dot {
  width: 8px; height: 8px; border-radius: 50%; background: var(--accent);
  animation: pulse 1.2s infinite;
}
@keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.35; } }
.btn-cancel {
  padding: 3px 10px; border-radius: var(--radius-sm); border: 1px solid var(--border);
  background: transparent; color: var(--text-secondary); cursor: pointer; font-size: 11px;
}
.btn-cancel:hover { color: var(--text-primary); border-color: var(--border-active); }
.agent-reason { font-family: var(--font-mono); }
.agent-usage { font-family: var(--font-mono); }
.agent-error { color: #e05b5b; }
.agent-env-hint { color: #d4a017; }
</style>
