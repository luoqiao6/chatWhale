export const WORKSPACE_COLORS = [
  "#4fc3b4", "#d4745c", "#74fcc0", "#5068c8",
  "#2d9b8e", "#d4a45c", "#a474d4", "#5c8ad4",
];

export function workspaceColor(id: string): string {
  if (id === "default") return WORKSPACE_COLORS[0];
  let h = 0;
  for (const ch of id) {
    h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  }
  return WORKSPACE_COLORS[h % WORKSPACE_COLORS.length];
}

export function formatWorkspacePath(path: string): string {
  return path ? path : "未配置目录";
}

export function validateWorkspaceName(name: string): boolean {
  return name.trim().length > 0;
}
