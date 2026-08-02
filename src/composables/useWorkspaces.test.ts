import { beforeEach, describe, expect, it, vi } from "vitest";
import { useWorkspaces, buildDefaultWorkspace } from "./useWorkspaces";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockRejectedValue(new Error("no tauri")),
}));

class MemoryStorage implements Storage {
  private map = new Map<string, string>();
  get length() { return this.map.size; }
  clear() { this.map.clear(); }
  getItem(k: string) { return this.map.get(k) ?? null; }
  key(i: number) { return [...this.map.keys()][i] ?? null; }
  removeItem(k: string) { this.map.delete(k); }
  setItem(k: string, v: string) { this.map.set(k, v); }
}

beforeEach(() => {
  (globalThis as any).localStorage = new MemoryStorage();
});

describe("useWorkspaces（浏览器降级）", () => {
  it("initWorkspaces 首次创建默认工作空间并选中", async () => {
    const ws = useWorkspaces();
    await ws.initWorkspaces();
    expect(ws.workspaces.value).toHaveLength(1);
    expect(ws.currentWorkspace.value?.name).toBe("默认工作空间");
  });

  it("switchWorkspace 持久化当前空间并过滤活跃/归档", async () => {
    const ws = useWorkspaces();
    await ws.initWorkspaces();
    ws.workspaces.value.push({
      id: "w1", name: "项目A", path: "/tmp/a",
      archived: false, created_at: 1, updated_at: 1,
    });
    ws.workspaces.value.push({
      id: "w2", name: "项目B", path: "/tmp/b",
      archived: true, created_at: 2, updated_at: 2,
    });
    ws.switchWorkspace("w1");
    expect(ws.currentWorkspace.value?.id).toBe("w1");
    expect(ws.activeWorkspaces.value.map((w) => w.id)).toEqual(["default", "w1"]);
    expect(ws.archivedWorkspaces.value.map((w) => w.id)).toEqual(["w2"]);
    expect((globalThis as any).localStorage.getItem("chatwhale-active-workspace")).toBe("w1");
  });

  it("buildDefaultWorkspace 返回稳定结构", () => {
    const d = buildDefaultWorkspace();
    expect(d.id).toBe("default");
    expect(d.name).toBe("默认工作空间");
    expect(d.archived).toBe(false);
  });
});
