<script setup lang="ts">
import { ref } from "vue";

const baseUrl = ref("http://localhost:8080/v1");
const apiKey = ref("");
const showKey = ref(false);
const balance = ref<{ is_available: boolean; balance_infos: { currency: string; total_balance: string }[] } | null>(null);

const emit = defineEmits<{
  close: [];
}>();

async function saveSettings() {
  localStorage.setItem("chatwhale-base-url", baseUrl.value);
  localStorage.setItem("chatwhale-api-key", apiKey.value);
}

async function queryBalance() {
  try {
    const resp = await fetch(`${baseUrl.value.replace(/\/$/, "")}/../user/balance`, {
      headers: { Authorization: `Bearer ${apiKey.value}` },
    });
    if (resp.ok) {
      balance.value = await resp.json();
    }
  } catch {
    // ignore
  }
}

// Load saved settings
const saved = localStorage.getItem("chatwhale-base-url");
if (saved) baseUrl.value = saved;
const savedKey = localStorage.getItem("chatwhale-api-key");
if (savedKey) apiKey.value = savedKey;
</script>

<template>
  <div class="settings-overlay" @click.self="emit('close')">
    <div class="settings-panel">
      <div class="settings-header">
        <h2>设置</h2>
        <button class="close-btn" @click="emit('close')">✕</button>
      </div>
      <div class="settings-body">
        <div class="setting-group">
          <label class="setting-label">API Base URL</label>
          <input v-model="baseUrl" class="setting-input" placeholder="http://localhost:8080/v1" />
        </div>
        <div class="setting-group">
          <label class="setting-label">API Key</label>
          <div class="key-input-wrap">
            <input
              :type="showKey ? 'text' : 'password'"
              v-model="apiKey"
              class="setting-input"
              placeholder="sk-..."
            />
            <button class="toggle-key" @click="showKey = !showKey">{{ showKey ? "隐藏" : "显示" }}</button>
          </div>
        </div>
        <div class="setting-actions">
          <button class="btn-primary" @click="saveSettings">保存设置</button>
          <button class="btn-secondary" @click="queryBalance">查询余额</button>
        </div>
        <div v-if="balance" class="balance-info">
          <div class="balance-title">账户余额</div>
          <div v-for="info in balance.balance_infos" :key="info.currency" class="balance-row">
            <span>{{ info.currency }}</span>
            <span class="balance-amount">{{ info.total_balance }}</span>
          </div>
          <div class="balance-status" :class="{ available: balance.is_available }">
            {{ balance.is_available ? "✅ 可用" : "❌ 不可用" }}
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
  width: 440px; max-height: 80vh; background: var(--bg-card);
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
.settings-body { padding: 20px; overflow-y: auto; display: flex; flex-direction: column; gap: 16px; }
.setting-group { display: flex; flex-direction: column; gap: 6px; }
.setting-label { font-size: 12px; font-weight: 600; color: var(--text-secondary); }
.setting-input {
  padding: 8px 12px; border: 1px solid var(--border); border-radius: var(--radius-sm);
  background: var(--bg-input); color: var(--text-primary); font-size: 13px;
  font-family: var(--font-mono); outline: none; width: 100%;
}
.setting-input:focus { border-color: var(--border-active); }
.key-input-wrap { display: flex; gap: 8px; }
.key-input-wrap .setting-input { flex: 1; }
.toggle-key {
  font-size: 12px; padding: 0 10px; border-radius: var(--radius-sm);
  border: 1px solid var(--border); background: var(--bg-input);
  color: var(--text-secondary); cursor: pointer; white-space: nowrap;
}
.toggle-key:hover { background: var(--bg-hover); }
.setting-actions { display: flex; gap: 8px; margin-top: 4px; }
.btn-primary, .btn-secondary {
  padding: 8px 16px; border-radius: var(--radius-sm); font-size: 13px; cursor: pointer; border: none;
}
.btn-primary { background: var(--accent); color: var(--bg-primary); }
.btn-primary:hover { opacity: 0.85; }
.btn-secondary { background: var(--bg-hover); color: var(--text-secondary); }
.btn-secondary:hover { background: var(--border); color: var(--text-primary); }
.balance-info {
  background: var(--accent-bg); border: 1px solid var(--accent-bg);
  border-radius: var(--radius); padding: 14px;
}
.balance-title { font-size: 12px; font-weight: 600; color: var(--text-secondary); margin-bottom: 8px; }
.balance-row {
  display: flex; justify-content: space-between; font-size: 14px;
  font-family: var(--font-mono); padding: 4px 0;
}
.balance-amount { color: var(--accent); font-weight: 600; }
.balance-status { font-size: 12px; margin-top: 6px; color: var(--text-muted); }
.balance-status.available { color: var(--accent); }
</style>
