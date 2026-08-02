import { describe, expect, it } from "vitest";
import { normalizeAgentSettings } from "./useAgentSettings";

describe("normalizeAgentSettings", () => {
  it("把数字值转成字符串，保证 set_agent_settings 的 HashMap<String,String> 能反序列化", () => {
    const result = normalizeAgentSettings({
      "agent.llm_timeout": 120,
      "agent.max_iterations": 5,
      "agent.workspace_root": "/work",
    });
    expect(result).toEqual({
      "agent.llm_timeout": "120",
      "agent.max_iterations": "5",
      "agent.workspace_root": "/work",
    });
  });

  it("字符串值原样保留", () => {
    const result = normalizeAgentSettings({
      "agent.command_approval": "always",
      "agent.command_whitelist": "[]",
    });
    expect(result).toEqual({
      "agent.command_approval": "always",
      "agent.command_whitelist": "[]",
    });
  });

  it("null/undefined 值转为空字符串，避免 JSON null 导致反序列化失败", () => {
    const result = normalizeAgentSettings({
      "agent.workspace_root": null,
      "agent.skills_dir": undefined,
    });
    expect(result).toEqual({
      "agent.workspace_root": "",
      "agent.skills_dir": "",
    });
  });
});
