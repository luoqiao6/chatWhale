import { describe, expect, it } from "vitest";
import {
  WORKSPACE_COLORS,
  validateWorkspaceName,
  workspaceDirName,
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

  it("目录名取路径最后一段", () => {
    expect(workspaceDirName("/Volumes/Data/work/chatWhale")).toBe("chatWhale");
    expect(workspaceDirName("C:\\Users\\foo\\project")).toBe("project");
    expect(workspaceDirName("/a/b/")).toBe("b");
    expect(workspaceDirName("project")).toBe("project");
  });

  it("根路径与空路径边界", () => {
    expect(workspaceDirName("/")).toBe("/");
    expect(workspaceDirName("")).toBe("未配置目录");
  });

  it("空间名校验拒绝空白", () => {
    expect(validateWorkspaceName("  ")).toBe(false);
    expect(validateWorkspaceName("项目A")).toBe(true);
  });
});
