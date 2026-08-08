export interface SettingField {
  key: string;
  label: string;
  type: "text" | "number" | "select" | "textarea";
  options?: string[];
}

export const SETTING_FIELDS: SettingField[] = [
  { key: "agent.skills_dir", label: "Skills 目录（全局）", type: "text" },
  { key: "agent.command_approval", label: "命令审批策略", type: "select", options: ["always", "whitelist", "never"] },
  { key: "agent.max_iterations", label: "最大工具循环次数", type: "number" },
  { key: "agent.llm_timeout", label: "LLM 超时（秒）", type: "number" },
  { key: "agent.command_timeout", label: "命令超时（秒）", type: "number" },
  { key: "agent.approval_timeout", label: "审批超时（秒）", type: "number" },
  { key: "agent.max_result_bytes", label: "工具结果上限（字节）", type: "number" },
  { key: "agent.command_whitelist", label: "命令白名单（JSON）", type: "textarea" },
  { key: "agent.sensitive_paths", label: "敏感路径扩展（JSON，glob）", type: "textarea" },
  { key: "agent.browser_enabled", label: "浏览器工具（CDP）", type: "select", options: ["false", "true"] },
  { key: "agent.browser_path", label: "浏览器可执行文件路径（留空自动探测）", type: "text" },
  { key: "agent.browser_approval", label: "浏览器操作审批策略", type: "select", options: ["navigation", "always"] },
  { key: "agent.browser_content_policy", label: "网页内容读取级别（全局默认）", type: "select", options: ["strict", "normal", "trusted"] },
  { key: "agent.browser_domain_policy", label: "域名覆盖（JSON：域名→级别）", type: "textarea" },
];
