import { ref, type Ref } from "vue";
import type {
  AgentChatParams,
  AgentDonePayload,
  AgentUsage,
  ApprovalRequest,
  Message,
  ToolExecution,
} from "../types";

type UnlistenFn = () => void;

interface ToolStartPayload {
  id: string;
  name: string;
  arguments: string;
  source: string;
}

interface ToolResultPayload {
  id: string;
  name: string;
  result: string;
  error?: string | null;
}

export function useAgent(messages: Ref<Message[]>, saveMessages: () => void) {
  const isAgentRunning = ref(false);
  const toolStates = ref<Record<string, ToolExecution>>({});
  const pendingApproval = ref<ApprovalRequest | null>(null);
  const agentUsage = ref<AgentUsage | null>(null);
  const agentError = ref<string | null>(null);
  const lastReason = ref<string>("");
  let unlistenFns: UnlistenFn[] = [];
  let activeAssistantIndex = -1;

  function cleanup() {
    unlistenFns.forEach((fn) => {
      try {
        fn();
      } catch {
        // ignore
      }
    });
    unlistenFns = [];
  }

  function setToolState(id: string, patch: Partial<ToolExecution>) {
    const cur = toolStates.value[id] ?? {
      id,
      name: "",
      arguments: "",
      source: "builtin",
      status: "running" as const,
    };
    toolStates.value = { ...toolStates.value, [id]: { ...cur, ...patch } };
  }

  function ensureActiveAssistant() {
    if (activeAssistantIndex < 0 || activeAssistantIndex >= messages.value.length) {
      messages.value.push({ role: "assistant", content: null, reasoning_content: null });
      activeAssistantIndex = messages.value.length - 1;
    }
    return messages.value[activeAssistantIndex];
  }

  async function startAgent(params: AgentChatParams) {
    cleanup();
    isAgentRunning.value = true;
    agentError.value = null;
    lastReason.value = "";
    toolStates.value = {};
    agentUsage.value = null;
    pendingApproval.value = null;
    activeAssistantIndex = -1;

    const { listen } = await import("@tauri-apps/api/event");
    const { invoke } = await import("@tauri-apps/api/core");

    unlistenFns.push(
      await listen<{ content: string }>("agent-chunk", (e) => {
        const m = ensureActiveAssistant();
        m.content = (m.content ?? "") + e.payload.content;
      }),
    );
    unlistenFns.push(
      await listen<{ content: string }>("agent-reasoning", (e) => {
        const m = ensureActiveAssistant();
        m.reasoning_content = (m.reasoning_content ?? "") + e.payload.content;
      }),
    );
    unlistenFns.push(
      await listen<ToolStartPayload>("agent-tool-start", (e) => {
        const p = e.payload;
        setToolState(p.id, {
          id: p.id,
          name: p.name,
          arguments: p.arguments,
          source: p.source,
          status: "running",
        });
      }),
    );
    unlistenFns.push(
      await listen<ToolResultPayload>("agent-tool-result", (e) => {
        const p = e.payload;
        setToolState(p.id, {
          status: p.error ? "error" : "done",
          result: p.error ?? p.result,
          error: p.error ?? undefined,
        });
      }),
    );
    unlistenFns.push(
      await listen<ApprovalRequest>("agent-approval-request", (e) => {
        pendingApproval.value = e.payload;
      }),
    );
    unlistenFns.push(
      await listen<AgentUsage>("agent-usage", (e) => {
        agentUsage.value = e.payload;
      }),
    );
    unlistenFns.push(
      await listen<AgentDonePayload>("agent-done", (e) => {
        const payload = e.payload;
        lastReason.value = payload.reason;
        messages.value = payload.messages;
        saveMessages();
        isAgentRunning.value = false;
        pendingApproval.value = null;
        toolStates.value = {};
        if (payload.reason === "error" && !agentError.value) {
          agentError.value = "Agent 运行出错，已保留部分进度";
        }
        if (payload.reason === "cancelled") {
          agentError.value = null;
        }
        cleanup();
      }),
    );
    unlistenFns.push(
      await listen<{ message: string }>("agent-error", (e) => {
        agentError.value = e.payload.message;
      }),
    );

    try {
      await invoke("agent_chat", { params });
    } catch (err) {
      agentError.value = String(err);
      isAgentRunning.value = false;
      cleanup();
    }
  }

  async function cancelAgent() {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("agent_cancel");
    } catch {
      // 幂等，忽略
    }
  }

  async function approveCommand(id: string, approved: boolean) {
    pendingApproval.value = null;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("agent_approve", { id, approved });
    } catch (err) {
      agentError.value = String(err);
    }
  }

  return {
    isAgentRunning,
    toolStates,
    pendingApproval,
    agentUsage,
    agentError,
    lastReason,
    startAgent,
    cancelAgent,
    approveCommand,
    cleanup,
  };
}
