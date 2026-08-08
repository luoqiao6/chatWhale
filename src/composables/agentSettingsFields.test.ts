import { describe, expect, it } from "vitest";
import { SETTING_FIELDS } from "./agentSettingsFields";

describe("agentSettingsFields", () => {
  it("包含浏览器工具相关设置字段", () => {
    const keys = SETTING_FIELDS.map((f) => f.key);
    expect(keys).toContain("agent.browser_enabled");
    expect(keys).toContain("agent.browser_path");
    expect(keys).toContain("agent.browser_approval");
    expect(keys).toContain("agent.browser_content_policy");
    expect(keys).toContain("agent.browser_domain_policy");
  });
});
