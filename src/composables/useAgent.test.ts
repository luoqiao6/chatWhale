import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick, ref } from "vue";
import { watchAgentLoading } from "./useAgent";

describe("watchAgentLoading", () => {
  it("resets isLoading to false when agent run finishes", async () => {
    const isLoading = ref(true);
    const isAgentRunning = ref(true);
    watchAgentLoading(isLoading, isAgentRunning);
    isAgentRunning.value = false;
    await nextTick();
    expect(isLoading.value).toBe(false);
  });

  it("keeps isLoading unchanged while agent is running", async () => {
    const isLoading = ref(true);
    const isAgentRunning = ref(true);
    watchAgentLoading(isLoading, isAgentRunning);
    await nextTick();
    expect(isLoading.value).toBe(true);
  });

  it("does not flip isLoading to true when a run starts", async () => {
    const isLoading = ref(false);
    const isAgentRunning = ref(false);
    watchAgentLoading(isLoading, isAgentRunning);
    isAgentRunning.value = true;
    await nextTick();
    expect(isLoading.value).toBe(false);
  });
});

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import { useAgent } from "./useAgent";

describe("useAgent startAgent workspaceId", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("invoke agent_chat 时携带 workspaceId", async () => {
    const messages = ref([]);
    const agent = useAgent(messages as any, () => {});
    invokeMock.mockRejectedValue(new Error("stop"));
    await agent.startAgent({} as any, "w1");
    expect(invokeMock).toHaveBeenCalledWith("agent_chat", {
      params: {},
      workspaceId: "w1",
    });
  });
});
