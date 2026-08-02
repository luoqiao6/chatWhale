<script setup lang="ts">
import { ref } from "vue";
import type { ThemeName, Conversation, Workspace } from "../types";
import { THEME_META } from "../types";
import WorkspaceSwitcher from "./WorkspaceSwitcher.vue";
import { workspaceColor } from "../composables/workspaceUi";

defineProps<{
  currentTheme: ThemeName;
  currentConvId: string | null;
  currentModel: string;
  groupedConversations: { label: string; items: Conversation[] }[];
  currentWorkspace: Workspace | null;
  activeWorkspaces: Workspace[];
  archivedWorkspaces: Workspace[];
  isAgentRunning: boolean;
}>();

const emit = defineEmits<{
  switchTheme: [theme: ThemeName];
  selectConversation: [id: string];
  newConversation: [];
  openSettings: [];
  openAgentSettings: [];
  openModelManager: [];
  selectWorkspace: [id: string];
  openWorkspaceManager: [];
  newWorkspace: [];
  moveConversation: [id: string, target: string];
  deleteConversation: [id: string];
}>();

const themeNames: ThemeName[] = ["frost", "morning-dew", "aurora", "dusk", "deep-ocean"];
const openMenuFor = ref<string | null>(null);
</script>

<template>
  <aside class="sidebar">
    <div class="sidebar-brand">
      <div class="sidebar-brand-icon">🐋</div>
      <div class="sidebar-brand-text">chat<span class="accent">Whale</span></div>
      <button class="settings-btn" title="Agent 设置" @click="emit('openAgentSettings')">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <path d="M12 2a3 3 0 0 1 3 3v1h3a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h3V5a3 3 0 0 1 3-3z"/>
          <circle cx="12" cy="13" r="2"/>
          <path d="M12 15v3"/>
        </svg>
      </button>
      <button class="settings-btn" title="设置" @click="emit('openSettings')">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
        </svg>
      </button>
    </div>
    <WorkspaceSwitcher
      :current-workspace="currentWorkspace"
      :active="activeWorkspaces"
      :archived="archivedWorkspaces"
      :is-agent-running="isAgentRunning"
      @select="emit('selectWorkspace', $event)"
      @open-manager="emit('openWorkspaceManager')"
      @new-workspace="emit('newWorkspace')"
    />
    <div class="sidebar-actions">
      <button class="btn-new-chat" @click="emit('newConversation')">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
        </svg>
        新建对话
      </button>
    </div>
    <div class="sidebar-conversations">
      <template v-for="group in groupedConversations" :key="group.label">
        <div class="sidebar-section-title">{{ group.label }}</div>
        <div
          v-for="conv in group.items"
          :key="conv.id"
          class="conv-item"
          :class="{ active: conv.id === currentConvId }"
          @click="emit('selectConversation', conv.id)"
        >
          <span
            class="conv-item-icon"
            :style="{ color: workspaceColor(currentWorkspace?.id ?? 'default') }"
          >●</span>
          <span class="conv-item-text">{{ conv.title }}</span>
          <button
            class="conv-more"
            @click.stop="openMenuFor = openMenuFor === conv.id ? null : conv.id"
          >⋮</button>
          <div v-if="openMenuFor === conv.id" class="conv-menu" @click.stop>
            <div
              v-for="w in activeWorkspaces"
              :key="w.id"
              class="conv-menu-item"
              @click="emit('moveConversation', conv.id, w.id)"
            >移动到「{{ w.name }}」</div>
            <div class="conv-menu-item danger" @click="emit('deleteConversation', conv.id)">删除</div>
          </div>
        </div>
      </template>
      <div v-if="groupedConversations.length === 0" class="no-convs">
        该工作空间暂无对话
      </div>
    </div>
    <div class="sidebar-footer">
      <div class="sidebar-section-title" style="padding:0 2px 6px;">界面主题</div>
      <div class="theme-options">
        <div
          v-for="t in themeNames"
          :key="t"
          class="theme-option"
          :class="{ active: t === currentTheme }"
          :title="`${THEME_META[t].label} · ${THEME_META[t].desc}`"
          @click="emit('switchTheme', t)"
        >
          <span class="theme-swatch" :style="{ background: THEME_META[t].swatch, border: t === 'frost' || t === 'morning-dew' ? '1px solid #ccc' : '1px solid rgba(255,255,255,0.1)' }"></span>
          <span class="theme-label">{{ THEME_META[t].label }}</span>
        </div>
      </div>
      <div class="model-selector" title="点击切换模型" @click="emit('openModelManager')">
        <span class="model-dot"></span>
        <div class="model-info">
          <div class="model-name">{{ currentModel }}</div>
          <div class="model-status">本地 · 运行中</div>
        </div>
        <span class="model-chevron">▾</span>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: var(--sidebar-width); min-width: var(--sidebar-width);
  background: var(--bg-sidebar); border-right: 1px solid var(--border);
  display: flex; flex-direction: column; user-select: none;
}
.sidebar-brand {
  height: var(--header-height); padding: 0 20px; display: flex; align-items: center; gap: 10px;
  border-bottom: 1px solid var(--border);
}
.sidebar-brand-icon {
  width: 32px; height: 32px; border-radius: var(--radius);
  background: linear-gradient(135deg, #4fc3b4 0%, #2e8b80 100%);
  display: flex; align-items: center; justify-content: center; font-size: 18px;
}
.sidebar-brand-text { font-size: 17px; font-weight: 700; letter-spacing: -0.3px; }
.sidebar-brand-text .accent { color: var(--accent); }

.settings-btn {
  margin-left: auto; width: 30px; height: 30px; border-radius: var(--radius-sm); border: none;
  background: transparent; color: var(--text-muted); cursor: pointer;
  display: flex; align-items: center; justify-content: center;
}
.settings-btn:hover { background: var(--bg-hover); color: var(--text-primary); }

.sidebar-actions { padding: 12px 16px; }
.btn-new-chat {
  width: 100%; padding: 9px 14px; border-radius: var(--radius);
  border: 1px dashed var(--border); background: transparent;
  color: var(--text-secondary); font-size: 13px; cursor: pointer;
  display: flex; align-items: center; gap: 8px; font-family: var(--font-sans);
  transition: all 0.15s;
}
.btn-new-chat:hover { background: var(--bg-hover); color: var(--text-primary); border-color: var(--border-active); }

.sidebar-conversations { flex: 1; overflow-y: auto; padding: 4px 12px; }
.sidebar-section-title {
  font-size: 11px; font-weight: 600; text-transform: uppercase;
  letter-spacing: 0.5px; color: var(--text-muted); padding: 8px 8px 4px;
}
.no-convs { text-align: center; padding: 20px; color: var(--text-muted); font-size: 12px; }
.conv-item {
  position: relative;
  padding: 8px 10px; border-radius: var(--radius-sm); cursor: pointer;
  font-size: 13px; color: var(--text-secondary); margin-bottom: 2px;
  display: flex; align-items: center; gap: 8px; white-space: nowrap; overflow: hidden;
  transition: background 0.12s;
}
.conv-item:hover { background: var(--bg-hover); }
.conv-item.active { background: var(--accent-bg); color: var(--accent); }
.conv-item-icon { opacity: 0.4; flex-shrink: 0; }
.conv-item.active .conv-item-icon { opacity: 1; }
.conv-item-text { overflow: hidden; text-overflow: ellipsis; }
.conv-more {
  margin-left: auto; width: 20px; height: 20px; border: none; border-radius: 4px;
  background: transparent; color: var(--text-muted); cursor: pointer; opacity: 0;
  flex-shrink: 0; line-height: 1;
}
.conv-item:hover .conv-more { opacity: 1; }
.conv-menu {
  position: absolute; right: 8px; top: calc(100% - 2px); z-index: 70;
  background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius-sm);
  box-shadow: 0 6px 18px rgba(0,0,0,0.2); padding: 4px; min-width: 150px;
}
.conv-menu-item {
  padding: 6px 10px; border-radius: 4px; font-size: 12px; cursor: pointer;
  color: var(--text-secondary); white-space: nowrap;
}
.conv-menu-item:hover { background: var(--bg-hover); color: var(--text-primary); }
.conv-menu-item.danger { color: #e05b5b; }

.sidebar-footer {
  padding: 12px 16px; border-top: 1px solid var(--border);
  display: flex; flex-direction: column; gap: 8px;
}
.theme-options {
  display: flex; gap: 4px; margin-bottom: 10px;
}
.theme-option {
  display: flex; flex-direction: column; align-items: center; gap: 3px;
  padding: 6px 4px 5px; border-radius: var(--radius-sm); cursor: pointer;
  border: 1px solid transparent; font-size: 10px; color: var(--text-muted);
  transition: all 0.12s; flex: 1; text-align: center;
}
.theme-option:hover { background: var(--bg-hover); }
.theme-option.active { border-color: var(--accent); background: var(--accent-bg); color: var(--accent); }
.theme-swatch {
  width: 24px; height: 16px; border-radius: 3px;
  border: 1px solid rgba(255,255,255,0.1); flex-shrink: 0;
}

.model-selector {
  display: flex; align-items: center; gap: 8px; padding: 8px 10px;
  border-radius: var(--radius-sm); background: var(--bg-card); cursor: pointer;
  font-size: 13px;
}
.model-selector:hover { background: var(--bg-hover); }
.model-dot { width: 7px; height: 7px; border-radius: 50%; background: var(--accent); flex-shrink: 0; }
.model-info { flex: 1; }
.model-name { font-size: 13px; font-weight: 500; }
.model-status { font-size: 11px; color: var(--text-muted); }
.model-chevron { color: var(--text-muted); font-size: 11px; }
</style>
