/**
 * 将设置对象的所有值统一转为字符串。
 *
 * Agent 设置界面里 number 输入框经 Vue v-model 转换后值是数字，
 * 而后端 set_agent_settings 的参数是 HashMap<String, String>，
 * 直接传数字会导致 serde 反序列化失败、整个保存请求被拒绝。
 */
export function normalizeAgentSettings(
  settings: Record<string, unknown>,
): Record<string, string> {
  const normalized: Record<string, string> = {};
  for (const [key, value] of Object.entries(settings)) {
    if (typeof value === "string") {
      normalized[key] = value;
    } else if (value === null || value === undefined) {
      normalized[key] = "";
    } else {
      normalized[key] = String(value);
    }
  }
  return normalized;
}
