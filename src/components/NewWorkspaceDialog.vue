<script setup lang="ts">
import { ref } from "vue";
import type { WorkspaceSummary } from "../types";
import { validateWorkspaceName } from "../composables/workspaceUi";

const props = defineProps<{
  workspaces: WorkspaceSummary[];
}>();

const emit = defineEmits<{
  close: [];
  refresh: [];
}>();

const newName = ref("");
const newPath = ref("");
const copyFrom = ref<string | null>(null);
const errorMsg = ref("");

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
    emit("close");
  }
}
</script>

<template>
  <div class="settings-overlay" @click.self="emit('close')">
    <div class="settings-panel">
      <div class="settings-header">
        <h2>新建工作空间</h2>
        <button class="close-btn" @click="emit('close')">✕</button>
      </div>
      <div class="settings-body">
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
.agent-error { color: #e05b5b; font-size: 12px; }
</style>
