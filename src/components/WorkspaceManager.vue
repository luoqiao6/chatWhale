<script setup lang="ts">
import { ref } from "vue";
import type { WorkspaceSummary } from "../types";
import { formatWorkspacePath, validateWorkspaceName } from "../composables/workspaceUi";

const props = defineProps<{
  workspaces: WorkspaceSummary[];
  currentId: string;
}>();

const emit = defineEmits<{
  close: [];
  refresh: [];
  openAgentSettings: [workspaceId: string];
}>();

const newName = ref("");
const newPath = ref("");
const copyFrom = ref<string | null>(null);
const errorMsg = ref("");
const deleting = ref<WorkspaceSummary | null>(null);
const deleteConfirm = ref("");

function copyOptions(): { id: string; label: string }[] {
  const list = [
    { id: "__none__", label: "不复制（使用默认值）" },
    ...props.workspaces
      .filter((w) => !w.archived)
      .map((w) => ({ id: w.id, label: w.name })),
  ];
  return list;
}

async function pickDirectory() {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") newPath.value = dir;
  } catch {
    // 浏览器模式手输路径
  }
}

async function createWorkspace() {
  const name = newName.value.trim();
  if (!validateWorkspaceName(name)) {
    errorMsg.value = "请输入空间名称";
    return;
  }
  errorMsg.value = "";
  const { useWorkspaces } = await import("../composables/useWorkspaces");
  const ws = await useWorkspaces().createWorkspace({
    name,
    path: newPath.value.trim(),
    copyFrom: copyFrom.value === "__none__" ? null : copyFrom.value,
  });
  if (ws) {
    newName.value = "";
    newPath.value = "";
    copyFrom.value = null;
    emit("refresh");
  }
}

async function toggleArchived(w: WorkspaceSummary) {
  const { useWorkspaces } = await import("../composables/useWorkspaces");
  await useWorkspaces().setArchived(w.id, !w.archived);
  emit("refresh");
}

async function rename(w: WorkspaceSummary) {
  const name = window.prompt("重命名工作空间", w.name);
  if (!name || !validateWorkspaceName(name)) return;
  const { useWorkspaces } = await import("../composables/useWorkspaces");
  await useWorkspaces().renameWorkspace(w.id, name.trim());
  emit("refresh");
}

async function confirmDelete() {
  if (!deleting.value) return;
  if (deleteConfirm.value !== deleting.value.name) return;
  const { useWorkspaces } = await import("../composables/useWorkspaces");
  await useWorkspaces().deleteWorkspace(deleting.value.id);
  deleting.value = null;
  deleteConfirm.value = "";
  emit("refresh");
}
</script>

<template>
  <div class="settings-overlay" @click.self="emit('close')">
    <div class="settings-panel">
      <div class="settings-header">
        <h2>工作空间管理</h2>
        <button class="close-btn" @click="emit('close')">✕</button>
      </div>
      <div class="settings-body">
        <div class="settings-section">新建工作空间</div>
        <div class="setting-group">
          <label class="setting-label">名称</label>
          <input v-model="newName" class="setting-input" placeholder="项目名称" />
        </div>
        <div class="setting-group">
          <label class="setting-label">工作目录</label>
          <div class="dir-row">
            <input v-model="newPath" class="setting-input" placeholder="/path/to/project" />
            <button class="btn-secondary" @click="pickDirectory">选择</button>
          </div>
        </div>
        <div class="setting-group">
          <label class="setting-label">复制设置来源</label>
          <select v-model="copyFrom" class="setting-input">
            <option v-for="o in copyOptions()" :key="o.id" :value="o.id">{{ o.label }}</option>
          </select>
        </div>
        <div v-if="errorMsg" class="agent-error">{{ errorMsg }}</div>
        <button class="btn-primary" @click="createWorkspace">创建工作空间</button>

        <div class="settings-section">空间列表</div>
        <div class="ws-list">
          <div v-for="w in workspaces" :key="w.id" class="ws-row">
            <div class="ws-row-main">
              <span class="ws-row-name">{{ w.archived ? "📦 " : "" }}{{ w.name }}</span>
              <span class="ws-row-path">{{ formatWorkspacePath(w.path) }} · {{ w.conversation_count }} 会话</span>
            </div>
            <div class="ws-row-actions">
              <button class="btn-secondary" @click="rename(w)">重命名</button>
              <button class="btn-secondary" @click="emit('openAgentSettings', w.id)">Agent 设置</button>
              <button
                v-if="w.id !== 'default'"
                class="btn-secondary"
                @click="toggleArchived(w)"
              >{{ w.archived ? "恢复" : "归档" }}</button>
              <button
                v-if="w.id !== 'default'"
                class="btn-secondary danger"
                @click="deleting = w; deleteConfirm = ''"
              >彻底删除</button>
            </div>
          </div>
        </div>
      </div>

      <div v-if="deleting" class="mcp-editor-overlay" @click.self="deleting = null">
        <div class="mcp-editor">
          <h3>彻底删除「{{ deleting.name }}」</h3>
          <p class="delete-warn">
            将永久删除该空间的 {{ deleting.conversation_count }} 个会话及其全部设置，且不可恢复。
            请输入空间名称「{{ deleting.name }}」确认：
          </p>
          <input v-model="deleteConfirm" class="setting-input" :placeholder="deleting.name" />
          <div class="approval-actions">
            <button class="btn-primary danger" :disabled="deleteConfirm !== deleting.name" @click="confirmDelete">确认删除</button>
            <button class="btn-secondary" @click="deleting = null">取消</button>
          </div>
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
  width: 620px; max-height: 84vh; background: var(--bg-card);
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
.settings-body { padding: 16px 20px; overflow-y: auto; display: flex; flex-direction: column; gap: 12px; }
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
.dir-row { display: flex; gap: 8px; }
.dir-row .setting-input { flex: 1; }
.btn-primary, .btn-secondary {
  padding: 8px 16px; border-radius: var(--radius-sm); font-size: 13px; cursor: pointer; border: none;
  font-family: var(--font-sans);
}
.btn-primary { background: var(--accent); color: var(--bg-primary); }
.btn-primary:hover { opacity: 0.85; }
.btn-secondary { background: var(--bg-hover); color: var(--text-secondary); }
.btn-secondary:hover { background: var(--border); color: var(--text-primary); }
.danger { background: #e05b5b; color: #fff; }
.agent-error { color: #e05b5b; font-size: 12px; }
.ws-list { display: flex; flex-direction: column; gap: 8px; }
.ws-row {
  border: 1px solid var(--border); border-radius: var(--radius-sm);
  padding: 10px 12px; display: flex; align-items: center; justify-content: space-between; gap: 8px;
}
.ws-row-main { display: flex; flex-direction: column; min-width: 0; }
.ws-row-name { font-size: 13px; font-weight: 600; }
.ws-row-path { font-size: 11px; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ws-row-actions { display: flex; gap: 6px; flex-shrink: 0; }
.mcp-editor-overlay {
  position: fixed; inset: 0; background: rgba(0,0,0,0.5);
  display: flex; align-items: center; justify-content: center; z-index: 110;
}
.mcp-editor {
  width: 460px; background: var(--bg-card); border: 1px solid var(--border);
  border-radius: var(--radius-lg); padding: 20px;
  display: flex; flex-direction: column; gap: 10px;
}
.delete-warn { font-size: 13px; color: var(--text-secondary); }
.approval-actions { display: flex; gap: 8px; margin-top: 10px; }
</style>
