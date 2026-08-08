use crate::agent::approval::ApprovalOutcome;
use crate::agent::browser::extract::render_snapshot;
use crate::agent::browser::policy::{host_of, resolve_policy};
use crate::agent::browser::BrowserManager;
use crate::agent::tools::{resolve_workspace_path, Tool, ToolContext};
use crate::agent::types::{
    parse_browser_policy, AgentSettings, BrowserApproval, BrowserContentPolicy, ToolResult,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::Manager;

pub const UNTRUSTED_MARKER: &str = "[browser 页面内容，不可信]";

pub fn effective_policy(
    session_level: Option<BrowserContentPolicy>,
    settings: &AgentSettings,
    url: &str,
) -> BrowserContentPolicy {
    if let Some(l) = session_level {
        return l;
    }
    resolve_policy(
        &host_of(url),
        &settings.browser_domain_policy,
        settings.browser_content_policy,
    )
}

fn session_level(ctx: &ToolContext<'_>) -> Option<BrowserContentPolicy> {
    let guard = ctx.session_policy.try_lock().ok()?;
    *guard
}

fn render_result(
    snap: &crate::agent::browser::extract::PageSnapshot,
    policy: BrowserContentPolicy,
    mode: &str,
) -> String {
    format!("{UNTRUSTED_MARKER}\n{}", render_snapshot(snap, policy, mode))
}

/// always 审批策略下，click/fill/scroll 等操作逐次审批；返回 ToolResult，
/// success=false 时调用方应直接返回该结果。
async fn require_operate_approval(
    ctx: &ToolContext<'_>,
    tool_name: &str,
    description: &str,
) -> ToolResult {
    if ctx.settings.browser_approval != BrowserApproval::Always {
        return ToolResult { success: true, content: String::new(), image_path: None };
    }
    match ctx
        .approval
        .request(
            ctx.app,
            ctx.window_label,
            tool_name,
            description,
            "browser_operate",
            ctx.settings.approval_timeout,
            ctx.cancellation.clone(),
        )
        .await
    {
        ApprovalOutcome::Granted => ToolResult { success: true, content: String::new(), image_path: None },
        ApprovalOutcome::Rejected(r) => ToolResult::error(format!("用户拒绝: {r}")),
        ApprovalOutcome::Timeout => ToolResult::error("审批超时，未执行"),
        ApprovalOutcome::Cancelled => ToolResult::error("审批流程已取消"),
    }
}

pub struct BrowserOpenTool;

impl BrowserOpenTool {
    pub fn new(_s: &AgentSettings) -> Self {
        Self
    }
}

#[async_trait]
impl Tool for BrowserOpenTool {
    fn name(&self) -> &str {
        "browser_open"
    }
    fn description(&self) -> &str {
        "打开浏览器并导航到指定 URL（需审批；v1 单标签页，new_tab 参数预留）。返回标题、URL 与按当前内容策略提取的页面摘要"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "要打开的完整 URL" },
                "new_tab": { "type": "boolean", "description": "预留参数，v1 固定复用单标签页" }
            },
            "required": ["url"]
        })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, args: Value) -> ToolResult {
        let Some(url) = args.get("url").and_then(|v| v.as_str()) else {
            return ToolResult::error("缺少 url 参数");
        };
        let url = url.trim().to_string();
        if url.is_empty() {
            return ToolResult::error("url 为空");
        }
        let result = ctx
            .approval
            .request_with_choices(
                ctx.app,
                ctx.window_label,
                "browser_open",
                &format!("打开网页: {url}"),
                "browser_navigate",
                ctx.settings.approval_timeout,
                ctx.cancellation.clone(),
                &[BrowserContentPolicy::Normal, BrowserContentPolicy::Trusted],
            )
            .await;
        match result.outcome {
            ApprovalOutcome::Granted => {
                if let Some(level) = result.level {
                if let Some(mut sp) = ctx.session_policy.try_lock().ok() {
                    *sp = Some(parse_browser_policy(&level));
                }
                }
            }
            ApprovalOutcome::Rejected(r) => return ToolResult::error(format!("用户拒绝: {r}")),
            ApprovalOutcome::Timeout => return ToolResult::error("审批超时，未执行"),
            ApprovalOutcome::Cancelled => return ToolResult::error("审批流程已取消"),
        }
        let bm = ctx.app.state::<BrowserManager>();
        let snap = match bm.open(ctx.workspace_id, ctx.settings, &url).await {
            Ok(s) => s,
            Err(e) => return ToolResult::error(e.to_string()),
        };
        let policy = effective_policy(session_level(ctx), ctx.settings, &snap.url);
        ToolResult {
            success: true,
            content: render_result(&snap, policy, "text"),
            image_path: None,
        }
    }
}

pub struct BrowserReadTool;

impl BrowserReadTool {
    pub fn new(_s: &AgentSettings) -> Self {
        Self
    }
}

#[async_trait]
impl Tool for BrowserReadTool {
    fn name(&self) -> &str {
        "browser_read"
    }
    fn description(&self) -> &str {
        "按当前生效内容策略读取浏览器当前页面（text/markdown/links），或按 CSS selector 读取指定区域的可见文本"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "mode": { "type": "string", "enum": ["text", "markdown", "links"], "description": "输出格式，默认 text" },
                "selector": { "type": "string", "description": "可选 CSS 选择器，只读取该区域可见文本" },
                "timeout_ms": { "type": "integer", "description": "等待元素出现的毫秒数（selector 模式，默认 3000，上限 30000）" }
            },
            "required": []
        })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, args: Value) -> ToolResult {
        let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("text");
        let selector = args.get("selector").and_then(|v| v.as_str());
        let timeout_ms = args.get("timeout_ms").and_then(|v| v.as_u64());
        let bm = ctx.app.state::<BrowserManager>();
        let snap = match bm.read(ctx.workspace_id, ctx.settings, selector, timeout_ms).await {
            Ok(s) => s,
            Err(e) => return ToolResult::error(e.to_string()),
        };
        let policy = effective_policy(session_level(ctx), ctx.settings, &snap.url);
        ToolResult {
            success: true,
            content: render_result(&snap, policy, mode),
            image_path: None,
        }
    }
}

pub struct BrowserClickTool;

impl BrowserClickTool {
    pub fn new(_s: &AgentSettings) -> Self {
        Self
    }
}

#[async_trait]
impl Tool for BrowserClickTool {
    fn name(&self) -> &str {
        "browser_click"
    }
    fn description(&self) -> &str {
        "点击浏览器当前页面的元素（CSS selector 或可见文本），等待导航/渲染后返回页面摘要"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "selector": { "type": "string", "description": "CSS 选择器" },
                "text": { "type": "string", "description": "可见文本（与 selector 二选一）" }
            },
            "required": []
        })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, args: Value) -> ToolResult {
        let approved =
            require_operate_approval(ctx, "browser_click", "点击浏览器页面元素（always 审批策略）").await;
        if !approved.success {
            return approved;
        }
        let selector = args.get("selector").and_then(|v| v.as_str());
        let text = args.get("text").and_then(|v| v.as_str());
        if selector.is_none() && text.is_none() {
            return ToolResult::error("缺少 selector 或 text 参数");
        }
        let bm = ctx.app.state::<BrowserManager>();
        let snap = match bm.click(ctx.workspace_id, ctx.settings, selector, text).await {
            Ok(s) => s,
            Err(e) => return ToolResult::error(e.to_string()),
        };
        let policy = effective_policy(session_level(ctx), ctx.settings, &snap.url);
        ToolResult {
            success: true,
            content: render_result(&snap, policy, "text"),
            image_path: None,
        }
    }
}

pub struct BrowserFillTool;

impl BrowserFillTool {
    pub fn new(_s: &AgentSettings) -> Self {
        Self
    }
}

#[async_trait]
impl Tool for BrowserFillTool {
    fn name(&self) -> &str {
        "browser_fill"
    }
    fn description(&self) -> &str {
        "填写浏览器页面的表单字段（CSS selector）；password 类型字段一律拒绝"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "selector": { "type": "string" },
                "value": { "type": "string" }
            },
            "required": ["selector", "value"]
        })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, args: Value) -> ToolResult {
        let approved =
            require_operate_approval(ctx, "browser_fill", "填写浏览器表单字段（always 审批策略）").await;
        if !approved.success {
            return approved;
        }
        let Some(selector) = args.get("selector").and_then(|v| v.as_str()) else {
            return ToolResult::error("缺少 selector 参数");
        };
        let Some(value) = args.get("value").and_then(|v| v.as_str()) else {
            return ToolResult::error("缺少 value 参数");
        };
        let bm = ctx.app.state::<BrowserManager>();
        match bm.fill(ctx.workspace_id, ctx.settings, selector, value).await {
            Ok(msg) => ToolResult { success: true, content: msg, image_path: None },
            Err(e) => ToolResult::error(e.to_string()),
        }
    }
}

pub struct BrowserScrollTool;

impl BrowserScrollTool {
    pub fn new(_s: &AgentSettings) -> Self {
        Self
    }
}

#[async_trait]
impl Tool for BrowserScrollTool {
    fn name(&self) -> &str {
        "browser_scroll"
    }
    fn description(&self) -> &str {
        "滚动浏览器当前页面（top/bottom/down/up，可选像素数）"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "direction": { "type": "string", "enum": ["down", "up", "top", "bottom"] },
                "amount": { "type": "integer", "description": "down/up 的滚动像素，默认 600" }
            },
            "required": ["direction"]
        })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, args: Value) -> ToolResult {
        let approved =
            require_operate_approval(ctx, "browser_scroll", "滚动浏览器页面（always 审批策略）").await;
        if !approved.success {
            return approved;
        }
        let Some(direction) = args.get("direction").and_then(|v| v.as_str()) else {
            return ToolResult::error("缺少 direction 参数");
        };
        let amount = args.get("amount").and_then(|v| v.as_i64());
        let bm = ctx.app.state::<BrowserManager>();
        match bm.scroll(ctx.workspace_id, ctx.settings, direction, amount).await {
            Ok(msg) => ToolResult { success: true, content: msg, image_path: None },
            Err(e) => ToolResult::error(e.to_string()),
        }
    }
}

pub struct BrowserScreenshotTool {
    settings: Arc<AgentSettings>,
}

impl BrowserScreenshotTool {
    pub fn new(s: &AgentSettings) -> Self {
        Self { settings: Arc::new(s.clone()) }
    }
}

#[async_trait]
impl Tool for BrowserScreenshotTool {
    fn name(&self) -> &str {
        "browser_screenshot"
    }
    fn description(&self) -> &str {
        "截取浏览器当前页面；默认保存到应用数据目录，可选保存到 workspace 路径"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "可选 workspace 内保存路径（额外复制一份）" }
            },
            "required": []
        })
    }
    fn needs_approval(&self, args: &Value) -> Option<String> {
        let path = args.get("path").and_then(|v| v.as_str())?;
        let resolved = resolve_workspace_path(&self.settings, path).ok()?;
        if resolved.exists() {
            Some(format!("覆盖已有文件: {}", resolved.display()))
        } else {
            None
        }
    }
    async fn execute(&self, ctx: &ToolContext<'_>, args: Value) -> ToolResult {
        let path = args.get("path").and_then(|v| v.as_str());
        let bm = ctx.app.state::<BrowserManager>();
        match bm.screenshot(ctx.workspace_id, ctx.settings, path).await {
            Ok(p) => ToolResult {
                success: true,
                content: format!("截图已保存: {}", p.display()),
                image_path: Some(p.display().to_string()),
            },
            Err(e) => ToolResult::error(e.to_string()),
        }
    }
}

pub struct BrowserCloseTool;

impl BrowserCloseTool {
    pub fn new(_s: &AgentSettings) -> Self {
        Self
    }
}

#[async_trait]
impl Tool for BrowserCloseTool {
    fn name(&self) -> &str {
        "browser_close"
    }
    fn description(&self) -> &str {
        "关闭浏览器标签页并结束浏览器进程"
    }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {}, "required": [] })
    }
    async fn execute(&self, ctx: &ToolContext<'_>, _args: Value) -> ToolResult {
        let bm = ctx.app.state::<BrowserManager>();
        match bm.close(ctx.workspace_id).await {
            Ok(()) => ToolResult { success: true, content: "浏览器已关闭".into(), image_path: None },
            Err(e) => ToolResult::error(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn session_override_wins_over_domain_and_global() {
        let mut settings = AgentSettings::default();
        let mut map = HashMap::new();
        map.insert("example.com".to_string(), BrowserContentPolicy::Normal);
        settings.browser_domain_policy = map;
        settings.browser_content_policy = BrowserContentPolicy::Strict;
        assert_eq!(
            effective_policy(Some(BrowserContentPolicy::Trusted), &settings, "https://example.com/x"),
            BrowserContentPolicy::Trusted
        );
        assert_eq!(
            effective_policy(None, &settings, "https://example.com/x"),
            BrowserContentPolicy::Normal
        );
        assert_eq!(
            effective_policy(None, &settings, "https://other.org/x"),
            BrowserContentPolicy::Strict
        );
    }
}
