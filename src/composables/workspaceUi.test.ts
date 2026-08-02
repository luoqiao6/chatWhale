import { describe, expect, it } from "vitest";
import {
  WORKSPACE_COLORS,
  formatWorkspacePath,
  validateWorkspaceName,
  workspaceColor,
} from "./workspaceUi";

describe("workspaceUi", () => {
  it("默认空间使用固定鲸青色", () => {
    expect(workspaceColor("default")).toBe(WORKSPACE_COLORS[0]);
  });

  it("颜色分配稳定且落在调色板内", () => {
    const a = workspaceColor("w-abc");
    const b = workspaceColor("w-abc");
    expect(a).toBe(b);
    expect(WORKSPACE_COLORS).toContain(a);
  });

  it("空路径显示未配置目录", () => {
    expect(formatWorkspacePath("")).toBe("未配置目录");
    expect(formatWorkspacePath("/tmp/a")).toBe("/tmp/a");
  });

  it("空间名校验拒绝空白", () => {
    expect(validateWorkspaceName("  ")).toBe(false);
    expect(validateWorkspaceName("项目A")).toBe(true);
  });
});
