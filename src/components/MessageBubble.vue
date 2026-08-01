<script setup lang="ts">
import { ref } from "vue";
import { renderMarkdown } from "../composables/useMarkdown";
import type { Message } from "../types";

const props = defineProps<{
  message: Message;
  isLast: boolean;
  toolSources?: Record<string, string>;
  toolResults?: Record<string, string>;
}>();

const thinkingOpen = ref(true);
const toolCallOpen = ref<string[]>([]);

function toggleThinking() {
  thinkingOpen.value = !thinkingOpen.value;
}

function toggleToolCall(id: string) {
  const idx = toolCallOpen.value.indexOf(id);
  if (idx >= 0) {
    toolCallOpen.value.splice(idx, 1);
  } else {
    toolCallOpen.value.push(id);
  }
}

function getRenderedContent(content: string | null): string {
  if (!content) return "";
  return renderMarkdown(content);
}
</script>

<template>
  <!-- User message -->
  <div v-if="message.role === 'user'" class="msg msg-user">
    <div class="msg-bubble">{{ message.content }}</div>
  </div>

  <!-- Assistant message -->
  <div v-else-if="message.role === 'assistant'" class="msg msg-assistant">
    <div class="msg-role">
      <span class="msg-role-icon">🐋</span> chatWhale
    </div>

    <!-- Thinking Panel -->
    <div
      v-if="message.reasoning_content"
      class="thinking-panel"
      :class="{ open: thinkingOpen }"
    >
      <div class="thinking-header" @click="toggleThinking">
        <span class="thinking-dot"></span>
        <span class="thinking-label">深度思考</span>
        <span class="thinking-chevron">▶</span>
      </div>
      <div class="thinking-body">{{ message.reasoning_content }}</div>
    </div>

    <!-- Tool Call Cards -->
    <div
      v-for="tc in message.tool_calls"
      :key="tc.id"
      class="tool-call"
      :class="{ open: toolCallOpen.includes(tc.id) }"
    >
      <div class="tool-call-header" @click="toggleToolCall(tc.id)">
        <svg class="tool-call-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>
        </svg>
        <span class="tool-fn-name">{{ tc.function.name }}</span>
        <span class="tool-source-badge">{{ toolSources?.[tc.id] ?? "builtin" }}</span>
        <span class="tool-status-text">
          {{ toolResults?.[tc.id] !== undefined ? "→ 完成" : "" }}
        </span>
        <span class="tool-chevron">▶</span>
      </div>
      <div class="tool-call-body">
        <div class="tool-section-label">调用参数</div>
        <div class="tool-json">{{ tc.function.arguments }}</div>
        <template v-if="toolResults?.[tc.id] !== undefined">
          <div class="tool-section-label">结果</div>
          <pre class="tool-result">{{ toolResults?.[tc.id] }}</pre>
        </template>
      </div>
    </div>

    <!-- Message content with Markdown -->
    <div v-if="message.content" class="msg-content" v-html="getRenderedContent(message.content)"></div>

    <!-- Loading indicator -->
    <div v-if="isLast && !message.content && !message.reasoning_content && !message.tool_calls?.length" class="loading-dots">
      <span></span><span></span><span></span>
    </div>
  </div>
</template>

<style scoped>
.msg { display: flex; }
.msg-user { display: flex; justify-content: flex-end; }
.msg-user .msg-bubble {
  background: var(--user-bubble); border: 1px solid var(--user-border);
  border-radius: var(--radius-lg); padding: 12px 16px; max-width: 80%;
  font-size: 14px; line-height: 1.6;
}
.msg-assistant { display: flex; flex-direction: column; gap: 10px; }
.msg-role {
  font-size: 12px; font-weight: 600; color: var(--text-muted);
  display: flex; align-items: center; gap: 6px;
}
.msg-role-icon {
  width: 20px; height: 20px; border-radius: 50%;
  background: var(--accent); opacity: 0.8;
  display: flex; align-items: center; justify-content: center; font-size: 10px;
}

/* Message content */
.msg-content { font-size: 14px; line-height: 1.7; color: var(--text-primary); }
.msg-content :deep(p) { margin-bottom: 10px; }
.msg-content :deep(p:last-child) { margin-bottom: 0; }
.msg-content :deep(strong) { color: var(--text-primary); font-weight: 600; }
.msg-content :deep(ul), .msg-content :deep(ol) { padding-left: 20px; margin-bottom: 10px; }
.msg-content :deep(li) { margin-bottom: 4px; }
.msg-content :deep(code:not(pre code)) {
  background: var(--bg-code); padding: 2px 6px; border-radius: 4px;
  font-family: var(--font-mono); font-size: 13px; color: var(--accent);
}
.msg-content :deep(pre) {
  background: var(--bg-code); border: 1px solid var(--code-border);
  border-radius: var(--radius); padding: 16px; overflow-x: auto;
  margin-bottom: 10px; position: relative;
}
.msg-content :deep(pre code) {
  font-family: var(--font-mono); font-size: 13px; line-height: 1.6; color: var(--code-text);
}
.msg-content :deep(.code-lang-tag) {
  position: absolute; top: 8px; right: 12px;
  font-size: 11px; color: var(--text-muted); font-family: var(--font-mono);
}
.msg-content :deep(.code-copy-btn) {
  position: absolute; top: 6px; right: 60px;
  width: 28px; height: 28px; border-radius: 4px; border: none;
  background: var(--bg-hover); color: var(--text-muted); cursor: pointer;
  display: flex; align-items: center; justify-content: center; font-size: 12px;
}
.msg-content :deep(.code-copy-btn:hover) { background: var(--border-active); color: var(--text-primary); }
.msg-content :deep(table) {
  border-collapse: collapse; width: 100%; margin-bottom: 10px;
  font-size: 13px; border-radius: var(--radius-sm); overflow: hidden;
}
.msg-content :deep(th) {
  background: var(--bg-code); padding: 8px 14px; text-align: left;
  font-weight: 600; border-bottom: 1px solid var(--code-border);
}
.msg-content :deep(td) {
  padding: 8px 14px; border-bottom: 1px solid var(--border);
}
.msg-content :deep(tr:last-child td) { border-bottom: none; }

/* Thinking Panel */
.thinking-panel {
  border: 1px solid var(--thinking-border); border-radius: var(--radius);
  background: var(--bg-thinking); overflow: hidden;
}
.thinking-header {
  padding: 8px 14px; display: flex; align-items: center; gap: 8px;
  cursor: pointer; font-size: 12px; color: var(--thinking-color);
}
.thinking-header:hover { background: var(--thinking-bg); }
.thinking-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--thinking-color); }
.thinking-label { font-weight: 600; flex: 1; }
.thinking-chevron { transition: transform 0.2s; font-size: 10px; }
.thinking-panel.open .thinking-chevron { transform: rotate(90deg); }
.thinking-body {
  display: none; padding: 10px 14px 12px; border-top: 1px solid var(--thinking-border);
  font-size: 13px; color: var(--thinking-color); line-height: 1.6; font-style: italic; opacity: 0.85;
}
.thinking-panel.open .thinking-body { display: block; }

/* Tool Call */
.tool-call {
  border: 1px solid var(--tool-border); border-radius: var(--radius);
  background: var(--bg-tool); overflow: hidden;
}
.tool-call-header {
  padding: 8px 14px; display: flex; align-items: center; gap: 8px;
  cursor: pointer; font-size: 12px; color: var(--tool-color);
}
.tool-call-header:hover { background: var(--tool-bg); }
.tool-call-icon { width: 16px; height: 16px; opacity: 0.7; }
.tool-fn-name { font-weight: 600; font-family: var(--font-mono); }
.tool-source-badge {
  font-size: 10px; padding: 1px 6px; border-radius: 100px;
  background: var(--accent-bg); color: var(--accent);
}
.tool-status-text { font-size: 11px; color: var(--text-muted); margin-left: auto; }
.tool-chevron { transition: transform 0.2s; font-size: 10px; margin-left: auto; }
.tool-call.open .tool-chevron { transform: rotate(90deg); }
.tool-call-body { display: none; padding: 10px 14px 12px; border-top: 1px solid var(--tool-border); }
.tool-call.open .tool-call-body { display: block; }
.tool-section-label {
  font-size: 11px; font-weight: 600; color: var(--text-muted);
  margin-bottom: 6px; text-transform: uppercase; letter-spacing: 0.5px;
}
.tool-json {
  font-family: var(--font-mono); font-size: 12px; background: rgba(0,0,0,0.15);
  border-radius: var(--radius-sm); padding: 10px 14px; overflow-x: auto; line-height: 1.6;
  color: var(--text-secondary);
}
.tool-result {
  font-family: var(--font-mono); font-size: 12px; background: rgba(0,0,0,0.15);
  border-radius: var(--radius-sm); padding: 10px 14px; overflow-x: auto; line-height: 1.6;
  color: var(--text-secondary); white-space: pre-wrap; word-break: break-all;
  max-height: 240px; overflow-y: auto;
}

/* Loading dots */
.loading-dots {
  display: flex; gap: 4px; padding: 4px 0;
}
.loading-dots span {
  width: 6px; height: 6px; border-radius: 50%;
  background: var(--accent); animation: bounce 1.4s infinite ease-in-out both;
}
.loading-dots span:nth-child(1) { animation-delay: -0.32s; }
.loading-dots span:nth-child(2) { animation-delay: -0.16s; }
@keyframes bounce {
  0%, 80%, 100% { transform: scale(0); }
  40% { transform: scale(1); }
}
</style>
