<script setup lang="ts">
import { ref, onMounted } from "vue";

interface Model {
  id: string;
  object: string;
  owned_by: string;
}

const models = ref<Model[]>([]);
const loading = ref(false);

const emit = defineEmits<{
  selectModel: [modelId: string];
  close: [];
}>();

async function fetchModels() {
  loading.value = true;
  try {
    const baseUrl = localStorage.getItem("chatwhale-base-url") || "http://localhost:8080/v1";
    const apiKey = localStorage.getItem("chatwhale-api-key") || "";
    const resp = await fetch(`${baseUrl.replace(/\/$/, "")}/models`, {
      headers: { Authorization: `Bearer ${apiKey}` },
    });
    if (resp.ok) {
      const data = await resp.json();
      models.value = data.data || [];
    }
  } catch {
    // Use demo models
    models.value = [
      { id: "deepseek-v4-flash", object: "model", owned_by: "deepseek" },
      { id: "deepseek-v4-pro", object: "model", owned_by: "deepseek" },
    ];
  } finally {
    loading.value = false;
  }
}

onMounted(fetchModels);
</script>

<template>
  <div class="manager-overlay" @click.self="emit('close')">
    <div class="manager-panel">
      <div class="manager-header">
        <h2>模型管理</h2>
        <button class="close-btn" @click="emit('close')">✕</button>
      </div>
      <div class="manager-body">
        <div v-if="loading" class="loading">加载中...</div>
        <div v-else-if="models.length === 0" class="empty">暂无可用模型，请检查 API 连接</div>
        <div
          v-for="model in models"
          :key="model.id"
          class="model-item"
          @click="emit('selectModel', model.id)"
        >
          <span class="model-dot"></span>
          <div class="model-info">
            <div class="model-name">{{ model.id }}</div>
            <div class="model-owner">{{ model.owned_by }}</div>
          </div>
          <span class="select-hint">选择</span>
        </div>
        <button class="refresh-btn" @click="fetchModels">刷新模型列表</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.manager-overlay {
  position: fixed; inset: 0; background: rgba(0,0,0,0.4);
  display: flex; align-items: center; justify-content: center; z-index: 100;
}
.manager-panel {
  width: 440px; max-height: 80vh; background: var(--bg-card);
  border: 1px solid var(--border); border-radius: var(--radius-lg);
  overflow: hidden; display: flex; flex-direction: column;
}
.manager-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 16px 20px; border-bottom: 1px solid var(--border);
}
.manager-header h2 { font-size: 15px; font-weight: 600; }
.close-btn {
  width: 28px; height: 28px; border-radius: var(--radius-sm); border: none;
  background: transparent; color: var(--text-muted); cursor: pointer; font-size: 14px;
}
.close-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
.manager-body { padding: 16px 20px; overflow-y: auto; display: flex; flex-direction: column; gap: 8px; }
.loading, .empty { text-align: center; padding: 20px; color: var(--text-muted); font-size: 13px; }
.model-item {
  display: flex; align-items: center; gap: 10px; padding: 10px 12px;
  border-radius: var(--radius); cursor: pointer; transition: background 0.12s;
  border: 1px solid transparent;
}
.model-item:hover { background: var(--bg-hover); border-color: var(--border); }
.model-dot { width: 7px; height: 7px; border-radius: 50%; background: var(--accent); flex-shrink: 0; }
.model-info { flex: 1; }
.model-name { font-size: 14px; font-weight: 500; }
.model-owner { font-size: 11px; color: var(--text-muted); }
.select-hint { font-size: 12px; color: var(--accent); opacity: 0; transition: opacity 0.12s; }
.model-item:hover .select-hint { opacity: 1; }
.refresh-btn {
  width: 100%; padding: 8px; border-radius: var(--radius-sm); border: 1px solid var(--border);
  background: var(--bg-input); color: var(--text-secondary); cursor: pointer; font-size: 13px;
  margin-top: 8px;
}
.refresh-btn:hover { background: var(--bg-hover); }
</style>
