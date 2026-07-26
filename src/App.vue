<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useConversations } from "./composables/useConversations";
import Sidebar from "./components/Sidebar.vue";
import ChatView from "./components/ChatView.vue";
import Settings from "./components/Settings.vue";
import ModelManager from "./components/ModelManager.vue";
import type { ThemeName } from "./types";

const { groupedConversations, createConversation, conversations } = useConversations();

const currentTheme = ref<ThemeName>("deep-ocean");
const currentConvId = ref<string | null>(null);
const currentModel = ref("deepseek-v4-pro");
const showSettings = ref(false);
const showModelManager = ref(false);

function switchTheme(theme: ThemeName) {
  currentTheme.value = theme;
  document.documentElement.setAttribute("data-theme", theme);
  localStorage.setItem("chatwhale-theme", theme);
}

function selectConversation(id: string) {
  currentConvId.value = id;
}

function newConversation() {
  const conv = createConversation("新对话", currentModel.value);
  currentConvId.value = conv.id;
}

function selectModel(modelId: string) {
  currentModel.value = modelId;
  showModelManager.value = false;
}

onMounted(() => {
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
      @switch-theme="switchTheme"
      @select-conversation="selectConversation"
      @new-conversation="newConversation"
      @open-settings="showSettings = true"
      @open-model-manager="showModelManager = true"
    />
    <ChatView
      :key="currentConvId"
      :conv-id="currentConvId"
      :model="currentModel"
    />
    <Settings v-if="showSettings" @close="showSettings = false" />
    <ModelManager v-if="showModelManager" @close="showModelManager = false" @select-model="selectModel" />
  </div>
</template>

<style scoped>
.app { display: flex; height: 100vh; }
</style>
