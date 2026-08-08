import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  groupConversationsByTime,
  isConversationEmpty,
  useConversations,
} from "./useConversations";
import type { Conversation } from "../types";

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

function conv(id: string, workspaceId: string, updatedAt: number): Conversation {
  return {
    id, title: "会话", model: "m", created_at: updatedAt,
    updated_at: updatedAt, messages: "[]", workspace_id: workspaceId,
  };
}

beforeEach(() => {
  (globalThis as any).localStorage = new MemoryStorage();
  localStorage.setItem(
    "chatwhale-conversations",
    JSON.stringify([conv("c1", "w1", Date.now()), conv("c2", "w2", Date.now())]),
  );
});

describe("useConversations", () => {
  it("loadConversations 仅加载目标空间会话（浏览器降级）", async () => {
    const c = useConversations();
    await c.loadConversations("w1");
    expect(c.conversations.value.map((x) => x.id)).toEqual(["c1"]);
  });

  it("旧数据缺少 workspace_id 时迁移为 default", async () => {
    localStorage.setItem(
      "chatwhale-conversations",
      JSON.stringify([{ ...conv("c9", "w1", 1), workspace_id: undefined }]),
    );
    const c = useConversations();
    await c.loadConversations("default");
    expect(c.conversations.value[0].workspace_id).toBe("default");
  });

  it("createConversation 绑定当前空间", async () => {
    const c = useConversations();
    const created = await c.createConversation("新对话", "m", "w1");
    expect(created.workspace_id).toBe("w1");
  });

  it("groupConversationsByTime 按时间分组", () => {
    const now = Date.now();
    const groups = groupConversationsByTime([
      conv("a", "w1", now),
      conv("b", "w1", now - 8 * 86400000),
    ]);
    expect(groups[0].label).toBe("今天");
    expect(groups[1].label).toBe("更早");
  });

  it("isConversationEmpty 判定空会话", () => {
    expect(isConversationEmpty(conv("e1", "w1", 1))).toBe(true);
    expect(
      isConversationEmpty({
        ...conv("e2", "w1", 1),
        messages: JSON.stringify([{ role: "user", content: "hi" }]),
      }),
    ).toBe(false);
    expect(
      isConversationEmpty({ ...conv("e3", "w1", 1), messages: "not-json" }),
    ).toBe(true);
  });

  it("moveConversation 改变会话归属（浏览器降级）", async () => {
    const c = useConversations();
    await c.loadConversations("w1");
    await c.moveConversation("c1", "w2");
    await c.loadConversations("w2");
    expect(c.conversations.value.map((x) => x.id)).toContain("c1");
  });

  it("deleteConversation 删除会话且不丢失其他空间数据（浏览器降级）", async () => {
    const c = useConversations();
    await c.loadConversations("w1");
    await c.deleteConversation("c1");
    const all = JSON.parse(localStorage.getItem("chatwhale-conversations") ?? "[]");
    expect(all.map((x: Conversation) => x.id)).toEqual(["c2"]);
    await c.loadConversations("w2");
    expect(c.conversations.value.map((x) => x.id)).toEqual(["c2"]);
  });
});
