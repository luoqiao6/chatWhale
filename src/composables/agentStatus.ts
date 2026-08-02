import type { AgentDonePayload, Message } from "../types";

/**
 * 把 Agent 结束原因映射为可读文案。
 * reason 为 "finish_reason" 时表示流式响应以 length/content_filter 等异常
 * finish_reason 结束，需结合 payload 里的实际值给出说明。
 */
export function formatAgentEndReason(
  reason: AgentDonePayload["reason"],
  finishReason?: string | null,
  mcpError?: string | null,
): string {
  switch (reason) {
    case "stop":
      return "完成";
    case "max_iterations":
      return "达到最大工具循环次数";
    case "cancelled":
      return "已取消";
    case "finish_reason":
      return formatAbnormalFinish(finishReason ?? null);
    case "mcp_error":
      return mcpError || "MCP server 连接失败";
    case "error":
      return "运行出错";
  }
}

function formatAbnormalFinish(finishReason: string | null): string {
  switch (finishReason) {
    case "length":
      return "输出达到 max_tokens 上限被截断（length）";
    case "content_filter":
      return "输出被内容安全策略拦截（content_filter）";
    case "insufficient_system_resource":
      return "系统资源不足，输出被中断（insufficient_system_resource）";
    default:
      return finishReason ? `非正常结束（${finishReason}）` : "非正常结束";
  }
}

/**
 * 合并 agent-done 消息：后端数组为权威，但若缺少本轮本地已流式生成的
 * assistant 消息（异常/取消/出错时后端可能没有保留当前轮），则补回本地内容，
 * 避免界面上的推理与结果被整体覆盖丢失。
 */
export function mergeDoneMessages(
  localMessages: Message[],
  localAssistantIndex: number,
  payloadMessages: Message[],
): Message[] {
  const merged = payloadMessages.slice();
  if (localAssistantIndex < 0 || localAssistantIndex >= localMessages.length) {
    return merged;
  }
  const local = localMessages[localAssistantIndex];
  const hasContent = Boolean(
    local.content ||
      local.reasoning_content ||
      (local.tool_calls && local.tool_calls.length > 0),
  );
  if (!hasContent) {
    return merged;
  }
  const alreadyInPayload = merged.some(
    (m) =>
      m.role === "assistant" &&
      m.content === local.content &&
      m.reasoning_content === local.reasoning_content,
  );
  if (!alreadyInPayload) {
    merged.push(local);
  }
  return merged;
}
