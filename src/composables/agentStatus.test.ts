import { describe, expect, it } from "vitest";
import type { Message } from "../types";
import { formatAgentEndReason, mergeDoneMessages } from "./agentStatus";

describe("formatAgentEndReason", () => {
  it("finish_reason=length 时给出可读的截断说明", () => {
    expect(formatAgentEndReason("finish_reason", "length")).toBe(
      "输出达到 max_tokens 上限被截断（length）",
    );
  });

  it("finish_reason=content_filter 时说明内容被拦截", () => {
    expect(formatAgentEndReason("finish_reason", "content_filter")).toContain(
      "内容安全",
    );
  });

  it("finish_reason 未知或缺失时降级为通用文案", () => {
    expect(formatAgentEndReason("finish_reason", "some_other")).toContain(
      "some_other",
    );
    expect(formatAgentEndReason("finish_reason", null)).toBe("非正常结束");
  });

  it("stop / cancelled / error 均映射为可读文案", () => {
    expect(formatAgentEndReason("stop")).toBe("完成");
    expect(formatAgentEndReason("cancelled")).toBe("已取消");
    expect(formatAgentEndReason("error")).toBe("运行出错");
  });

  it("mcp_error 优先展示后端给出的具体说明", () => {
    expect(formatAgentEndReason("mcp_error", null, "MCP server 连接失败，已剔除相关工具")).toContain(
      "已剔除相关工具",
    );
  });
});

describe("mergeDoneMessages", () => {
  const userMsg: Message = { role: "user", content: "你好" };

  it("后端缺少当前轮 assistant 消息时，保留本地已流式生成的内容", () => {
    const localAssistant: Message = {
      role: "assistant",
      content: "部分结果",
      reasoning_content: "部分思考",
    };
    const local = [userMsg, localAssistant];
    const merged = mergeDoneMessages(local, 1, [userMsg]);
    expect(merged).toEqual([userMsg, localAssistant]);
  });

  it("后端已包含同一轮 assistant 消息时不重复追加", () => {
    const localAssistant: Message = {
      role: "assistant",
      content: "结果",
      reasoning_content: "思考",
    };
    const local = [userMsg, localAssistant];
    const merged = mergeDoneMessages(local, 1, [userMsg, localAssistant]);
    expect(merged).toEqual([userMsg, localAssistant]);
  });

  it("本轮没有内容（空 assistant）时不追加占位消息", () => {
    const emptyAssistant: Message = { role: "assistant", content: null };
    const local = [userMsg, emptyAssistant];
    const merged = mergeDoneMessages(local, 1, [userMsg]);
    expect(merged).toEqual([userMsg]);
  });

  it("无本地 assistant 索引时直接采用后端消息", () => {
    const merged = mergeDoneMessages([userMsg], -1, [userMsg]);
    expect(merged).toEqual([userMsg]);
  });
});
