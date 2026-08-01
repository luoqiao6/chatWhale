<script setup lang="ts">
import { ref } from "vue";

defineProps<{
  isLoading: boolean;
  model?: string;
  agentMode: boolean;
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
  toggleAgent: [];
}>();

const MAX_FILE_SIZE = 10 * 1024 * 1024; // 10 MB
const inputText = ref("");
const attachedFile = ref<{ name: string; content: string } | null>(null);
const thinkingEnabled = ref(true);
const effort = ref<"high" | "max">("high");
const temperature = ref(1.0);
const maxTokens = ref(4096);
const fileInputRef = ref<HTMLInputElement | null>(null);
const textareaRef = ref<HTMLTextAreaElement | null>(null);

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    sendMessage();
  }
}

function sendMessage() {
  let content = inputText.value.trim();
  if (!content && !attachedFile.value) return;

  if (attachedFile.value) {
    const prefix = `[文件: ${attachedFile.value.name}]\n\`\`\`\n${attachedFile.value.content}\n\`\`\`\n\n`;
    content = prefix + content;
  }

  if (!content.trim() && attachedFile.value) {
    content = `请分析以下文件内容:\n\n\`\`\`\n${attachedFile.value.content}\n\`\`\``;
  }

  emit("send", {
    content,
    thinkingEnabled: thinkingEnabled.value,
    effort: effort.value,
    temperature: temperature.value,
    maxTokens: maxTokens.value,
  });

  inputText.value = "";
  attachedFile.value = null;
  if (textareaRef.value) {
    textareaRef.value.style.height = "auto";
  }
}

function triggerFileInput() {
  fileInputRef.value?.click();
}

function handleFileChange(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;

  if (file.size > MAX_FILE_SIZE) {
    alert(`文件大小不能超过 10 MB。当前文件: ${(file.size / 1024 / 1024).toFixed(2)} MB`);
    input.value = "";
    return;
  }

  const textExts = [".txt", ".md", ".json", ".xml", ".csv", ".tsv", ".yaml", ".yml", ".toml",
    ".js", ".ts", ".jsx", ".tsx", ".py", ".rs", ".go", ".java", ".c", ".cpp", ".h", ".hpp",
    ".css", ".html", ".scss", ".less", ".sql", ".sh", ".bash", ".zsh", ".rb", ".php",
    ".swift", ".kt", ".scala", ".r", ".lua", ".cfg", ".ini", ".conf", ".env", ".gitignore", ".log"];
  const ext = "." + file.name.split(".").pop()?.toLowerCase();

  if (!textExts.includes(ext)) {
    alert(`不支持的文件类型: ${ext}。支持的类型: 文本、代码、配置文件等`);
    input.value = "";
    return;
  }

  const reader = new FileReader();
  reader.onload = () => {
    attachedFile.value = { name: file.name, content: reader.result as string };
  };
  reader.readAsText(file);
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
          <input
            ref="fileInputRef"
            type="file"
            style="display:none"
            @change="handleFileChange"
            accept=".txt,.md,.json,.xml,.csv,.tsv,.yaml,.yml,.toml,.js,.ts,.jsx,.tsx,.py,.rs,.go,.java,.c,.cpp,.h,.hpp,.css,.html,.scss,.sql,.sh,.rb,.php,.swift,.kt,.r,.lua,.cfg,.ini,.conf,.log"
          />
          <button class="btn-input" title="附加文件" @click="triggerFileInput">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48" />
            </svg>
          </button>
          <div v-if="attachedFile" class="file-badge" :title="attachedFile.name">
            <span class="file-badge-icon">📄</span>
            <span class="file-badge-name">{{ attachedFile.name.length > 20 ? attachedFile.name.slice(0, 17) + '...' : attachedFile.name }}</span>
            <button class="file-remove" @click="attachedFile = null">✕</button>
          </div>
          <button
            class="btn-input"
            :class="{ active: agentMode }"
            :title="agentMode ? 'Agent 模式（工具调用回路）' : '普通模式'"
            @click="emit('toggleAgent')"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <path d="M12 2a3 3 0 0 1 3 3v1h3a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h3V5a3 3 0 0 1 3-3z"/>
              <circle cx="12" cy="13" r="2"/>
              <path d="M12 15v3"/>
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
.btn-input.active { color: var(--accent); background: var(--accent-bg); }
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

.file-badge {
  display: flex; align-items: center; gap: 4px;
  padding: 3px 8px; border-radius: var(--radius-sm);
  background: var(--accent-bg); border: 1px solid var(--border);
  font-size: 11px; color: var(--text-secondary); max-width: 180px;
  white-space: nowrap; cursor: default;
}
.file-badge-icon { font-size: 12px; flex-shrink: 0; }
.file-badge-name { overflow: hidden; text-overflow: ellipsis; }
.file-remove {
  border: none; background: transparent; color: var(--text-muted);
  cursor: pointer; font-size: 11px; padding: 0 2px; line-height: 1;
  flex-shrink: 0;
}
.file-remove:hover { color: var(--text-primary); }
</style>
