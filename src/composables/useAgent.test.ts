import { describe, expect, it } from "vitest";
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
