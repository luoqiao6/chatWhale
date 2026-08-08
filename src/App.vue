<script setup lang="ts">
import { ref, computed, watch, onMounted } from "vue";
import { isConversationEmpty, useConversations } from "./composables/useConversations";
import { useWorkspaces } from "./composables/useWorkspaces";
import Sidebar from "./components/Sidebar.vue";
import ChatView from "./components/ChatView.vue";
import Settings from "./components/Settings.vue";
import ModelManager from "./components/ModelManager.vue";
import AgentSettings from "./components/AgentSettings.vue";
import WorkspaceManager from "./components/WorkspaceManager.vue";
import NewWorkspaceDialog from "./components/NewWorkspaceDialog.vue";
import type { ThemeName, WorkspaceSummary } from "./types";

const {
  workspaces,
  currentWorkspace,
  activeWorkspaces,
  archivedWorkspaces,
  initWorkspaces,
  switchWorkspace,
} = useWorkspaces();

const {
  groupedConversations,
  loadConversations,
  createConversation,
  moveConversation,
  deleteConversation,
  getConversation,
} = useConversations();

const currentTheme = ref<ThemeName>("deep-ocean");
const currentConvId = ref<string | null>(null);
const currentModel = ref("deepseek-v4-pro");
const showSettings = ref(false);
const showModelManager = ref(false);
const showAgentSettings = ref(false);
const showWorkspaceManager = ref(false);
const showNewWorkspace = ref(false);
const agentSettingsWorkspaceId = ref<string | null>(null);
const agentRunning = ref(false);
const pathMissing = ref(false);

const workspaceSummaries = computed(() =>
  workspaces.value.map((w) => ({
    ...w,
    conversation_count: (w as WorkspaceSummary).conversation_count ?? 0,
  })),
);

const agentSettingsWorkspaceName = computed(() => {
  const id = agentSettingsWorkspaceId.value ?? currentWorkspace.value?.id ?? "default";
  return workspaces.value.find((w) => w.id === id)?.name ?? "";
});

watch(
  () => currentWorkspace.value?.path,
  async (p) => {
    if (!p) {
      pathMissing.value = false;
      return;
    }
    try {
      const { exists } = await import("@tauri-apps/plugin-fs");
      pathMissing.value = !(await exists(p));
    } catch {
      pathMissing.value = false;
    }
  },
  { immediate: true },
);

function switchTheme(theme: ThemeName) {
  currentTheme.value = theme;
  document.documentElement.setAttribute("data-theme", theme);
  localStorage.setItem("chatwhale-theme", theme);
}

function selectConversation(id: string) {
  currentConvId.value = id;
}

async function selectWorkspace(id: string) {
  if (id === currentWorkspace.value?.id) return;
  // 离开当前空间前删除空会话（用户未输入任何内容），避免残留"新对话"
  if (currentConvId.value) {
    const conv = getConversation(currentConvId.value);
    if (conv && isConversationEmpty(conv)) {
      await deleteConversation(currentConvId.value);
    }
  }
  switchWorkspace(id);
  await loadConversations(id);
  const ws = currentWorkspace.value;
  if (!ws || ws.archived) {
    currentConvId.value = null;
    return;
  }
  const conv = await createConversation("新对话", currentModel.value, ws.id);
  currentConvId.value = conv.id;
}

async function newConversation() {
  const ws = currentWorkspace.value;
  if (!ws || ws.archived) return;
  const conv = await createConversation("新对话", currentModel.value, ws.id);
  currentConvId.value = conv.id;
}

async function refreshAfterManage() {
  await initWorkspaces();
  await loadConversations(currentWorkspace.value?.id ?? "default");
}

function openAgentSettingsFor(workspaceId: string) {
  agentSettingsWorkspaceId.value = workspaceId;
  showAgentSettings.value = true;
}

function openSidebarAgentSettings() {
  agentSettingsWorkspaceId.value = null;
  showAgentSettings.value = true;
}

function selectModel(modelId: string) {
  currentModel.value = modelId;
  showModelManager.value = false;
}

onMounted(async () => {
  const saved = localStorage.getItem("chatwhale-theme") as ThemeName | null;
  if (saved) {
    currentTheme.value = saved;
    document.documentElement.setAttribute("data-theme", saved);
  } else {
    document.documentElement.setAttribute("data-theme", "deep-ocean");
  }
  // Sync sidebar footer and input area border heights
  syncBorders();
  window.addEventListener("resize", syncBorders);
  await initWorkspaces();
  await loadConversations(currentWorkspace.value?.id ?? "default");
});

function syncBorders() {
  const sf = document.querySelector(".sidebar-footer") as HTMLElement | null;
  const ia = document.querySelector(".input-area") as HTMLElement | null;
  if (!sf || !ia) return;
  sf.style.minHeight = "";
  ia.style.minHeight = "";
  requestAnimationFrame(() => {
    const h = Math.max(sf.offsetHeight, ia.offsetHeight);
    sf.style.minHeight = h + "px";
    ia.style.minHeight = h + "px";
  });
}
</script>

<template>
  <div class="app">
    <Sidebar
      :current-theme="currentTheme"
      :current-conv-id="currentConvId"
      :current-model="currentModel"
      :grouped-conversations="groupedConversations"
      :current-workspace="currentWorkspace"
      :active-workspaces="activeWorkspaces"
      :archived-workspaces="archivedWorkspaces"
      :is-agent-running="agentRunning"
      :path-missing="pathMissing"
      @switch-theme="switchTheme"
      @select-conversation="selectConversation"
      @new-conversation="newConversation"
      @open-settings="showSettings = true"
      @open-agent-settings="openSidebarAgentSettings"
      @open-model-manager="showModelManager = true"
      @select-workspace="selectWorkspace"
      @open-workspace-manager="showWorkspaceManager = true"
      @new-workspace="showNewWorkspace = true"
      @move-conversation="(id, target) => moveConversation(id, target)"
      @delete-conversation="deleteConversation"
    />
    <ChatView
      :key="currentConvId"
      :conv-id="currentConvId"
      :model="currentModel"
      :workspace-id="currentWorkspace?.id ?? 'default'"
      :workspace-archived="currentWorkspace?.archived ?? false"
      @agent-running-change="(v) => (agentRunning = v)"
    />
    <Settings v-if="showSettings" @close="showSettings = false" />
    <AgentSettings
      v-if="showAgentSettings"
      :workspace-id="agentSettingsWorkspaceId ?? currentWorkspace?.id ?? 'default'"
      :workspace-name="agentSettingsWorkspaceName"
      @close="showAgentSettings = false"
    />
    <ModelManager v-if="showModelManager" @close="showModelManager = false" @select-model="selectModel" />
    <WorkspaceManager
      v-if="showWorkspaceManager"
      :workspaces="workspaceSummaries"
      :current-id="currentWorkspace?.id ?? 'default'"
      @close="showWorkspaceManager = false"
      @refresh="refreshAfterManage"
      @open-agent-settings="openAgentSettingsFor"
    />
    <NewWorkspaceDialog
      v-if="showNewWorkspace"
      :workspaces="workspaceSummaries"
      @close="showNewWorkspace = false"
      @refresh="refreshAfterManage"
    />
  </div>
</template>

<style scoped>
.app { display: flex; height: 100vh; }
</style>
