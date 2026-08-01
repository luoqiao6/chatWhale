use crate::agent::approval::{policy_allows, ApprovalOutcome};
use crate::agent::types::{AgentSettings, ToolCall, ToolDef, ToolResult};
use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::io::{AsyncReadExt, BufReader};
use tokio_util::sync::CancellationToken;

pub struct ToolContext<'a> {
    pub app: &'a AppHandle,
    pub window_label: Option<&'a str>,
    pub settings: &'a AgentSettings,
    pub approval: &'a super::approval::ApprovalManager,
    pub cancellation: CancellationToken,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    /// 返回 Some(展示文本) 表示该调用需要审批（命令执行、覆盖文件等）。
    fn needs_approval(&self, _args: &Value) -> Option<String> {
        None
    }
    async fn execute(&self, ctx: &ToolContext<'_>, args: Value) -> ToolResult;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    order: Vec<String>,
}

impl ToolRegistry {
    pub fn with_builtins(settings: &AgentSettings) -> Self {
        let mut r = Self {
            tools: HashMap::new(),
            order: Vec::new(),
        };
        r.register(Box::new(ReadFileTool::new(settings)));
        r.register(Box::new(WriteFileTool::new(settings)));
        r.register(Box::new(ListDirectoryTool::new(settings)));
        r.register(Box::new(SearchFilesTool::new(settings)));
        r.register(Box::new(ExecuteCommandTool::new(settings)));
        r
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            return;
        }
        self.tools.insert(name.clone(), tool);
        self.order.push(name);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|b| b.as_ref())
    }

    pub fn list_definitions(&self) -> Vec<ToolDef> {
        self.order
            .iter()
            .filter_map(|n| self.tools.get(n))
            .map(|t| ToolDef::new(t.name(), t.description(), t.parameters()))
            .collect()
    }

    /// 执行工具：先做审批检查（命令工具走策略，写文件覆盖走确认），再执行、脱敏、截断。
    pub async fn execute(&self, ctx: &ToolContext<'_>, call: &ToolCall) -> ToolResult {
        let Some(tool) = self.get(&call.function.name) else {
            return ToolResult::error(format!("未知工具: {}", call.function.name));
        };
        let args: Value =
            serde_json::from_str(&call.function.arguments).unwrap_or(Value::Null);

        if let Some(reason) = tool.needs_approval(&args) {
            let is_command = tool.name() == "execute_command";
            let need_approval = if is_command {
                let cmd = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let cwd = args.get("cwd").and_then(|v| v.as_str());
                match policy_allows(ctx.settings, cmd, cwd) {
                    Some(true) => return ToolResult::error("命令执行已被禁用"),
                    Some(false) => true,
                    None => false,
                }
            } else {
                true
            };
            if need_approval {
                let display = if is_command {
                    let cmd = args
                        .get("command")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    format!("{reason}\n命令: {cmd}")
                } else {
                    reason.clone()
                };
                match ctx
                    .approval
                    .request(
                        ctx.app,
                        ctx.window_label,
                        &call.function.name,
                        &display,
                        "execute_command",
                        ctx.settings.approval_timeout,
                        ctx.cancellation.clone(),
                    )
                    .await
                {
                    ApprovalOutcome::Granted => {}
                    ApprovalOutcome::Rejected(r) => {
                        return ToolResult::error(format!("用户拒绝: {r}"));
                    }
                    ApprovalOutcome::Timeout => {
                        return ToolResult::error("审批超时，未执行");
                    }
                    ApprovalOutcome::Cancelled => {
                        return ToolResult::error("审批流程已取消");
                    }
                }
            }
        }

        let mut result = tool.execute(ctx, args).await;
        result.content = finalize_result(result.content, ctx.settings.max_result_bytes);
        result
    }
}

fn finalize_result(content: String, max_bytes: usize) -> String {
    let (redacted, _) = redact_secrets(&content);
    truncate_result(&redacted, max_bytes)
}

pub fn truncate_result(content: &str, max_bytes: usize) -> String {
    if content.len() <= max_bytes {
        return content.to_string();
    }
    let cut: String = content
        .chars()
        .take(max_bytes.saturating_sub(64))
        .collect();
    format!("{cut}\n[已截断: 原始 {} 字节]", content.len())
}

/// 脱敏：命中模式替换为 [REDACTED]，返回脱敏文本与命中次数。
pub fn redact_secrets(content: &str) -> (String, usize) {
    let patterns = [
        r"sk-[A-Za-z0-9]{20,}",
        r"AKIA[0-9A-Z]{16}",
        r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----",
        r"ghp_[A-Za-z0-9]{36,}",
        r"Bearer\s+[A-Za-z0-9._-]{20,}",
    ];
    let mut count = 0usize;
    let mut out = content.to_string();
    for p in patterns {
        let Ok(re) = Regex::new(p) else { continue };
        count += re.find_iter(&out).count();
        out = re.replace_all(&out, "[REDACTED]").to_string();
    }
    (out, count)
}

fn workspace_root(settings: &AgentSettings) -> anyhow::Result<PathBuf> {
    let root = settings
        .workspace_root
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("请先在 Agent 设置中配置工作目录"))?;
    let canon = std::fs::canonicalize(root)
        .map_err(|e| anyhow::anyhow!("工作目录不可用: {e}"))?;
    Ok(canon)
}

/// 路径沙箱：规范化（解析符号链接）后必须以 workspace 根为前缀，否则拒绝。
pub fn resolve_workspace_path(
    settings: &AgentSettings,
    path: &str,
) -> anyhow::Result<PathBuf> {
    let root = workspace_root(settings)?;
    let raw = PathBuf::from(path);
    let joined = if raw.is_absolute() { raw } else { root.join(raw) };
    let canon = std::fs::canonicalize(&joined)
        .or_else(|_| {
            // 目标不存在时（写文件），规范化其父目录并拼接文件名
            let parent = joined.parent().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "非法路径")
            })?;
            let name = joined.file_name().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "非法路径")
            })?;
            std::fs::canonicalize(parent).map(|p| p.join(name))
        })
        .map_err(|_| anyhow::anyhow!("路径超出 workspace 范围"))?;
    if canon.starts_with(&root) {
        Ok(canon)
    } else {
        Err(anyhow::anyhow!("路径超出 workspace 范围"))
    }
}

/// 内置 deny-list + 用户扩展（glob）。命中即拒绝。
pub fn is_denied_path(settings: &AgentSettings, path: &Path, _read: bool) -> bool {
    let file = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if file == ".git-credentials"
        || file == "id_rsa"
        || file == "id_ed25519"
        || file.starts_with(".env")
        || file.ends_with(".pem")
        || file.ends_with(".key")
        || file.ends_with(".pfx")
        || path
            .components()
            .any(|c| c.as_os_str() == ".ssh")
    {
        return true;
    }
    let full = path.to_string_lossy();
    settings
        .sensitive_paths
        .iter()
        .any(|pat| glob_to_regex(pat).is_match(&full))
}

fn glob_to_regex(pat: &str) -> Regex {
    let re = format!(
        "^{}$",
        regex::escape(pat).replace("\\*", ".*").replace("\\?", ".")
    );
    Regex::new(&re).unwrap_or_else(|_| Regex::new("$^").unwrap())
}

fn resolve_command_cwd(settings: &AgentSettings, cwd_arg: Option<&str>) -> PathBuf {
    if let Some(c) = cwd_arg {
        let p = PathBuf::from(c);
        if p.is_absolute() {
            return p;
        }
        if let Some(root) = &settings.workspace_root {
            return root.join(p);
        }
        return p;
    }
    if let Some(root) = &settings.workspace_root {
        root.clone()
    } else {
        std::env::current_dir().unwrap_or_default()
    }
}

struct ReadFileTool {
    settings: Arc<AgentSettings>,
}

impl ReadFileTool {
    fn new(s: &AgentSettings) -> Self {
        Self {
            settings: Arc::new(s.clone()),
        }
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description(&self) -> &str {
        "读取 workspace 内的文本文件（最多 1000 行，结果按上限截断）"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "相对 workspace 或绝对路径" } },
            "required": ["path"]
        })
    }
    async fn execute(&self, _ctx: &ToolContext<'_>, args: Value) -> ToolResult {
        let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
            return ToolResult::error("缺少 path 参数");
        };
        let resolved = match resolve_workspace_path(&self.settings, path) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(e.to_string()),
        };
        if is_denied_path(&self.settings, &resolved, true) {
            return ToolResult::error("该文件属于敏感文件 deny-list，禁止读取");
        }
        let content = match std::fs::read_to_string(&resolved) {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("读取失败: {e}")),
        };
        let total_lines = content.lines().count();
        let mut out: String = content.lines().take(1000).collect::<Vec<_>>().join("\n");
        if total_lines > 1000 {
            out.push_str(&format!("\n[已截断: 超过 1000 行，原始 {total_lines} 行]"));
        }
        ToolResult {
            success: true,
            content: out,
        }
    }
}

struct WriteFileTool {
    settings: Arc<AgentSettings>,
}

impl WriteFileTool {
    fn new(s: &AgentSettings) -> Self {
        Self {
            settings: Arc::new(s.clone()),
        }
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }
    fn description(&self) -> &str {
        "写入文件到 workspace（覆盖已有文件需用户审批）"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string", "description": "要写入的完整内容" }
            },
            "required": ["path", "content"]
        })
    }
    fn needs_approval(&self, args: &Value) -> Option<String> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or_default();
        let resolved = resolve_workspace_path(&self.settings, path).ok()?;
        if resolved.exists() {
            Some(format!("覆盖已有文件: {}", resolved.display()))
        } else {
            None
        }
    }
    async fn execute(&self, _ctx: &ToolContext<'_>, args: Value) -> ToolResult {
        let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
            return ToolResult::error("缺少 path 参数");
        };
        let Some(content) = args.get("content").and_then(|v| v.as_str()) else {
            return ToolResult::error("缺少 content 参数");
        };
        let resolved = match resolve_workspace_path(&self.settings, path) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(e.to_string()),
        };
        if is_denied_path(&self.settings, &resolved, false) {
            return ToolResult::error("该路径属于敏感文件 deny-list，禁止写入");
        }
        if let Some(parent) = resolved.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&resolved, content) {
            Ok(()) => ToolResult {
                success: true,
                content: format!("已写入 {} 字节", content.len()),
            },
            Err(e) => ToolResult::error(format!("写入失败: {e}")),
        }
    }
}

struct ListDirectoryTool {
    settings: Arc<AgentSettings>,
}

impl ListDirectoryTool {
    fn new(s: &AgentSettings) -> Self {
        Self {
            settings: Arc::new(s.clone()),
        }
    }
}

#[async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }
    fn description(&self) -> &str {
        "列出 workspace 内目录的内容（名称/类型/大小）"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "默认 workspace 根" } },
            "required": []
        })
    }
    async fn execute(&self, _ctx: &ToolContext<'_>, args: Value) -> ToolResult {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        let resolved = match resolve_workspace_path(&self.settings, path) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(e.to_string()),
        };
        let mut entries: Vec<Value> = Vec::new();
        let Ok(rd) = std::fs::read_dir(&resolved) else {
            return ToolResult::error("目录读取失败");
        };
        for entry in rd.flatten() {
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let is_symlink = entry.file_type().map(|t| t.is_symlink()).unwrap_or(false);
            let name = entry.file_name().to_string_lossy().to_string();
            entries.push(json!({
                "name": name,
                "type": if is_dir { "dir" } else if is_symlink { "symlink" } else { "file" },
                "size": std::fs::metadata(entry.path()).map(|m| m.len()).unwrap_or(0),
            }));
        }
        entries.sort_by(|a, b| {
            let ka = (a["type"] != "dir", a["name"].as_str().unwrap_or(""));
            let kb = (b["type"] != "dir", b["name"].as_str().unwrap_or(""));
            ka.cmp(&kb)
        });
        ToolResult {
            success: true,
            content: serde_json::to_string(&entries).unwrap_or_default(),
        }
    }
}

struct SearchFilesTool {
    settings: Arc<AgentSettings>,
}

impl SearchFilesTool {
    fn new(s: &AgentSettings) -> Self {
        Self {
            settings: Arc::new(s.clone()),
        }
    }

    fn walk(&self, dir: &Path, re: &Regex, out: &mut Vec<String>) {
        if out.len() >= 100 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for entry in rd.flatten() {
            if out.len() >= 100 {
                return;
            }
            let path = entry.path();
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name == ".git" || name == "node_modules" || name == "target" {
                continue;
            }
            if is_denied_path(&self.settings, &path, true) {
                continue;
            }
            if path.is_dir() {
                self.walk(&path, re, out);
            } else if path.is_file() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if re.is_match(&content) {
                        out.push(path.display().to_string());
                    }
                }
            }
        }
    }
}

#[async_trait]
impl Tool for SearchFilesTool {
    fn name(&self) -> &str {
        "search_files"
    }
    fn description(&self) -> &str {
        "在 workspace 内递归搜索文件内容（glob 模式，跳过 deny-list 与 .git）"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "glob 模式，如 *.env 或 *.log" },
                "path": { "type": "string", "description": "起始目录，默认 workspace 根" }
            },
            "required": ["pattern"]
        })
    }
    async fn execute(&self, _ctx: &ToolContext<'_>, args: Value) -> ToolResult {
        let Some(pattern) = args.get("pattern").and_then(|v| v.as_str()) else {
            return ToolResult::error("缺少 pattern 参数");
        };
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let resolved = match resolve_workspace_path(&self.settings, path) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(e.to_string()),
        };
        let re = glob_to_regex(pattern);
        let mut matches = Vec::new();
        self.walk(&resolved, &re, &mut matches);
        ToolResult {
            success: true,
            content: serde_json::to_string(&matches).unwrap_or_default(),
        }
    }
}

struct ExecuteCommandTool {
    settings: Arc<AgentSettings>,
}

impl ExecuteCommandTool {
    fn new(s: &AgentSettings) -> Self {
        Self {
            settings: Arc::new(s.clone()),
        }
    }
}

#[async_trait]
impl Tool for ExecuteCommandTool {
    fn name(&self) -> &str {
        "execute_command"
    }
    fn description(&self) -> &str {
        "执行 shell 命令（需用户审批；默认超时 60s）"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" },
                "cwd": { "type": "string", "description": "工作目录，相对 workspace 或绝对路径" }
            },
            "required": ["command"]
        })
    }
    fn needs_approval(&self, _args: &Value) -> Option<String> {
        Some("执行 shell 命令需要用户审批".into())
    }
    async fn execute(&self, _ctx: &ToolContext<'_>, args: Value) -> ToolResult {
        let Some(command) = args.get("command").and_then(|v| v.as_str()) else {
            return ToolResult::error("缺少 command 参数");
        };
        if command.trim().is_empty() {
            return ToolResult::error("命令为空");
        }
        let cwd = resolve_command_cwd(
            &self.settings,
            args.get("cwd").and_then(|v| v.as_str()),
        );
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd.current_dir(&cwd);
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        cmd.process_group(0);

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("命令启动失败: {e}")),
        };
        let pid = child.id();
        let mut stdout_reader = child.stdout.take().map(BufReader::new);
        let mut stderr_reader = child.stderr.take().map(BufReader::new);
        let mut stdout_text = String::new();
        let mut stderr_text = String::new();
        tokio::select! {
            status = child.wait() => {
                if let Some(r) = stdout_reader.as_mut() {
                    let _ = r.read_to_string(&mut stdout_text).await;
                }
                if let Some(r) = stderr_reader.as_mut() {
                    let _ = r.read_to_string(&mut stderr_text).await;
                }
                match status {
                    Ok(status) => {
                        let mut text = stdout_text;
                        if !stderr_text.trim().is_empty() {
                            text.push_str("\n[stderr]\n");
                            text.push_str(&stderr_text);
                        }
                        if text.trim().is_empty() {
                            text = if status.success() {
                                "(命令执行成功，无输出)".into()
                            } else {
                                format!("(命令退出码: {:?})", status.code())
                            };
                        }
                        ToolResult {
                            success: status.success(),
                            content: text,
                        }
                    }
                    Err(e) => ToolResult::error(format!("命令执行失败: {e}")),
                }
            }
            _ = tokio::time::sleep(self.settings.command_timeout) => {
                // 超时：kill 进程组（子进程与后代），防止孤儿进程
                #[cfg(unix)]
                if let Some(pid) = pid {
                    unsafe {
                        libc::killpg(pid as i32, libc::SIGKILL);
                    }
                }
                let _ = child.kill().await;
                let _ = child.wait().await;
                ToolResult::error(format!(
                    "命令执行超时（{}s）",
                    self.settings.command_timeout.as_secs()
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn settings_with_workspace(p: &str) -> AgentSettings {
        AgentSettings {
            workspace_root: Some(PathBuf::from(p)),
            max_result_bytes: 204_800,
            ..Default::default()
        }
    }

    #[test]
    fn sandbox_rejects_outside_workspace() {
        let root = std::env::temp_dir().join("cw-sandbox-test");
        std::fs::create_dir_all(&root).unwrap();
        let settings = settings_with_workspace(root.to_str().unwrap());
        let r = resolve_workspace_path(&settings, "../etc/passwd");
        assert!(r.is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sandbox_rejects_symlink_escape() {
        let root = std::env::temp_dir().join("cw-symlink-test");
        let outside = std::env::temp_dir().join("cw-symlink-outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let link = root.join("escape");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        let settings = settings_with_workspace(root.to_str().unwrap());
        let r = resolve_workspace_path(&settings, "escape");
        assert!(r.is_err());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn denies_sensitive_env_file() {
        let settings = settings_with_workspace("/tmp");
        assert!(is_denied_path(
            &settings,
            std::path::Path::new("/tmp/project/.env"),
            true
        ));
        assert!(is_denied_path(
            &settings,
            std::path::Path::new("/tmp/project/.env.local"),
            true
        ));
        assert!(is_denied_path(
            &settings,
            std::path::Path::new("/tmp/.ssh/config"),
            true
        ));
        assert!(!is_denied_path(
            &settings,
            std::path::Path::new("/tmp/readme.md"),
            true
        ));
    }

    #[test]
    fn redacts_secret_patterns() {
        let (out, count) = redact_secrets("key=sk-abc123def456ghi789jkl012, ak=AKIAIOSFODNN7EXAMPLE");
        assert!(out.contains("[REDACTED]"));
        assert!(count >= 2);
    }

    #[test]
    fn truncates_oversized_results() {
        let big = "x".repeat(300);
        let out = truncate_result(&big, 100);
        assert!(out.contains("[已截断"));
        assert!(out.len() < 200);
    }
}
