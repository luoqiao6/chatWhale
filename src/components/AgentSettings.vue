<script setup lang="ts">
import { ref, onMounted } from "vue";
import type { McpServerConfig } from "../types";
import { normalizeAgentSettings } from "../composables/useAgentSettings";
import { SETTING_FIELDS } from "../composables/agentSettingsFields";

const emit = defineEmits<{ close: [] }>();

const props = defineProps<{
  workspaceId: string;
  workspaceName: string;
}>();

const settings = ref<Record<string, string>>({});
const mcpServers = ref<McpServerConfig[]>([]);
const editing = ref<McpServerConfig | null>(null);
const editorArgs = ref("[]");
const editorEnv = ref("{}");
const showEditor = ref(false);
const errorMsg = ref("");
const saving = ref(false);

async function load() {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    settings.value = await invoke<Record<string, string>>("get_agent_settings", {
      workspaceId: props.workspaceId,
    });
    mcpServers.value = await invoke<McpServerConfig[]>("list_mcp_servers", {
      workspaceId: props.workspaceId,
    });
  } catch (e) {
    errorMsg.value = String(e);
  }
}

async function save() {
  saving.value = true;
  errorMsg.value = "";
  try {
    // 校验 JSON 字段
    JSON.parse(settings.value["agent.command_whitelist"] || "[]");
    JSON.parse(settings.value["agent.sensitive_paths"] || "[]");
    JSON.parse(settings.value["agent.browser_domain_policy"] || "{}");
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("set_agent_settings", {
      workspaceId: props.workspaceId,
      settings: normalizeAgentSettings(settings.value),
    });
    emit("close");
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    saving.value = false;
  }
}

async function pickDirectory(key: string) {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") {
      settings.value[key] = dir;
    }
  } catch {
    // 浏览器模式不支持目录选择
  }
}

function newServer() {
  editing.value = {
    id: crypto.randomUUID(),
    workspace_id: props.workspaceId,
    name: "",
    command: "",
    args: [],
    env: {},
    cwd: null,
    timeout: 30,
    transport: "stdio",
    enabled: true,
  };
  editorArgs.value = "[]";
  editorEnv.value = "{}";
  showEditor.value = true;
}

function editServer(s: McpServerConfig) {
  editing.value = { ...s, args: [...s.args], env: { ...s.env } };
  editorArgs.value = JSON.stringify(s.args);
  editorEnv.value = JSON.stringify(s.env);
  showEditor.value = true;
}

async function saveServer() {
  if (!editing.value) return;
  errorMsg.value = "";
  try {
    editing.value.args = JSON.parse(editorArgs.value || "[]");
    editing.value.env = JSON.parse(editorEnv.value || "{}");
    const { invoke } = await import("@tauri-apps/api/core");
    const exists = mcpServers.value.some((s) => s.id === editing.value!.id);
    if (exists) {
      await invoke("update_mcp_server", { server: editing.value });
    } else {
      await invoke("add_mcp_server", { server: editing.value });
    }
    showEditor.value = false;
    await load();
  } catch (e) {
    errorMsg.value = String(e);
  }
}

async function removeServer(s: McpServerConfig) {
  if (!confirm(`删除 MCP Server「${s.name}」？`)) return;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("remove_mcp_server", { id: s.id });
    await load();
  } catch (e) {
    errorMsg.value = String(e);
  }
}

function placeholderFor(key: string): string {
  if (key === "agent.command_whitelist") {
    return '[{"prefix":"git status","cwd":"/path"}]';
  }
  if (key === "agent.sensitive_paths") {
    return '["**/secrets/*"]';
  }
  if (key === "agent.browser_domain_policy") {
    return '{"example.com":"trusted","*.foo.com":"normal"}';
  }
  return "";
}

onMounted(load);
</script>

<template>
  <div class="settings-overlay" @click.self="emit('close')">
    <div class="settings-panel agent-settings-panel">
      <div class="settings-header">
        <h2>Agent 设置 · {{ workspaceName }}</h2>
        <button class="close-btn" @click="emit('close')">✕</button>
      </div>
      <div class="settings-body">
        <div v-if="errorMsg" class="agent-error">{{ errorMsg }}</div>

        <div class="settings-section">通用</div>
        <div v-for="f in SETTING_FIELDS" :key="f.key" class="setting-group">
          <label class="setting-label">{{ f.label }}</label>
          <div v-if="f.type === 'text'" class="dir-row">
            <input
              v-model="settings[f.key]"
              class="setting-input"
            />
            <button class="btn-secondary" @click="pickDirectory(f.key)">选择</button>
          </div>
          <select v-else-if="f.type === 'select'" v-model="settings[f.key]" class="setting-input">
            <option v-for="o in f.options" :key="o" :value="o">{{ o }}</option>
          </select>
          <textarea
            v-else-if="f.type === 'textarea'"
            v-model="settings[f.key]"
            class="setting-input"
            rows="3"
            :placeholder="placeholderFor(f.key)"
          ></textarea>
          <input v-else :type="f.type" v-model="settings[f.key]" class="setting-input" />
        </div>

        <div class="settings-section">MCP Servers（stdio，一期）</div>
        <div class="mcp-list">
          <div v-for="s in mcpServers" :key="s.id" class="mcp-item">
            <span class="mcp-name" :class="{ off: !s.enabled }">
              {{ s.name }} <span class="mcp-cmd">({{ s.command }})</span>
            </span>
            <span class="mcp-actions">
              <button class="btn-secondary" @click="editServer(s)">编辑</button>
              <button class="btn-secondary" @click="removeServer(s)">删除</button>
            </span>
          </div>
          <div v-if="mcpServers.length === 0" class="mcp-empty">暂无 MCP Server</div>
        </div>
        <button class="btn-secondary mcp-add" @click="newServer">+ 添加 MCP Server</button>
      </div>
      <div class="settings-footer">
        <button class="btn-primary" :disabled="saving" @click="save">保存设置</button>
      </div>
    </div>

    <div v-if="showEditor && editing" class="mcp-editor-overlay" @click.self="showEditor = false">
      <div class="mcp-editor">
        <h3>{{ mcpServers.some((s) => s.id === editing.id) ? "编辑" : "添加" }} MCP Server</h3>
        <label class="setting-label">名称</label>
        <input v-model="editing.name" class="setting-input" placeholder="my-server" />
        <label class="setting-label">启动命令</label>
        <input v-model="editing.command" class="setting-input" placeholder="npx / python / node ..." />
        <label class="setting-label">参数（JSON 数组）</label>
        <textarea v-model="editorArgs" class="setting-input" rows="2"></textarea>
        <label class="setting-label">环境变量（JSON）</label>
        <textarea v-model="editorEnv" class="setting-input" rows="3"></textarea>
        <label class="setting-label">工作目录（可选）</label>
        <input
          :value="editing.cwd ?? ''"
          class="setting-input"
          placeholder="/path/to/server"
          @input="editing.cwd = ($event.target as HTMLInputElement).value || null"
        />
        <label class="setting-label">调用超时（秒）</label>
        <input v-model.number="editing.timeout" type="number" class="setting-input" />
        <label class="setting-label checkbox-label">
          <input type="checkbox" v-model="editing.enabled" /> 启用
        </label>
        <div class="approval-actions">
          <button class="btn-primary" @click="saveServer">保存</button>
          <button class="btn-secondary" @click="showEditor = false">取消</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-overlay {
  position: fixed; inset: 0; background: rgba(0,0,0,0.4);
  display: flex; align-items: center; justify-content: center; z-index: 100;
}
.settings-panel {
  width: 520px; max-height: 84vh; background: var(--bg-card);
  border: 1px solid var(--border); border-radius: var(--radius-lg);
  overflow: hidden; display: flex; flex-direction: column;
}
.settings-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 16px 20px; border-bottom: 1px solid var(--border);
}
.settings-header h2 { font-size: 15px; font-weight: 600; }
.close-btn {
  width: 28px; height: 28px; border-radius: var(--radius-sm); border: none;
  background: transparent; color: var(--text-muted); cursor: pointer; font-size: 14px;
}
.close-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
.settings-body {
  padding: 16px 20px; overflow-y: auto; display: flex; flex-direction: column; gap: 12px;
}
.settings-section {
  font-size: 12px; font-weight: 700; color: var(--accent);
  margin-top: 8px; text-transform: uppercase; letter-spacing: 0.5px;
}
.setting-group { display: flex; flex-direction: column; gap: 6px; }
.setting-label { font-size: 12px; font-weight: 600; color: var(--text-secondary); }
.setting-input {
  padding: 8px 12px; border: 1px solid var(--border); border-radius: var(--radius-sm);
  background: var(--bg-input); color: var(--text-primary); font-size: 13px;
  font-family: var(--font-mono); outline: none; width: 100%;
}
.setting-input:focus { border-color: var(--border-active); }
.dir-row { display: flex; gap: 8px; }
.dir-row .setting-input { flex: 1; }
.btn-primary, .btn-secondary {
  padding: 8px 16px; border-radius: var(--radius-sm); font-size: 13px; cursor: pointer; border: none;
}
.btn-primary { background: var(--accent); color: var(--bg-primary); }
.btn-primary:hover { opacity: 0.85; }
.btn-secondary { background: var(--bg-hover); color: var(--text-secondary); }
.btn-secondary:hover { background: var(--border); color: var(--text-primary); }
.agent-error { color: #e05b5b; font-size: 12px; }
.mcp-list { display: flex; flex-direction: column; gap: 6px; }
.mcp-item {
  display: flex; align-items: center; justify-content: space-between;
  border: 1px solid var(--border); border-radius: var(--radius-sm);
  padding: 8px 12px; font-size: 13px;
}
.mcp-name.off { opacity: 0.5; }
.mcp-cmd { font-size: 11px; color: var(--text-muted); font-family: var(--font-mono); }
.mcp-actions { display: flex; gap: 6px; }
.mcp-empty { color: var(--text-muted); font-size: 12px; padding: 8px 0; }
.mcp-add { align-self: flex-start; }
.settings-footer {
  padding: 14px 20px; border-top: 1px solid var(--border);
  display: flex; justify-content: flex-end;
}
.checkbox-label { display: flex; align-items: center; gap: 6px; font-weight: 400; }
.mcp-editor-overlay {
  position: fixed; inset: 0; background: rgba(0,0,0,0.5);
  display: flex; align-items: center; justify-content: center; z-index: 110;
}
.mcp-editor {
  width: 440px; max-height: 84vh; overflow-y: auto;
  background: var(--bg-card); border: 1px solid var(--border);
  border-radius: var(--radius-lg); padding: 20px;
  display: flex; flex-direction: column; gap: 8px;
}
.mcp-editor h3 { font-size: 15px; font-weight: 600; margin-bottom: 8px; }
.approval-actions { display: flex; gap: 8px; margin-top: 10px; }
</style>
