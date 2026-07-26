<script setup lang="ts">
import { ref } from "vue";

defineProps<{
  isLoading: boolean;
}>();

interface SendParams {
  content: string;
  thinkingEnabled: boolean;
  effort: "high" | "max";
  temperature: number;
  maxTokens: number;
}

const emit = defineEmits<{
  send: [params: SendParams];
}>();

const inputText = ref("");
const thinkingEnabled = ref(true);
const effort = ref<"high" | "max">("high");
const temperature = ref(1.0);
const maxTokens = ref(4096);
const textareaRef = ref<HTMLTextAreaElement | null>(null);

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    sendMessage();
  }
}

function sendMessage() {
  const content = inputText.value.trim();
  if (!content) return;
  emit("send", {
    content,
    thinkingEnabled: thinkingEnabled.value,
    effort: effort.value,
    temperature: temperature.value,
    maxTokens: maxTokens.value,
  });
  inputText.value = "";
  if (textareaRef.value) {
    textareaRef.value.style.height = "auto";
  }
}

function autoGrow() {
  const el = textareaRef.value;
  if (el) {
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 200) + "px";
  }
}
</script>

<template>
  <div class="input-area">
    <div class="input-inner">
      <div class="input-params">
        <div class="param-group">
          <label>思考</label>
          <select v-model="thinkingEnabled">
            <option :value="true">enabled</option>
            <option :value="false">disabled</option>
          </select>
        </div>
        <div class="param-group">
          <label>effort</label>
          <select v-model="effort">
            <option value="high">high</option>
            <option value="max">max</option>
          </select>
        </div>
        <div class="param-group">
          <label>temperature</label>
          <input type="range" min="0" max="2" step="0.1" v-model.number="temperature" />
          <span class="param-value">{{ temperature.toFixed(1) }}</span>
        </div>
        <div class="param-group">
          <label>max_tokens</label>
          <input type="number" v-model.number="maxTokens" min="1" max="65536" />
        </div>
      </div>
      <div class="input-row">
        <textarea
          ref="textareaRef"
          class="input-textarea"
          rows="1"
          v-model="inputText"
          placeholder="输入消息，Enter 发送，Shift+Enter 换行..."
          @keydown="handleKeydown"
          @input="autoGrow"
        ></textarea>
        <div class="input-actions">
          <button class="btn-input" title="附加文件">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"/>
            </svg>
          </button>
          <button
            class="btn-send"
            :class="{ disabled: isLoading }"
            :disabled="isLoading"
            title="发送消息"
            @click="sendMessage"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/>
            </svg>
          </button>
        </div>
      </div>
      <div class="input-hint">chatWhale 可能会产生不准确信息，请核实重要内容</div>
    </div>
  </div>
</template>

<style scoped>
.input-area { border-top: 1px solid var(--border); padding: 16px 24px 20px; }
.input-inner { max-width: 780px; margin: 0 auto; }
.input-params {
  display: flex; gap: 12px; margin-bottom: 10px; align-items: center; flex-wrap: wrap;
}
.param-group {
  display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text-muted);
}
.param-group label { white-space: nowrap; }
.param-group select, .param-group input {
  background: var(--bg-input); border: 1px solid var(--border); border-radius: var(--radius-sm);
  color: var(--text-primary); padding: 5px 8px; font-size: 12px; font-family: var(--font-mono);
  outline: none;
}
.param-group select:focus, .param-group input:focus { border-color: var(--border-active); }
.param-group input[type="number"] { width: 60px; }
.param-group input[type="range"] { width: 80px; }
.param-value { font-family: var(--font-mono); font-size: 11px; min-width: 30px; text-align: right; }
.input-row {
  display: flex; gap: 10px; align-items: flex-end;
  background: var(--bg-input); border: 1px solid var(--border);
  border-radius: var(--radius-lg); padding: 8px 8px 8px 16px;
  transition: border-color 0.15s;
}
.input-row:focus-within { border-color: var(--border-active); }
.input-textarea {
  flex: 1; resize: none; border: none; background: transparent;
  color: var(--text-primary); font-size: 14px; line-height: 1.6;
  font-family: var(--font-sans); outline: none; min-height: 24px; max-height: 200px;
  padding: 4px 0; overflow-y: auto;
}
.input-textarea::placeholder { color: var(--text-muted); }
.input-actions { display: flex; gap: 6px; align-items: flex-end; padding-bottom: 2px; }
.btn-input {
  width: 32px; height: 32px; border-radius: var(--radius-sm); border: none;
  background: transparent; color: var(--text-muted); cursor: pointer;
  display: flex; align-items: center; justify-content: center;
}
.btn-input:hover { background: var(--bg-hover); color: var(--text-primary); }
.btn-send {
  width: 32px; height: 32px; border-radius: var(--radius-sm); border: none;
  background: var(--accent); color: var(--bg-primary); cursor: pointer;
  display: flex; align-items: center; justify-content: center;
}
.btn-send:hover { opacity: 0.85; }
.btn-send.disabled { opacity: 0.4; cursor: not-allowed; }
.input-hint {
  font-size: 11px; color: var(--text-muted); text-align: center; margin-top: 8px;
}
</style>
