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
];
