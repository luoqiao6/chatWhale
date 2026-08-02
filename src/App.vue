<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useConversations } from "./composables/useConversations";
import { useWorkspaces } from "./composables/useWorkspaces";
import Sidebar from "./components/Sidebar.vue";
import ChatView from "./components/ChatView.vue";
import Settings from "./components/Settings.vue";
import ModelManager from "./components/ModelManager.vue";
import AgentSettings from "./components/AgentSettings.vue";
import type { ThemeName } from "./types";

const {
  currentWorkspace,
  activeWorkspaces,
  archivedWorkspaces,
  initWorkspaces,
  switchWorkspace,
} = useWorkspaces();

const {
  groupedConversations,
  conversations,
  loadConversations,
  createConversation,
} = useConversations();

const currentTheme = ref<ThemeName>("deep-ocean");
const currentConvId = ref<string | null>(null);
const currentModel = ref("deepseek-v4-pro");
const showSettings = ref(false);
const showModelManager = ref(false);
const showAgentSettings = ref(false);
const showWorkspaceManager = ref(false);

function switchTheme(theme: ThemeName) {
  currentTheme.value = theme;
  document.documentElement.setAttribute("data-theme", theme);
  localStorage.setItem("chatwhale-theme", theme);
}

function selectConversation(id: string) {
  currentConvId.value = id;
}

async function selectWorkspace(id: string) {
  switchWorkspace(id);
  await loadConversations(id);
  // 会话仍属于目标空间则保留，否则回到空态
  currentConvId.value = conversations.value.some((c) => c.id === currentConvId.value)
    ? currentConvId.value
    : null;
}

async function newConversation() {
  const ws = currentWorkspace.value;
  if (!ws || ws.archived) return;
  const conv = await createConversation("新对话", currentModel.value, ws.id);
  currentConvId.value = conv.id;
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
      :is-agent-running="false"
      @switch-theme="switchTheme"
      @select-conversation="selectConversation"
      @new-conversation="newConversation"
      @open-settings="showSettings = true"
      @open-agent-settings="showAgentSettings = true"
      @open-model-manager="showModelManager = true"
      @select-workspace="selectWorkspace"
      @open-workspace-manager="showWorkspaceManager = true"
    />
    <ChatView
      :key="currentConvId"
      :conv-id="currentConvId"
      :model="currentModel"
      :workspace-id="currentWorkspace?.id ?? 'default'"
    />
    <Settings v-if="showSettings" @close="showSettings = false" />
    <AgentSettings v-if="showAgentSettings" @close="showAgentSettings = false" />
    <ModelManager v-if="showModelManager" @close="showModelManager = false" @select-model="selectModel" />
  </div>
</template>

<style scoped>
.app { display: flex; height: 100vh; }
</style>
