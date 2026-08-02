<script setup lang="ts">
import { ref } from "vue";
import type { Workspace, WorkspaceSummary } from "../types";
import { formatWorkspacePath, workspaceColor } from "../composables/workspaceUi";

defineProps<{
  currentWorkspace: Workspace | null;
  active: WorkspaceSummary[];
  archived: WorkspaceSummary[];
  isAgentRunning: boolean;
  pathMissing?: boolean;
}>();

const emit = defineEmits<{
  select: [id: string];
  openManager: [];
  newWorkspace: [];
}>();

const open = ref(false);

function toggle() {
  if (open.value) {
    open.value = false;
    return;
  }
  open.value = true;
}

function pick(id: string) {
  open.value = false;
  emit("select", id);
}
</script>

<template>
  <div class="ws-switcher">
    <button
      class="ws-trigger"
      :disabled="isAgentRunning"
      :title="isAgentRunning ? 'Agent 正在运行，请稍后再切换' : '切换工作空间'"
      @click="toggle"
    >
      <span
        class="ws-dot"
        :style="{ background: workspaceColor(currentWorkspace?.id ?? 'default') }"
      ></span>
      <span class="ws-meta">
        <span class="ws-name">{{ currentWorkspace?.name ?? "未选择工作空间" }}</span>
        <span class="ws-path">
          {{ pathMissing ? "⚠ " : "" }}{{ formatWorkspacePath(currentWorkspace?.path ?? "") }}
        </span>
      </span>
      <span class="ws-chevron">▾</span>
    </button>

    <div v-if="open" class="ws-popover">
      <div class="ws-group-title">工作空间</div>
      <div
        v-for="w in active"
        :key="w.id"
        class="ws-item"
        :class="{ active: w.id === currentWorkspace?.id }"
        @click="pick(w.id)"
      >
        <span class="ws-item-dot" :style="{ background: workspaceColor(w.id) }"></span>
        <span class="ws-item-text">
          <span class="ws-item-name">{{ w.name }}</span>
          <span class="ws-item-path">{{ formatWorkspacePath(w.path) }} · {{ w.conversation_count ?? 0 }} 会话</span>
        </span>
      </div>

      <template v-if="archived.length">
        <div class="ws-group-title">已归档</div>
        <div
          v-for="w in archived"
          :key="w.id"
          class="ws-item"
          @click="pick(w.id)"
        >
          <span class="ws-item-dot" :style="{ background: workspaceColor(w.id) }"></span>
          <span class="ws-item-text">
            <span class="ws-item-name">📦 {{ w.name }}</span>
            <span class="ws-item-path">{{ formatWorkspacePath(w.path) }} · 已归档</span>
          </span>
        </div>
      </template>

      <div class="ws-actions">
        <button class="ws-btn" @click="emit('newWorkspace')">+ 新建工作空间</button>
        <button class="ws-btn" @click="emit('openManager')">管理空间…</button>
      </div>
    </div>

    <div v-if="open" class="ws-backdrop" @click="open = false"></div>
  </div>
</template>

<style scoped>
.ws-switcher { position: relative; padding: 10px 12px 4px; }
.ws-trigger {
  width: 100%; display: flex; align-items: center; gap: 8px;
  padding: 8px 10px; border: 1px solid var(--border); border-radius: var(--radius-sm);
  background: var(--bg-card); color: var(--text-primary); cursor: pointer;
  font-family: var(--font-sans); text-align: left;
}
.ws-trigger:disabled { opacity: 0.5; cursor: not-allowed; }
.ws-trigger:hover:not(:disabled) { border-color: var(--border-active); }
.ws-dot, .ws-item-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
.ws-meta { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.ws-name { font-size: 13px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ws-path { font-size: 11px; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ws-chevron { color: var(--text-muted); font-size: 11px; }
.ws-popover {
  position: absolute; left: 12px; right: 12px; top: calc(100% - 2px); z-index: 60;
  background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius);
  box-shadow: 0 8px 24px rgba(0,0,0,0.25); max-height: 320px; overflow-y: auto;
  padding: 8px;
}
.ws-backdrop { position: fixed; inset: 0; z-index: 55; }
.ws-popover { z-index: 60; }
.ws-group-title {
  font-size: 11px; font-weight: 600; color: var(--text-muted);
  padding: 6px 8px 4px; text-transform: uppercase; letter-spacing: 0.5px;
}
.ws-item {
  display: flex; align-items: center; gap: 8px; padding: 7px 8px;
  border-radius: var(--radius-sm); cursor: pointer;
}
.ws-item:hover { background: var(--bg-hover); }
.ws-item.active { background: var(--accent-bg); }
.ws-item-text { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.ws-item-name { font-size: 13px; color: var(--text-primary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ws-item-path { font-size: 11px; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ws-actions { display: flex; gap: 6px; padding: 8px 4px 4px; border-top: 1px solid var(--border); margin-top: 6px; }
.ws-btn {
  flex: 1; padding: 6px 8px; border: 1px solid var(--border); border-radius: var(--radius-sm);
  background: transparent; color: var(--text-secondary); font-size: 12px; cursor: pointer;
  font-family: var(--font-sans);
}
.ws-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
</style>
