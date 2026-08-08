# chatWhale 浏览器工具（CDP 驱动 Chrome/Edge）实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 chatWhale Agent 增加一组 `browser_*` 内置工具：可见窗口驱动系统 Chrome/Edge，支持打开/读取（按用户可选内容策略）/点击/填表/滚动/截图/关闭，且登录态按工作空间隔离、跨会话保留。

**Architecture:** 在 `src-tauri/src/agent/browser/` 新增模块，采用 **chromiumoxide（CDP 协议客户端）**驱动系统 Chrome/Edge：通过 `BrowserConfig` 指定可执行文件与独立 `--user-data-dir`，启动**可见窗口**并连接 CDP；`cdp.rs` 只暴露导航/求值/截图三个内部方法，把 chromiumoxide 的 API 差异隔离在单文件内。`BrowserManager` 以 Tauri managed state 存在（`app.manage`），按 workspace 缓存会话并串行化操作；7 个浏览器工具复用现有 `Tool` trait、审批管理器与事件推送。内容读取级别（strict/normal/trusted）由用户选择：全局默认 + 域名覆盖 + `browser_open` 审批弹窗“本次临时放宽”。

**Tech Stack:** Rust（tokio、reqwest 已有；新增 `chromiumoxide` 0.7）、Tauri v2 managed state、Vue 3 + TypeScript、vitest。

## Global Constraints

- 一律使用简体中文注释与提交说明。
- 设计文档：`docs/superpowers/specs/2026-08-08-browser-tools-design.md`（本计划逐条落实其第 5、7、8、10、12 节）。
- **chromiumoxide 为 CDP 客户端实现**：`cdp.rs` 只暴露启动/求值/导航/截图等少量内部方法（浏览器启动、页面切换、事件排空由 chromiumoxide 处理）；不做 `browser_eval`（后续增强）。Task 4 含“核对本地 crate API”步骤，chromiumoxide 小版本 API 差异只允许在 `cdp.rs` 内适配。
- 模型不可自选内容级别：工具不暴露 policy 参数；级别仅由用户通过设置或审批弹窗决定。
- 所有 `browser_*` 工具结果继续走 `mod.rs` 的统一出口：`finalize_result`（脱敏 + 按 `agent.max_result_bytes` 截断），并前置不可信标记 `[browser 页面内容，不可信]`。
- `browser_open` 必须审批（弹窗展示完整 URL，并提供 normal/trusted 临时放宽选项）；`always` 审批策略下 `browser_click` / `browser_fill` / `browser_scroll` 也逐次审批。
- 截图默认保存到 `<app_data>/browser-screenshots/<workspace_id>/`；可选 `path` 参数（workspace 内）额外复制一份。
- 提取过滤在结果边界强制执行：`extract.rs` 只把策略允许的数据放进 `ToolResult`；password 类型字段的值在任何级别都跳过。
- 验收命令（AGENTS.md 口径）：`cargo test`、`npm test`、`npm run typecheck`、`npm run build` 全绿；提交前执行。
- 新增依赖需联网拉取（`chromiumoxide` 及其传递依赖）；首次 `cargo check` 若因网络被沙箱拦截，需按提示申请批准。
- git 提交需要写入 `.git`（沙箱外权限）；若被沙箱拦截，按提示申请批准，不要绕过。
- 不改动：`sse.rs`（普通模式遗留）、普通模式前端 fetch 流式、MCP stdio 传输一期实现。
- 工作区未跟踪的 `test/` 目录属于用户文件，提交时只 `git add` 本任务相关文件，不得 `git add -A`。

---

## 文件结构

```
src-tauri/
├── Cargo.toml                     # 修改: 新增 chromiumoxide
├── capabilities/default.json      # 修改: 新增 assetProtocol 作用域（截图预览）
├── src/
│   ├── lib.rs                     # 修改: agent_approve 增加 level; setup 中 manage BrowserManager
│   └── agent/
│       ├── types.rs               # 修改: BrowserContentPolicy / BrowserApproval / AgentSettings 新字段 / AGENT_SETTING_KEYS / 解析
│       ├── approval.rs            # 修改: ApprovalReply + request_with_choices + resolve_with_level
│       ├── tools.rs               # 修改: ToolContext 增加 workspace_id / session_policy; ToolResult 增加 image_path; 注册浏览器工具
│       ├── mod.rs                 # 修改: 创建 session_policy 并注入 ToolContext; agent-tool-result 携带 image_path
│       ├── mcp/mod.rs             # 修改: ToolResult 结构体字面量补 image_path
│       └── browser/               # 新增
│           ├── mod.rs             # BrowserManager：会话缓存/串行化/生命周期 + Drop 清理
│           ├── policy.rs          # host_of / match_domain / resolve_policy（生效级别解析）
│           ├── locator.rs         # Chrome/Edge 可执行文件探测
│           ├── cdp.rs             # chromiumoxide 封装：启动/求值/导航/截图（API 差异隔离在此文件）
│           ├── extract.rs         # SNAPSHOT_JS + PageSnapshot + render_snapshot（按策略/mode）
│           └── tools.rs           # 7 个 browser_* 工具（含自我审批与临时放宽）
src-tauri/tests/browser_integration.rs   # 新增: 本地 fixture HTTP 服务 + 真实浏览器集成测试（无浏览器 skip）
src/
├── types/index.ts                 # 修改: ApprovalRequest.choices / ToolExecution.image_path
├── composables/useAgent.ts        # 修改: approveCommand(level) / ToolResultPayload.image_path
├── composables/agentSettingsFields.ts # 修改: 新增 5 个浏览器设置字段
├── composables/agentSettingsFields.test.ts # 新增: vitest
└── components/
    ├── AgentSettings.vue          # 修改: browser_domain_policy JSON 校验
    └── ChatView.vue               # 修改: 审批弹窗级别按钮 + 工具卡片截图缩略图
```

---

### Task 1: 内容策略类型（types.rs）

**Files:**
- Modify: `src-tauri/src/agent/types.rs`
- Test: `src-tauri/src/agent/types.rs`（模块内 `#[cfg(test)]`）

**Interfaces:**
- Consumes: 无。
- Produces:
  - `pub enum BrowserContentPolicy { Strict, Normal, Trusted }`（`Serialize/Deserialize`，`Default = Strict`，`PartialOrd/Ord`，变体顺序即级别顺序）
  - `impl BrowserContentPolicy { pub fn as_str(&self) -> &'static str }`
  - `pub fn parse_browser_policy(s: &str) -> BrowserContentPolicy`
  - `pub enum BrowserApproval { Navigation, Always }`（`Default = Navigation`）
  - `pub fn parse_browser_approval(s: &str) -> BrowserApproval`
  - `pub fn parse_domain_policy(s: &str) -> HashMap<String, BrowserContentPolicy>`
  - `AgentSettings` 新增字段：`browser_enabled: bool`、`browser_path: Option<String>`、`browser_approval: BrowserApproval`、`browser_content_policy: BrowserContentPolicy`、`browser_domain_policy: HashMap<String, BrowserContentPolicy>`
  - `AGENT_SETTING_KEYS` 新增 5 个键

- [ ] **Step 1: 写失败测试**

在 `src-tauri/src/agent/types.rs` 的测试模块追加：

```rust
#[test]
fn parses_browser_policy_variants() {
    assert_eq!(parse_browser_policy("strict"), BrowserContentPolicy::Strict);
    assert_eq!(parse_browser_policy("normal"), BrowserContentPolicy::Normal);
    assert_eq!(parse_browser_policy("trusted"), BrowserContentPolicy::Trusted);
    assert_eq!(parse_browser_policy("garbage"), BrowserContentPolicy::Strict);
    assert_eq!(BrowserContentPolicy::Trusted.as_str(), "trusted");
}

#[test]
fn browser_policy_ordering() {
    assert!(BrowserContentPolicy::Strict < BrowserContentPolicy::Normal);
    assert!(BrowserContentPolicy::Normal < BrowserContentPolicy::Trusted);
}

#[test]
fn parses_browser_approval_variants() {
    assert_eq!(parse_browser_approval("navigation"), BrowserApproval::Navigation);
    assert_eq!(parse_browser_approval("always"), BrowserApproval::Always);
    assert_eq!(parse_browser_approval("x"), BrowserApproval::Navigation);
}

#[test]
fn parses_domain_policy_json() {
    let map = parse_domain_policy(r#"{"example.com":"trusted","*.foo.com":"normal"}"#);
    assert_eq!(map.get("example.com"), Some(&BrowserContentPolicy::Trusted));
    assert_eq!(map.get("*.foo.com"), Some(&BrowserContentPolicy::Normal));
    assert!(parse_domain_policy("not json").is_empty());
}

#[test]
fn load_settings_includes_browser_defaults() {
    let map = HashMap::new();
    let s = load_agent_settings(&map);
    assert!(!s.browser_enabled);
    assert!(s.browser_path.is_none());
    assert_eq!(s.browser_approval, BrowserApproval::Navigation);
    assert_eq!(s.browser_content_policy, BrowserContentPolicy::Strict);
    assert!(s.browser_domain_policy.is_empty());
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test -p chatwhale agent::types`
Expected: FAIL（`BrowserContentPolicy` 等未定义，编译错误）

- [ ] **Step 3: 实现**

在 `src-tauri/src/agent/types.rs` 顶部 `use` 区确认已有 `HashMap`；在 `AgentSettings` 之前新增：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BrowserContentPolicy {
    #[default]
    Strict,
    Normal,
    Trusted,
}

impl BrowserContentPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            BrowserContentPolicy::Strict => "strict",
            BrowserContentPolicy::Normal => "normal",
            BrowserContentPolicy::Trusted => "trusted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BrowserApproval {
    #[default]
    Navigation,
    Always,
}

pub fn parse_browser_policy(s: &str) -> BrowserContentPolicy {
    match s.trim() {
        "normal" => BrowserContentPolicy::Normal,
        "trusted" => BrowserContentPolicy::Trusted,
        _ => BrowserContentPolicy::Strict,
    }
}

pub fn parse_browser_approval(s: &str) -> BrowserApproval {
    match s.trim() {
        "always" => BrowserApproval::Always,
        _ => BrowserApproval::Navigation,
    }
}

pub fn parse_domain_policy(s: &str) -> HashMap<String, BrowserContentPolicy> {
    serde_json::from_str::<HashMap<String, String>>(s)
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| (k, parse_browser_policy(&v)))
        .collect()
}
```

`AgentSettings` 增加字段：

```rust
pub struct AgentSettings {
    pub workspace_root: Option<PathBuf>,
    pub max_iterations: usize,
    pub skills_dir: Option<PathBuf>,
    pub command_approval: ApprovalPolicy,
    pub command_whitelist: Vec<WhitelistEntry>,
    pub llm_timeout: Duration,
    pub tool_timeout: Duration,
    pub command_timeout: Duration,
    pub approval_timeout: Duration,
    pub max_result_bytes: usize,
    pub sensitive_paths: Vec<String>,
    pub browser_enabled: bool,
    pub browser_path: Option<String>,
    pub browser_approval: BrowserApproval,
    pub browser_content_policy: BrowserContentPolicy,
    pub browser_domain_policy: HashMap<String, BrowserContentPolicy>,
}
```

`impl Default for AgentSettings` 的返回值补：

```rust
            sensitive_paths: Vec::new(),
            browser_enabled: false,
            browser_path: None,
            browser_approval: BrowserApproval::Navigation,
            browser_content_policy: BrowserContentPolicy::Strict,
            browser_domain_policy: HashMap::new(),
```

`AGENT_SETTING_KEYS` 末尾追加：

```rust
    ("agent.browser_enabled", "false"),
    ("agent.browser_path", ""),
    ("agent.browser_approval", "navigation"),
    ("agent.browser_content_policy", "strict"),
    ("agent.browser_domain_policy", "{}"),
```

`load_agent_settings` 中 `AgentSettings {` 字面量补：

```rust
        browser_enabled: parse_bool(&get("agent.browser_enabled", "false"), false),
        browser_path: {
            let p = get("agent.browser_path", "").trim().to_string();
            if p.is_empty() { None } else { Some(p) }
        },
        browser_approval: parse_browser_approval(&get("agent.browser_approval", "navigation")),
        browser_content_policy: parse_browser_policy(&get("agent.browser_content_policy", "strict")),
        browser_domain_policy: parse_domain_policy(&get("agent.browser_domain_policy", "{}")),
```

并在文件内新增：

```rust
pub fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim() {
        "true" | "1" => true,
        "false" | "0" => false,
        _ => default,
    }
}
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cargo test -p chatwhale agent::types`
Expected: PASS（5 个新测试 + 既有测试全绿）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/agent/types.rs
git commit -m "feat(agent): 新增浏览器内容策略与审批设置类型"
```

---

### Task 2: 内容策略解析（browser/policy.rs）

**Files:**
- Create: `src-tauri/src/agent/browser/policy.rs`
- Test: 同文件 `#[cfg(test)]`

**Interfaces:**
- Consumes: `crate::agent::types::BrowserContentPolicy`
- Produces:
  - `pub fn host_of(url: &str) -> String`（去 scheme/端口/路径，小写）
  - `pub fn match_domain(host: &str, pattern: &str) -> bool`（精确或 `*.` 前缀通配）
  - `pub fn resolve_policy(host: &str, domain_policy: &HashMap<String, BrowserContentPolicy>, global: BrowserContentPolicy) -> BrowserContentPolicy`（最长匹配优先）

- [ ] **Step 1: 写失败测试**

```rust
use super::*;
use crate::agent::types::BrowserContentPolicy;
use std::collections::HashMap;

#[test]
fn extracts_host_from_url() {
    assert_eq!(host_of("https://example.com/a?b=1"), "example.com");
    assert_eq!(host_of("http://127.0.0.1:8080/x"), "127.0.0.1");
    assert_eq!(host_of("example.com"), "example.com");
}

#[test]
fn matches_exact_and_wildcard_domain() {
    assert!(match_domain("example.com", "example.com"));
    assert!(!match_domain("sub.example.com", "example.com"));
    assert!(match_domain("example.com", "*.example.com"));
    assert!(match_domain("a.b.example.com", "*.example.com"));
    assert!(!match_domain("example.org", "*.example.com"));
}

#[test]
fn resolves_policy_with_longest_match() {
    let mut map = HashMap::new();
    map.insert("example.com".to_string(), BrowserContentPolicy::Normal);
    map.insert("*.example.com".to_string(), BrowserContentPolicy::Trusted);
    assert_eq!(resolve_policy("a.example.com", &map, BrowserContentPolicy::Strict), BrowserContentPolicy::Trusted);
    assert_eq!(resolve_policy("example.com", &map, BrowserContentPolicy::Strict), BrowserContentPolicy::Normal);
    assert_eq!(resolve_policy("other.org", &map, BrowserContentPolicy::Strict), BrowserContentPolicy::Strict);
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test -p chatwhale browser::policy`
Expected: FAIL（模块不存在，编译错误）

- [ ] **Step 3: 实现**

```rust
use crate::agent::types::BrowserContentPolicy;
use std::collections::HashMap;

pub fn host_of(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let host_port = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    host_port.split(':').next().unwrap_or("").to_ascii_lowercase()
}

pub fn match_domain(host: &str, pattern: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let pattern = pattern.to_ascii_lowercase();
    if let Some(rest) = pattern.strip_prefix("*.") {
        host == rest || host.ends_with(&format!(".{rest}"))
    } else {
        host == pattern
    }
}

pub fn resolve_policy(
    host: &str,
    domain_policy: &HashMap<String, BrowserContentPolicy>,
    global: BrowserContentPolicy,
) -> BrowserContentPolicy {
    let mut best: Option<(usize, BrowserContentPolicy)> = None;
    for (pattern, level) in domain_policy {
        if match_domain(host, pattern) {
            let len = pattern.len();
            if best.map(|(bl, _)| len > bl).unwrap_or(true) {
                best = Some((len, *level));
            }
        }
    }
    best.map(|(_, l)| l).unwrap_or(global)
}
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cargo test -p chatwhale browser::policy`
Expected: PASS（3 个测试全绿）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/agent/browser/policy.rs
git commit -m "feat(agent): 浏览器内容策略域名匹配与生效级别解析"
```

---

### Task 3: 浏览器可执行文件探测（browser/locator.rs）

**Files:**
- Create: `src-tauri/src/agent/browser/locator.rs`
- Test: 同文件 `#[cfg(test)]`

**Interfaces:**
- Produces:
  - `pub fn default_candidates() -> Vec<PathBuf>`
  - `pub fn locate(browser_path: Option<&str>) -> Option<PathBuf>`（设置优先；未设置时遍历候选，返回第一个存在且为文件的路径）

- [ ] **Step 1: 写失败测试**

```rust
use super::*;

#[test]
fn settings_path_wins_when_exists() {
    let dir = std::env::temp_dir().join(format!("cw-locator-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join("fake-browser");
    std::fs::write(&exe, "#!/bin/sh").unwrap();
    assert_eq!(locate(Some(exe.to_str().unwrap())), Some(exe.clone()));
    assert!(locate(Some(dir.join("missing").to_str().unwrap())).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn candidates_are_non_empty() {
    assert!(!default_candidates().is_empty());
}

#[test]
fn locate_without_settings_returns_existing_file_or_none() {
    let found = locate(None);
    if found.is_some() {
        assert!(found.unwrap().is_file());
    }
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test -p chatwhale browser::locator`
Expected: FAIL（模块不存在）

- [ ] **Step 3: 实现**

```rust
use std::path::PathBuf;

pub fn default_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(target_os = "macos")]
    {
        v.push(PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"));
        v.push(PathBuf::from("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"));
        v.push(PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"));
    }
    #[cfg(target_os = "windows")]
    {
        v.push(PathBuf::from("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"));
        v.push(PathBuf::from("C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe"));
        v.push(PathBuf::from("C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe"));
        v.push(PathBuf::from("C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe"));
    }
    #[cfg(target_os = "linux")]
    {
        v.push(PathBuf::from("/usr/bin/google-chrome"));
        v.push(PathBuf::from("/usr/bin/google-chrome-stable"));
        v.push(PathBuf::from("/usr/bin/chromium"));
        v.push(PathBuf::from("/usr/bin/chromium-browser"));
        v.push(PathBuf::from("/usr/bin/microsoft-edge"));
    }
    v
}

pub fn locate(browser_path: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = browser_path {
        let p = PathBuf::from(p);
        return if p.is_file() { Some(p) } else { None };
    }
    default_candidates().into_iter().find(|p| p.is_file())
}
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cargo test -p chatwhale browser::locator`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/agent/browser/locator.rs
git commit -m "feat(agent): Chrome/Edge 可执行文件探测"
```

---

### Task 4: chromiumoxide CDP 客户端（browser/cdp.rs + Cargo 依赖）

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/agent/browser/cdp.rs`
- Test: 同文件 `#[cfg(test)]`（缺失可执行文件时启动失败；真实浏览器留到 Task 8 集成测试）

**Interfaces:**
- Consumes: `chromiumoxide`（CDP 协议客户端）；`futures-util`（已有）。
- Produces:
  - `pub struct BrowserProcess`：持有 `chromiumoxide::browser::Browser` 与事件排空任务；`pub async fn stop(self)` 关闭浏览器并回收进程
  - `pub async fn launch(executable: &Path, user_data_dir: &Path) -> Result<(BrowserProcess, CdpSession)>`：指定浏览器可执行文件与独立 profile，启动可见窗口并连接 CDP
  - `pub struct CdpSession`：`evaluate(expression: &str) -> Result<serde_json::Value>`、`navigate(url: &str)`、`wait_ready(timeout: Duration)`、`screenshot() -> Result<Vec<u8>>`

- [ ] **Step 1: 添加依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 中追加：

```toml
chromiumoxide = "0.7"
```

Run: `cargo check -p chatwhale`
Expected: 成功解析新依赖（首次联网拉取；若被沙箱拦截，按提示申请批准）

- [ ] **Step 2: 核对 crate 实际 API（必做，未核对不得进入下一步）**

chromiumoxide 0.7.x 小版本的 API 名称偶有差异。拉取后打开本地源码确认下列签名；后续代码若与本地源码不一致，以本地源码为准调整（只允许在 `cdp.rs` 内适配）。

Run: `ls ~/.cargo/registry/src/*/chromiumoxide-0.7.*/src`
Expected: 目录存在。逐项核对：

1. `BrowserConfig::builder()` 及 `.with_user_data_dir(path)` / `.executable(path)` / `.with_arg(arg)` / `.build()`
2. `Browser::launch(config).await` 返回 `(Browser, Handler)`，`Handler` 实现 `Stream`
3. `Browser::new_page(url)` 返回 `Page`
4. `Page::evaluate_expression(expr)` 返回 `JSValue`，其 value 的提取方式（`into_value::<Value>()` 或 `.value()`）
5. `Page::screenshot(...)` 返回的 `Screenshot` 取字节的方式（`bytes` 字段或 `Deref<Target=[u8]>`）
6. `Page` 的导航等待：`wait_for_navigation()` 的签名与返回
7. `Browser` 的关闭/回收：`close()` 或 `kill()` 的实际名称

- [ ] **Step 3: 写失败测试**

```rust
use super::*;

#[tokio::test]
async fn launch_fails_with_missing_executable() {
    let dir = std::env::temp_dir().join(format!("cw-cdp-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = std::env::temp_dir().join("definitely-not-a-browser");
    let result = launch(&exe, &dir).await;
    assert!(result.is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
```

- [ ] **Step 4: 运行测试验证失败**

Run: `cargo test -p chatwhale browser::cdp`
Expected: FAIL（模块/函数不存在）

- [ ] **Step 5: 实现（签名以 Step 2 核对的本地源码为准）**

```rust
use anyhow::{anyhow, Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::Page;
use futures_util::StreamExt;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

pub struct BrowserProcess {
    browser: Browser,
    _handler: tokio::task::JoinHandle<()>,
}

impl BrowserProcess {
    pub async fn stop(self) {
        // 关闭浏览器连接并回收事件任务；浏览器进程由 chromiumoxide 生命周期管理
        let _ = self.browser.close().await;
        self._handler.abort();
    }
}

pub async fn launch(executable: &Path, user_data_dir: &Path) -> Result<(BrowserProcess, CdpSession)> {
    let config = BrowserConfig::builder()
        .with_user_data_dir(user_data_dir)
        .executable(executable)
        .with_arg("--no-first-run")
        .with_arg("--no-default-browser-check")
        .with_arg("--remote-allow-origins=*")
        .build()
        .context("构建浏览器配置失败")?;
    let (browser, mut handler) = Browser::launch(config).await.context("启动浏览器失败")?;
    let handler_task = tokio::spawn(async move {
        while handler.next().await.is_some() {}
    });
    let page = browser
        .new_page("about:blank")
        .await
        .context("创建页面失败")?;
    Ok((
        BrowserProcess { browser, _handler: handler_task },
        CdpSession { page },
    ))
}

pub struct CdpSession {
    page: Page,
}

impl CdpSession {
    pub async fn evaluate(&mut self, expression: &str) -> Result<Value> {
        let js = self
            .page
            .evaluate_expression(expression)
            .await
            .context("页面 JS 执行失败")?;
        let v = js
            .into_value::<Value>()
            .map_err(|_| anyhow!("页面 JS 返回值解析失败"))?;
        Ok(v)
    }

    pub async fn navigate(&mut self, url: &str) -> Result<()> {
        self.page.navigate(url).await.context("导航失败")?;
        let _ = self.page.wait_for_navigation().await;
        self.wait_ready(Duration::from_secs(30)).await
    }

    pub async fn wait_ready(&mut self, timeout: Duration) -> Result<()> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow!("页面加载超时"));
            }
            let v = self.evaluate("document.readyState").await?;
            if v.as_str() == Some("complete") {
                // 给 SPA 首帧渲染留一个宏任务窗口
                let _ = self.evaluate("new Promise(r => setTimeout(r, 300))").await;
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    pub async fn screenshot(&mut self) -> Result<Vec<u8>> {
        let shot = self
            .page
            .screenshot(Default::default())
            .await
            .context("截图失败")?;
        Ok(shot.bytes.to_vec())
    }
}
```

说明：若 Step 2 核对后发现 `evaluate_expression` 的返回值提取、`screenshot` 的字节字段、`Browser` 关闭方法的名称与上述不同，按本地源码调整这几行即可；**不要改动 `BrowserManager`（Task 7）对本文件接口的调用方式**。

- [ ] **Step 6: 运行测试验证通过**

Run: `cargo test -p chatwhale browser::cdp`
Expected: PASS（缺失可执行文件时 `launch` 返回 Err；crate 编译通过）

- [ ] **Step 7: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/agent/browser/cdp.rs
git commit -m "feat(agent): chromiumoxide CDP 客户端（启动/求值/导航/截图）"
```

---

### Task 5: 页面快照与策略化提取（browser/extract.rs）

**Files:**
- Create: `src-tauri/src/agent/browser/extract.rs`
- Test: 同文件 `#[cfg(test)]`

**Interfaces:**
- Consumes: `crate::agent::types::BrowserContentPolicy`
- Produces:
  - `pub const SNAPSHOT_JS: &str`（页面内执行，返回九类数据）
  - `pub struct PageSnapshot`（`Deserialize`，`#[serde(rename_all = "camelCase")]`，字段：title/url/article_text/body_text/links/alts/form_values/data_attrs/hidden_text/script_text）
  - `pub struct LinkItem { pub text: String, pub href: String }`、`pub struct FormValue { pub name: String, pub value: String }`、`pub struct AttrItem { pub name: String, pub value: String }`
  - `pub fn render_snapshot(snap: &PageSnapshot, policy: BrowserContentPolicy, mode: &str) -> String`（mode：text/markdown/links）

- [ ] **Step 1: 写失败测试**

```rust
use super::*;
use crate::agent::types::BrowserContentPolicy;

fn fixture() -> PageSnapshot {
    PageSnapshot {
        title: "Test".into(),
        url: "https://example.com/".into(),
        article_text: "正文".into(),
        body_text: "正文\n导航".into(),
        links: vec![LinkItem { text: "官网".into(), href: "https://example.com/".into() }],
        alts: vec!["示意图".into()],
        form_values: vec![FormValue { name: "token".into(), value: "abc123".into() }],
        data_attrs: vec![AttrItem { name: "data-id".into(), value: "42".into() }],
        hidden_text: "隐藏段".into(),
        script_text: "var x=1;".into(),
    }
}

#[test]
fn strict_renders_article_text_without_sensitive_classes() {
    let out = render_snapshot(&fixture(), BrowserContentPolicy::Strict, "text");
    assert!(out.contains("正文"));
    assert!(!out.contains("abc123"));
    assert!(!out.contains("var x=1;"));
}

#[test]
fn normal_adds_links_and_alts_but_not_form_values() {
    let out = render_snapshot(&fixture(), BrowserContentPolicy::Normal, "text");
    assert!(out.contains("官网"));
    assert!(out.contains("示意图"));
    assert!(!out.contains("abc123"));
}

#[test]
fn trusted_adds_form_values_attributes_hidden_and_scripts() {
    let out = render_snapshot(&fixture(), BrowserContentPolicy::Trusted, "text");
    assert!(out.contains("abc123"));
    assert!(out.contains("data-id = 42"));
    assert!(out.contains("隐藏段"));
    assert!(out.contains("var x=1;"));
}

#[test]
fn strict_links_mode_omits_urls() {
    let out = render_snapshot(&fixture(), BrowserContentPolicy::Strict, "links");
    assert!(out.contains("官网"));
    assert!(!out.contains("https://"));
}

#[test]
fn markdown_mode_formats_links_with_url_when_allowed() {
    let out = render_snapshot(&fixture(), BrowserContentPolicy::Normal, "markdown");
    assert!(out.contains("[官网](https://example.com/)"));
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test -p chatwhale browser::extract`
Expected: FAIL（模块不存在）

- [ ] **Step 3: 实现**

```rust
use crate::agent::types::BrowserContentPolicy;
use serde::Deserialize;

/// 页面内采集结构化快照；字段名与 PageSnapshot 的 camelCase 对应。
pub const SNAPSHOT_JS: &str = r#"
(() => {
  const collectVisible = (root) => (root && root.innerText) ? root.innerText : "";
  const main = document.querySelector("article") || document.querySelector("main");
  const links = [];
  document.querySelectorAll("a[href]").forEach((a) => {
    const text = (a.innerText || "").trim().slice(0, 200);
    const href = (a.getAttribute("href") || "").slice(0, 1000);
    if (text || href) links.push({ text, href });
  });
  const alts = [];
  document.querySelectorAll("img[alt]").forEach((img) => {
    const alt = (img.getAttribute("alt") || "").trim().slice(0, 500);
    if (alt) alts.push(alt);
  });
  const formValues = [];
  document.querySelectorAll("input, textarea, select").forEach((el) => {
    const type = (el.getAttribute("type") || "text").toLowerCase();
    if (type === "password" || type === "hidden") return;
    const name = el.getAttribute("name") || el.getAttribute("id") || "";
    const value = el.value;
    if (name && value) formValues.push({ name: name.slice(0, 100), value: String(value).slice(0, 500) });
  });
  const dataAttrs = [];
  document.querySelectorAll("[data-]").forEach((el) => {
    if (dataAttrs.length >= 200) return;
    for (const attr of el.attributes) {
      if (attr.name.startsWith("data-") && attr.value) {
        dataAttrs.push({ name: attr.name.slice(0, 100), value: attr.value.slice(0, 500) });
        if (dataAttrs.length >= 200) break;
      }
    }
  });
  let hiddenText = "";
  document.querySelectorAll("body *").forEach((el) => {
    if (hiddenText.length >= 20000) return;
    const st = getComputedStyle(el);
    if (st.display === "none" || st.visibility === "hidden") {
      const t = (el.textContent || "").trim();
      if (t) hiddenText += t + "\n";
    }
  });
  let scriptText = "";
  document.querySelectorAll("script, style").forEach((el) => {
    if (scriptText.length >= 20000) return;
    const t = (el.textContent || "").trim();
    if (t) scriptText += t + "\n";
  });
  return {
    title: document.title || "",
    url: location.href,
    articleText: collectVisible(main) || collectVisible(document.body),
    bodyText: collectVisible(document.body),
    links: links.slice(0, 200),
    alts: alts.slice(0, 200),
    formValues: formValues.slice(0, 200),
    dataAttrs,
    hiddenText,
    scriptText,
  };
})()
"#;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSnapshot {
    pub title: String,
    pub url: String,
    pub article_text: String,
    pub body_text: String,
    pub links: Vec<LinkItem>,
    pub alts: Vec<String>,
    pub form_values: Vec<FormValue>,
    pub data_attrs: Vec<AttrItem>,
    pub hidden_text: String,
    pub script_text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinkItem {
    pub text: String,
    pub href: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormValue {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttrItem {
    pub name: String,
    pub value: String,
}

pub fn render_snapshot(snap: &PageSnapshot, policy: BrowserContentPolicy, mode: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", snap.title));
    out.push_str(&format!("URL: {}\n\n", snap.url));
    match mode {
        "markdown" => render_markdown(snap, policy, &mut out),
        "links" => render_links(snap, policy, &mut out),
        _ => render_text(snap, policy, &mut out),
    }
    out
}

fn render_text(snap: &PageSnapshot, policy: BrowserContentPolicy, out: &mut String) {
    match policy {
        BrowserContentPolicy::Strict => out.push_str(&snap.article_text),
        _ => {
            out.push_str(&snap.body_text);
            if !snap.links.is_empty() {
                out.push_str("\n\n## 链接\n");
                for l in &snap.links {
                    out.push_str(&format!("- {}: {}\n", l.text, l.href));
                }
            }
            if !snap.alts.is_empty() {
                out.push_str("\n## 图片说明\n");
                for a in &snap.alts {
                    out.push_str(&format!("- {a}\n"));
                }
            }
            if policy == BrowserContentPolicy::Trusted {
                if !snap.form_values.is_empty() {
                    out.push_str("\n## 表单值\n");
                    for f in &snap.form_values {
                        out.push_str(&format!("- {} = {}\n", f.name, f.value));
                    }
                }
                if !snap.data_attrs.is_empty() {
                    out.push_str("\n## data 属性\n");
                    for d in &snap.data_attrs {
                        out.push_str(&format!("- {} = {}\n", d.name, d.value));
                    }
                }
                if !snap.hidden_text.is_empty() {
                    out.push_str("\n## 隐藏文本\n");
                    out.push_str(&snap.hidden_text);
                    out.push('\n');
                }
                if !snap.script_text.is_empty() {
                    out.push_str("\n## 脚本/样式文本\n");
                    out.push_str(&snap.script_text);
                }
            }
        }
    }
}

fn render_links(snap: &PageSnapshot, policy: BrowserContentPolicy, out: &mut String) {
    for l in &snap.links {
        if policy >= BrowserContentPolicy::Normal {
            out.push_str(&format!("- {}: {}\n", l.text, l.href));
        } else {
            out.push_str(&format!("- {}\n", l.text));
        }
    }
}

fn render_markdown(snap: &PageSnapshot, policy: BrowserContentPolicy, out: &mut String) {
    let body = match policy {
        BrowserContentPolicy::Strict => &snap.article_text,
        _ => &snap.body_text,
    };
    for para in body.split("\n\n") {
        let p = para.trim();
        if !p.is_empty() {
            out.push_str(p);
            out.push_str("\n\n");
        }
    }
    if !snap.links.is_empty() {
        out.push_str("## 链接\n");
        for l in &snap.links {
            if policy >= BrowserContentPolicy::Normal {
                out.push_str(&format!("- [{}]({})\n", l.text, l.href));
            } else {
                out.push_str(&format!("- {}\n", l.text));
            }
        }
    }
    if policy == BrowserContentPolicy::Trusted && !snap.form_values.is_empty() {
        out.push_str("\n## 表单值\n");
        for f in &snap.form_values {
            out.push_str(&format!("- {} = {}\n", f.name, f.value));
        }
    }
}
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cargo test -p chatwhale browser::extract`
Expected: PASS（5 个测试全绿）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/agent/browser/extract.rs
git commit -m "feat(agent): 页面快照与按内容策略提取"
```

---

### Task 6: 审批协议扩展（approval.rs + lib.rs + 前端类型）

**Files:**
- Modify: `src-tauri/src/agent/approval.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/types/index.ts`
- Modify: `src/composables/useAgent.ts`
- Test: `src-tauri/src/agent/approval.rs`（模块内测试）；`npm test`（前端现有测试）

**Interfaces:**
- Consumes: `crate::agent::types::BrowserContentPolicy`
- Produces:
  - `pub struct ApprovalResult { pub outcome: ApprovalOutcome, pub level: Option<String> }`
  - `ApprovalManager::request_with_choices(app, window_label, tool_name, command, policy, timeout, cancellation, choices: &[BrowserContentPolicy]) -> ApprovalResult`
  - `ApprovalManager::resolve_with_level(id, approved, level: Option<String>) -> bool`
  - `pub fn resolve_global_with_level(id: &str, approved: bool, level: Option<String>) -> bool`
  - `ApprovalPayload` 增加可选 `choices: Option<Vec<ApprovalChoice>>`（`ApprovalChoice { level: String, label: String }`）
  - Tauri command `agent_approve(id: String, approved: bool, level: Option<String>)`
  - 前端 `approveCommand(id, approved, level?)`；`ApprovalRequest.choices`

- [ ] **Step 1: 写失败测试（approval.rs）**

```rust
#[tokio::test]
async fn resolve_with_level_returns_level() {
    let mgr = ApprovalManager::new();
    let (tx, rx) = tokio::sync::oneshot::channel();
    mgr.pending
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert("a1".into(), tx);
    assert!(mgr.resolve_with_level("a1", true, Some("trusted".into())));
    let reply = rx.await.unwrap();
    assert!(reply.approved);
    assert_eq!(reply.level.as_deref(), Some("trusted"));
    assert!(!mgr.resolve_with_level("missing", true, None));
}
```

（`ApprovalReply` 为私有类型；测试位于同模块，可访问。）

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test -p chatwhale agent::approval`
Expected: FAIL（`resolve_with_level` / `ApprovalReply` 未定义）

- [ ] **Step 3: 实现 approval.rs**

顶部 `use` 追加：`use crate::agent::types::BrowserContentPolicy;`

新增类型与 payload 字段：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalResult {
    pub outcome: ApprovalOutcome,
    pub level: Option<String>,
}

#[derive(Clone)]
struct ApprovalReply {
    approved: bool,
    level: Option<String>,
}

#[derive(Serialize, Clone)]
struct ApprovalChoice {
    level: String,
    label: String,
}
```

`ApprovalPayload` 追加字段：

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    choices: Option<Vec<ApprovalChoice>>,
```

`pending` 字段类型改为：

```rust
    pending: Mutex<HashMap<String, oneshot::Sender<ApprovalReply>>>,
```

将原 `request` 替换为（保留 `request` 的旧签名，内部转调新方法）：

```rust
    pub async fn request(
        &self,
        app: &AppHandle,
        window_label: Option<&str>,
        tool_name: &str,
        command: &str,
        policy: &str,
        timeout: std::time::Duration,
        cancellation: CancellationToken,
    ) -> ApprovalOutcome {
        self.request_with_choices(app, window_label, tool_name, command, policy, timeout, cancellation, &[])
            .await
            .outcome
    }

    pub async fn request_with_choices(
        &self,
        app: &AppHandle,
        window_label: Option<&str>,
        tool_name: &str,
        command: &str,
        policy: &str,
        timeout: std::time::Duration,
        cancellation: CancellationToken,
        choices: &[BrowserContentPolicy],
    ) -> ApprovalResult {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        let choices_payload = if choices.is_empty() {
            None
        } else {
            Some(
                choices
                    .iter()
                    .map(|c| ApprovalChoice {
                        level: c.as_str().to_string(),
                        label: match c {
                            BrowserContentPolicy::Normal => "允许并放宽到 normal".to_string(),
                            BrowserContentPolicy::Trusted => "允许并放宽到 trusted".to_string(),
                            BrowserContentPolicy::Strict => "允许（strict）".to_string(),
                        },
                    })
                    .collect(),
            )
        };
        self.pending
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id.clone(), tx);
        let _ = emit_agent_event(
            app,
            window_label,
            super::EVENT_APPROVAL_REQUEST,
            ApprovalPayload {
                id: &id,
                tool_name,
                command,
                policy,
                choices: choices_payload,
            },
        );

        tokio::select! {
            _ = cancellation.cancelled() => {
                self.pending.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
                ApprovalResult { outcome: ApprovalOutcome::Cancelled, level: None }
            }
            _ = tokio::time::sleep(timeout) => {
                self.pending.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
                ApprovalResult { outcome: ApprovalOutcome::Timeout, level: None }
            }
            v = rx => match v {
                Ok(reply) => ApprovalResult {
                    outcome: if reply.approved {
                        ApprovalOutcome::Granted
                    } else {
                        ApprovalOutcome::Rejected("用户拒绝".into())
                    },
                    level: reply.level,
                },
                Err(_) => ApprovalResult { outcome: ApprovalOutcome::Timeout, level: None },
            }
        }
    }
```

`resolve` 替换为：

```rust
    pub fn resolve_with_level(&self, id: &str, approved: bool, level: Option<String>) -> bool {
        let sender = self
            .pending
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id);
        match sender {
            Some(tx) => {
                let _ = tx.send(ApprovalReply { approved, level });
                true
            }
            None => false,
        }
    }

    pub fn resolve(&self, id: &str, approved: bool) -> bool {
        self.resolve_with_level(id, approved, None)
    }
```

`resolve_global` 之后追加：

```rust
pub fn resolve_global_with_level(id: &str, approved: bool, level: Option<String>) -> bool {
    global_manager().resolve_with_level(id, approved, level)
}
```

- [ ] **Step 4: 修改 lib.rs 的 agent_approve**

```rust
#[tauri::command]
async fn agent_approve(id: String, approved: bool, level: Option<String>) -> Result<(), String> {
    if approval::resolve_global_with_level(&id, approved, level) {
        Ok(())
    } else {
        Err(format!("审批请求 {id} 不存在或已超时"))
    }
}
```

- [ ] **Step 5: 修改前端类型与调用**

`src/types/index.ts` 的 `ApprovalRequest` 追加：

```ts
  choices?: { level: string; label: string }[];
```

`src/composables/useAgent.ts` 的 `ToolResultPayload` 接口追加：

```ts
  image_path?: string | null;
```

`approveCommand` 改为：

```ts
  async function approveCommand(id: string, approved: boolean, level?: string) {
    pendingApproval.value = null;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("agent_approve", { id, approved, level: level ?? null });
    } catch (err) {
      agentError.value = String(err);
    }
  }
```

- [ ] **Step 6: 运行测试验证通过**

Run: `cargo test -p chatwhale agent::approval && npm test`
Expected: PASS（新增 resolve 测试 + 前端现有 vitest 全绿）

- [ ] **Step 7: 提交**

```bash
git add src-tauri/src/agent/approval.rs src-tauri/src/lib.rs src/types/index.ts src/composables/useAgent.ts
git commit -m "feat(agent): 审批协议支持临时放宽级别（agent_approve 增加 level）"
```

---

### Task 7: BrowserManager 与工具回路接线

**Files:**
- Create: `src-tauri/src/agent/browser/mod.rs`
- Modify: `src-tauri/src/agent/tools.rs`（ToolContext 字段）
- Modify: `src-tauri/src/agent/types.rs`（ToolResult.image_path）
- Modify: `src-tauri/src/agent/mod.rs`（session_policy 注入 + 事件携带 image_path）
- Modify: `src-tauri/src/agent/mcp/mod.rs`（ToolResult 字面量）
- Modify: `src-tauri/src/lib.rs`（setup 中 manage BrowserManager）
- Test: `src-tauri/src/agent/browser/mod.rs`（close 幂等）

**Interfaces:**
- Consumes: `browser::cdp`、`browser::locator`、`browser::extract::PageSnapshot`、`crate::agent::types::*`
- Produces:
  - `pub struct BrowserManager { base_dir: PathBuf, sessions: tokio::sync::Mutex<HashMap<String, BrowserSession>> }`，`pub fn new(base_dir: PathBuf) -> Self`
  - 方法：`open(workspace_id, settings, url) -> Result<PageSnapshot>`、`read(workspace_id, settings, selector: Option<&str>, timeout_ms: Option<u64>) -> Result<PageSnapshot>`、`click(workspace_id, settings, selector: Option<&str>, text: Option<&str>) -> Result<PageSnapshot>`、`fill(workspace_id, settings, selector, value) -> Result<String>`、`scroll(workspace_id, settings, direction, amount) -> Result<String>`、`screenshot(workspace_id, settings, path: Option<&str>) -> Result<PathBuf>`、`close(workspace_id) -> Result<()>`
  - `ToolContext` 增加 `workspace_id: &'a str` 与 `session_policy: Arc<tokio::sync::Mutex<Option<BrowserContentPolicy>>>`
  - `ToolResult` 增加 `#[serde(default)] pub image_path: Option<String>`
  - `mod.rs` 创建 session_policy 并注入；`agent-tool-result` 事件携带 `image_path`

- [ ] **Step 1: 写失败测试（browser/mod.rs）**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn close_missing_session_is_ok() {
        let dir = std::env::temp_dir().join(format!("cw-bm-{}", uuid::Uuid::new_v4()));
        let bm = BrowserManager::new(dir.clone());
        assert!(bm.close("nope").await.is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test -p chatwhale browser::mod`
Expected: FAIL（模块不存在）

- [ ] **Step 3: 实现 browser/mod.rs**

```rust
pub mod cdp;
pub mod extract;
pub mod locator;
pub mod policy;

use crate::agent::browser::cdp::{self, BrowserProcess, CdpSession};
use crate::agent::browser::extract::{PageSnapshot, SNAPSHOT_JS};
use crate::agent::browser::locator;
use crate::agent::tools::resolve_workspace_path;
use crate::agent::types::AgentSettings;
use anyhow::{anyhow, Context, Result};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::Mutex;

pub struct BrowserSession {
    process: BrowserProcess,
    cdp: CdpSession,
}

pub struct BrowserManager {
    base_dir: PathBuf,
    sessions: Mutex<HashMap<String, BrowserSession>>,
}

impl BrowserManager {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    async fn ensure_session(&self, workspace_id: &str, browser_path: Option<&str>) -> Result<()> {
        let mut sessions = self.sessions.lock().await;
        if sessions.contains_key(workspace_id) {
            return Ok(());
        }
        let exe = locator::locate(browser_path)
            .ok_or_else(|| anyhow!("未找到 Chrome/Edge，请安装或配置 agent.browser_path"))?;
        let profile = self.base_dir.join("profiles").join(workspace_id);
        std::fs::create_dir_all(&profile).context("创建浏览器 profile 目录失败")?;
        let (process, cdp) = cdp::launch(&exe, &profile).await?;
        sessions.insert(
            workspace_id.to_string(),
            BrowserSession { process, cdp },
        );
        Ok(())
    }

    async fn snapshot(session: &mut BrowserSession) -> Result<PageSnapshot> {
        let v = session.cdp.evaluate(SNAPSHOT_JS).await?;
        serde_json::from_value(v).context("页面快照解析失败")
    }

    pub async fn open(
        &self,
        workspace_id: &str,
        settings: &AgentSettings,
        url: &str,
    ) -> Result<PageSnapshot> {
        self.ensure_session(workspace_id, settings.browser_path.as_deref()).await?;
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(workspace_id)
            .ok_or_else(|| anyhow!("浏览器会话不存在"))?;
        session.cdp.navigate(url).await?;
        Self::snapshot(session).await
    }

    pub async fn read(
        &self,
        workspace_id: &str,
        settings: &AgentSettings,
        selector: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<PageSnapshot> {
        self.ensure_session(workspace_id, settings.browser_path.as_deref()).await?;
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(workspace_id)
            .ok_or_else(|| anyhow!("浏览器会话不存在"))?;
        if let Some(sel) = selector {
            let timeout = timeout_ms.unwrap_or(3000).min(30_000);
            let js = format!(
                r#"new Promise((resolve) => {{
  const sel = {};
  const t0 = Date.now();
  const iv = setInterval(() => {{
    const el = document.querySelector(sel);
    if (el || Date.now() - t0 > {}) {{
      clearInterval(iv);
      const found = document.querySelector(sel);
      resolve({{ ok: !!found, text: found ? found.innerText || "" : "", title: document.title || "", url: location.href }});
    }}
  }}, 200);
}})"#,
                serde_json::json!(sel),
                timeout
            );
            let v = session.cdp.evaluate(&js).await?;
            if v["ok"].as_bool() != Some(true) {
                return Err(anyhow!("选择器 {sel} 在 {timeout}ms 内未出现"));
            }
            return serde_json::from_value(json!({
                "title": v["title"],
                "url": v["url"],
                "articleText": v["text"],
                "bodyText": v["text"],
                "links": [],
                "alts": [],
                "formValues": [],
                "dataAttrs": [],
                "hiddenText": "",
                "scriptText": "",
            }))
            .context("选择器快照解析失败");
        }
        Self::snapshot(session).await
    }

    pub async fn click(
        &self,
        workspace_id: &str,
        settings: &AgentSettings,
        selector: Option<&str>,
        text: Option<&str>,
    ) -> Result<PageSnapshot> {
        self.ensure_session(workspace_id, settings.browser_path.as_deref()).await?;
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(workspace_id)
            .ok_or_else(|| anyhow!("浏览器会话不存在"))?;
        let js = format!(
            r#"(() => {{
  const SEL = {};
  const TXT = {};
  const target = SEL
    ? document.querySelector(SEL)
    : [...document.querySelectorAll("a,button,input[type=submit],input[type=button],[role=button]")]
        .find((el) => (el.innerText || "").trim().includes(TXT)) || null;
  if (!target) return {{ ok: false, reason: "not found" }};
  target.scrollIntoView({{ block: "center" }});
  target.click();
  return {{ ok: true }};
}})()"#,
            serde_json::json!(selector),
            serde_json::json!(text.unwrap_or(""))
        );
        let v = session.cdp.evaluate(&js).await?;
        if v["ok"].as_bool() != Some(true) {
            return Err(anyhow!("未找到可点击元素（selector={selector:?}, text={text:?}）"));
        }
        session.cdp.wait_ready(Duration::from_secs(15)).await?;
        Self::snapshot(session).await
    }

    pub async fn fill(
        &self,
        workspace_id: &str,
        settings: &AgentSettings,
        selector: &str,
        value: &str,
    ) -> Result<String> {
        self.ensure_session(workspace_id, settings.browser_path.as_deref()).await?;
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(workspace_id)
            .ok_or_else(|| anyhow!("浏览器会话不存在"))?;
        let js = format!(
            r#"(() => {{
  const el = document.querySelector({});
  if (!el) return {{ ok: false, reason: "not found" }};
  const type = (el.getAttribute("type") || "text").toLowerCase();
  if (type === "password") return {{ ok: false, reason: "password field forbidden" }};
  el.focus();
  el.value = {};
  el.dispatchEvent(new Event("input", {{ bubbles: true }}));
  el.dispatchEvent(new Event("change", {{ bubbles: true }}));
  return {{ ok: true }};
}})()"#,
            serde_json::json!(selector),
            serde_json::json!(value)
        );
        let v = session.cdp.evaluate(&js).await?;
        if v["ok"].as_bool() != Some(true) {
            let reason = v["reason"].as_str().unwrap_or("unknown");
            return Err(anyhow!("填表失败: {reason}"));
        }
        Ok(format!("已填写 {selector}"))
    }

    pub async fn scroll(
        &self,
        workspace_id: &str,
        settings: &AgentSettings,
        direction: &str,
        amount: Option<i64>,
    ) -> Result<String> {
        self.ensure_session(workspace_id, settings.browser_path.as_deref()).await?;
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(workspace_id)
            .ok_or_else(|| anyhow!("浏览器会话不存在"))?;
        let amt = amount.unwrap_or(600);
        let js = format!(
            r#"(() => {{
  const DIR = {};
  const AMT = {};
  if (DIR === "top") window.scrollTo(0, 0);
  else if (DIR === "bottom") window.scrollTo(0, document.body.scrollHeight);
  else if (DIR === "down") window.scrollBy(0, AMT);
  else if (DIR === "up") window.scrollBy(0, -AMT);
  return {{ x: window.scrollX, y: window.scrollY }};
}})()"#,
            serde_json::json!(direction),
            amt
        );
        let v = session.cdp.evaluate(&js).await?;
        Ok(format!("已滚动到 ({}, {})", v["x"], v["y"]))
    }

    pub async fn screenshot(
        &self,
        workspace_id: &str,
        settings: &AgentSettings,
        path: Option<&str>,
    ) -> Result<PathBuf> {
        self.ensure_session(workspace_id, settings.browser_path.as_deref()).await?;
        let bytes = {
            let mut sessions = self.sessions.lock().await;
            let session = sessions
                .get_mut(workspace_id)
                .ok_or_else(|| anyhow!("浏览器会话不存在"))?;
            session.cdp.screenshot().await?
        };
        let dir = self.base_dir.join("screenshots").join(workspace_id);
        std::fs::create_dir_all(&dir).context("创建截图目录失败")?;
        let name = format!("{}.png", uuid::Uuid::new_v4());
        let app_path = dir.join(&name);
        std::fs::write(&app_path, &bytes).context("写入截图失败")?;
        if let Some(p) = path {
            let resolved = resolve_workspace_path(settings, p)?;
            if let Some(parent) = resolved.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::copy(&app_path, &resolved).context("复制截图到 workspace 失败")?;
            Ok(resolved)
        } else {
            Ok(app_path)
        }
    }

    pub async fn close(&self, workspace_id: &str) -> Result<()> {
        let mut sessions = self.sessions.lock().await;
        if let Some(s) = sessions.remove(workspace_id) {
            s.process.stop().await;
        }
        Ok(())
    }
}
```

说明：`tools` 模块声明放在 Task 8 创建文件后再加；本任务 `pub mod tools;` 会因文件不存在编译失败，因此**本任务先不声明 `pub mod tools;`**，Task 8 再加。

- [ ] **Step 4: ToolResult 增加 image_path（types.rs）**

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub content: String,
    #[serde(default)]
    pub image_path: Option<String>,
}

impl ToolResult {
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            content: format!("Error: {}", msg.into()),
            image_path: None,
        }
    }
}
```

`src-tauri/src/agent/tools.rs` 中所有 `ToolResult { success: ..., content: ... }` 字面量补 `image_path: None`（共 4 处：ReadFileTool、WriteFileTool、ListDirectoryTool、SearchFilesTool、ExecuteCommandTool 各 1 处 success 字面量）。

`src-tauri/src/agent/mcp/mod.rs` 两处 `ToolResult { ... }` 字面量补 `image_path: None`。

- [ ] **Step 5: ToolContext 扩展（tools.rs）**

```rust
pub struct ToolContext<'a> {
    pub app: &'a AppHandle,
    pub window_label: Option<&'a str>,
    pub settings: &'a AgentSettings,
    pub approval: &'a super::approval::ApprovalManager,
    pub cancellation: CancellationToken,
    pub workspace_id: &'a str,
    pub session_policy: Arc<tokio::sync::Mutex<Option<BrowserContentPolicy>>>,
}
```

顶部 `use crate::agent::types::BrowserContentPolicy;` 并确认 `Arc` 已导入（`std::sync::Arc` 已有）。

- [ ] **Step 6: mod.rs 注入 session_policy 与事件 image_path**

`src-tauri/src/agent/mod.rs` 的 `run_agent_inner` 中，`let ctx = ToolContext {` 之前加：

```rust
let session_policy = Arc::new(tokio::sync::Mutex::new(None::<BrowserContentPolicy>));
```

`ToolContext` 字面量改为：

```rust
let ctx = ToolContext {
    app,
    window_label: Some(window_label),
    settings,
    approval,
    cancellation: runtime.cancellation.clone(),
    workspace_id,
    session_policy: session_policy.clone(),
};
```

`EVENT_TOOL_RESULT` 的 json 增加：

```rust
let image_path = result.image_path.clone();
"image_path": image_path,
```

顶部 import 追加：`use crate::agent::types::BrowserContentPolicy;`（`types::*` 已导入则无需）。

- [ ] **Step 7: lib.rs setup 中 manage BrowserManager**

顶部追加：`use crate::agent::browser::BrowserManager;` 与 `use tauri::Manager;`

`tauri::Builder::default()` 链上、`.invoke_handler` 之前加：

```rust
        .setup(|app| {
            let base = app.path().app_data_dir()?.join("browser-profiles");
            app.manage(BrowserManager::new(base));
            Ok(())
        })
```

- [ ] **Step 8: 运行测试验证通过**

Run: `cargo test -p chatwhale browser::tests && cargo test -p chatwhale`
Expected: PASS（close 幂等测试通过；全量 crate 测试编译通过）

- [ ] **Step 9: 提交**

```bash
git add src-tauri/src/agent/browser/mod.rs src-tauri/src/agent/tools.rs src-tauri/src/agent/types.rs src-tauri/src/agent/mod.rs src-tauri/src/agent/mcp/mod.rs src-tauri/src/lib.rs
git commit -m "feat(agent): BrowserManager 生命周期与工具回路接线（ToolResult 携带 image_path）"
```

---

### Task 8: 浏览器工具集与集成测试

**Files:**
- Create: `src-tauri/src/agent/browser/tools.rs`
- Modify: `src-tauri/src/agent/browser/mod.rs`（追加 `pub mod tools;`）
- Modify: `src-tauri/src/agent/tools.rs`（注册浏览器工具）
- Create: `src-tauri/tests/browser_integration.rs`
- Test: `src-tauri/tests/browser_integration.rs`（真实浏览器，找不到则 skip）

**Interfaces:**
- Consumes: `BrowserManager` 全部方法、`extract::render_snapshot`、`policy::{host_of, resolve_policy}`、`types::*`、`tools::{Tool, ToolContext, ToolResult}`
- Produces:
  - 7 个 `Tool` 实现：`browser_open` / `browser_read` / `browser_click` / `browser_fill` / `browser_scroll` / `browser_screenshot` / `browser_close`
  - `pub fn effective_policy(session_level: Option<BrowserContentPolicy>, settings: &AgentSettings, url: &str) -> BrowserContentPolicy`（可单测）
  - `tools.rs` 的 `with_builtins` 在 `settings.browser_enabled` 时注册以上工具

- [ ] **Step 1: 写失败测试（tools.rs 内 effective_policy）**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::BrowserContentPolicy;
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
```

- [ ] **Step 2: 实现 browser/tools.rs（完整内容）**

```rust
use crate::agent::approval::ApprovalOutcome;
use crate::agent::browser::BrowserManager;
use crate::agent::browser::extract::render_snapshot;
use crate::agent::browser::policy::{host_of, resolve_policy};
use crate::agent::tools::{resolve_workspace_path, Tool, ToolContext, ToolResult};
use crate::agent::types::{AgentSettings, BrowserApproval, BrowserContentPolicy};
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
    ctx.session_policy
        .lock()
        .ok()
        .and_then(|g| *g)
}

fn render_result(snap: &crate::agent::browser::extract::PageSnapshot, policy: BrowserContentPolicy, mode: &str) -> String {
    format!("{UNTRUSTED_MARKER}\n{}", render_snapshot(snap, policy, mode))
}

struct BrowserOpenTool {
    settings: Arc<AgentSettings>,
}

impl BrowserOpenTool {
    fn new(s: &AgentSettings) -> Self {
        Self { settings: Arc::new(s.clone()) }
    }
}

#[async_trait]
impl Tool for BrowserOpenTool {
    fn name(&self) -> &str { "browser_open" }
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
                    if let Ok(mut sp) = ctx.session_policy.lock() {
                        *sp = Some(crate::agent::types::parse_browser_policy(&level));
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

struct BrowserReadTool {
    settings: Arc<AgentSettings>,
}

impl BrowserReadTool {
    fn new(s: &AgentSettings) -> Self {
        Self { settings: Arc::new(s.clone()) }
    }
}

#[async_trait]
impl Tool for BrowserReadTool {
    fn name(&self) -> &str { "browser_read" }
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

struct BrowserClickTool {
    settings: Arc<AgentSettings>,
}

impl BrowserClickTool {
    fn new(s: &AgentSettings) -> Self {
        Self { settings: Arc::new(s.clone()) }
    }
}

#[async_trait]
impl Tool for BrowserClickTool {
    fn name(&self) -> &str { "browser_click" }
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
        if self.settings.browser_approval == BrowserApproval::Always {
            match ctx
                .approval
                .request(
                    ctx.app,
                    ctx.window_label,
                    "browser_click",
                    "点击浏览器页面元素（always 审批策略）",
                    "browser_operate",
                    ctx.settings.approval_timeout,
                    ctx.cancellation.clone(),
                )
                .await
            {
                ApprovalOutcome::Granted => {}
                ApprovalOutcome::Rejected(r) => return ToolResult::error(format!("用户拒绝: {r}")),
                ApprovalOutcome::Timeout => return ToolResult::error("审批超时，未执行"),
                ApprovalOutcome::Cancelled => return ToolResult::error("审批流程已取消"),
            }
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

struct BrowserFillTool {
    settings: Arc<AgentSettings>,
}

impl BrowserFillTool {
    fn new(s: &AgentSettings) -> Self {
        Self { settings: Arc::new(s.clone()) }
    }
}

#[async_trait]
impl Tool for BrowserFillTool {
    fn name(&self) -> &str { "browser_fill" }
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
        if self.settings.browser_approval == BrowserApproval::Always {
            match ctx
                .approval
                .request(
                    ctx.app,
                    ctx.window_label,
                    "browser_fill",
                    "填写浏览器表单字段（always 审批策略）",
                    "browser_operate",
                    ctx.settings.approval_timeout,
                    ctx.cancellation.clone(),
                )
                .await
            {
                ApprovalOutcome::Granted => {}
                ApprovalOutcome::Rejected(r) => return ToolResult::error(format!("用户拒绝: {r}")),
                ApprovalOutcome::Timeout => return ToolResult::error("审批超时，未执行"),
                ApprovalOutcome::Cancelled => return ToolResult::error("审批流程已取消"),
            }
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

struct BrowserScrollTool {
    settings: Arc<AgentSettings>,
}

impl BrowserScrollTool {
    fn new(s: &AgentSettings) -> Self {
        Self { settings: Arc::new(s.clone()) }
    }
}

#[async_trait]
impl Tool for BrowserScrollTool {
    fn name(&self) -> &str { "browser_scroll" }
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
        if self.settings.browser_approval == BrowserApproval::Always {
            match ctx
                .approval
                .request(
                    ctx.app,
                    ctx.window_label,
                    "browser_scroll",
                    "滚动浏览器页面（always 审批策略）",
                    "browser_operate",
                    ctx.settings.approval_timeout,
                    ctx.cancellation.clone(),
                )
                .await
            {
                ApprovalOutcome::Granted => {}
                ApprovalOutcome::Rejected(r) => return ToolResult::error(format!("用户拒绝: {r}")),
                ApprovalOutcome::Timeout => return ToolResult::error("审批超时，未执行"),
                ApprovalOutcome::Cancelled => return ToolResult::error("审批流程已取消"),
            }
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

struct BrowserScreenshotTool {
    settings: Arc<AgentSettings>,
}

impl BrowserScreenshotTool {
    fn new(s: &AgentSettings) -> Self {
        Self { settings: Arc::new(s.clone()) }
    }
}

#[async_trait]
impl Tool for BrowserScreenshotTool {
    fn name(&self) -> &str { "browser_screenshot" }
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

struct BrowserCloseTool {
    settings: Arc<AgentSettings>,
}

impl BrowserCloseTool {
    fn new(s: &AgentSettings) -> Self {
        Self { settings: Arc::new(s.clone()) }
    }
}

#[async_trait]
impl Tool for BrowserCloseTool {
    fn name(&self) -> &str { "browser_close" }
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
```

- [ ] **Step 3: 注册浏览器工具（tools.rs）**

`src-tauri/src/agent/tools.rs` 顶部追加：

```rust
use crate::agent::browser::tools::{
    BrowserClickTool, BrowserCloseTool, BrowserFillTool, BrowserOpenTool, BrowserReadTool,
    BrowserScreenshotTool, BrowserScrollTool,
};
```

`with_builtins` 末尾追加：

```rust
        if settings.browser_enabled {
            r.register(Box::new(BrowserOpenTool::new(settings)));
            r.register(Box::new(BrowserReadTool::new(settings)));
            r.register(Box::new(BrowserClickTool::new(settings)));
            r.register(Box::new(BrowserFillTool::new(settings)));
            r.register(Box::new(BrowserScrollTool::new(settings)));
            r.register(Box::new(BrowserScreenshotTool::new(settings)));
            r.register(Box::new(BrowserCloseTool::new(settings)));
        }
        r
```

`src-tauri/src/agent/browser/mod.rs` 的 `pub mod policy;` 后追加 `pub mod tools;`。

- [ ] **Step 4: 写集成测试（browser_integration.rs）**

```rust
use chatwhale_lib::agent::browser::locator;
use chatwhale_lib::agent::browser::BrowserManager;
use chatwhale_lib::agent::types::AgentSettings;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn spawn_fixture_server() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { continue };
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf).await;
                let body = r#"<!doctype html><html><body>
                  <article><h1>Hello Browser</h1><p id="js">loading</p></article>
                  <a id="go" href="/clicked">Go</a>
                  <input id="name" name="name" value="preset">
                  <script>document.getElementById("js").textContent = "rendered by js";</script>
                </body></html>"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });
    (format!("http://{addr}"), handle)
}

#[tokio::test]
async fn browser_open_read_click_fill_scroll_screenshot_close() {
    let Some(exe) = locator::locate(None) else {
        eprintln!("未检测到 Chrome/Edge，跳过集成测试");
        return;
    };
    let (base, _server) = spawn_fixture_server().await;
    let tmp = std::env::temp_dir().join(format!("cw-browser-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).unwrap();
    let bm = BrowserManager::new(tmp.clone());
    let settings = AgentSettings {
        browser_path: Some(exe.display().to_string()),
        ..Default::default()
    };

    let snap = bm.open("test", &settings, &format!("{base}/")).await.unwrap();
    assert!(snap.body_text.contains("rendered by js"), "JS 渲染内容未出现");
    assert!(snap.article_text.contains("Hello Browser"));

    let snap2 = bm.click("test", &settings, None, Some("Go")).await.unwrap();
    assert!(snap2.url.ends_with("/clicked"), "点击后 URL 未跳转: {}", snap2.url);

    let msg = bm.fill("test", &settings, "#name", "hello").await.unwrap();
    assert!(msg.contains("hello") || msg.contains("#name"));
    let snap3 = bm.read("test", &settings, None, None).await.unwrap();
    assert!(snap3.form_values.iter().any(|f| f.name == "name" && f.value == "hello"));

    let scroll_msg = bm.scroll("test", &settings, "bottom", None).await.unwrap();
    assert!(scroll_msg.contains("已滚动"));

    let shot = bm.screenshot("test", &settings, None).await.unwrap();
    assert!(std::fs::metadata(&shot).unwrap().len() > 100, "截图文件过小");

    bm.close("test").await.unwrap();
    bm.close("test").await.unwrap(); // 幂等
    let _ = std::fs::remove_dir_all(&tmp);
}
```

- [ ] **Step 5: 运行单元与集成测试**

Run: `cargo test -p chatwhale browser::tools && cargo test -p chatwhale --test browser_integration`
Expected: 单元测试 PASS；集成测试在无 Chrome/Edge 时打印 skip 提示并 PASS，有 Chrome/Edge 时真实浏览器全流程 PASS。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/agent/browser/tools.rs src-tauri/src/agent/browser/mod.rs src-tauri/src/agent/tools.rs src-tauri/tests/browser_integration.rs
git commit -m "feat(agent): 浏览器工具集（open/read/click/fill/scroll/screenshot/close）与集成测试"
```

---

### Task 9: 前端设置字段、审批级别按钮与截图预览

**Files:**
- Modify: `src/composables/agentSettingsFields.ts`
- Modify: `src/components/AgentSettings.vue`
- Modify: `src/types/index.ts`
- Modify: `src/composables/useAgent.ts`
- Modify: `src/components/ChatView.vue`
- Modify: `src-tauri/capabilities/default.json`
- Create: `src/composables/agentSettingsFields.test.ts`
- Test: `npm test`、`npm run typecheck`

**Interfaces:**
- Consumes: Task 6 的 `ApprovalRequest.choices` 与 `approveCommand(id, approved, level?)`、Task 7 的 `agent-tool-result.image_path`
- Produces: 设置表单 5 个新字段；审批弹窗级别按钮；工具卡片截图缩略图（`convertFileSrc`）

- [ ] **Step 1: 写失败测试（vitest）**

```ts
import { describe, expect, it } from "vitest";
import { SETTING_FIELDS } from "./agentSettingsFields";

describe("agentSettingsFields", () => {
  it("包含浏览器工具相关设置字段", () => {
    const keys = SETTING_FIELDS.map((f) => f.key);
    expect(keys).toContain("agent.browser_enabled");
    expect(keys).toContain("agent.browser_path");
    expect(keys).toContain("agent.browser_approval");
    expect(keys).toContain("agent.browser_content_policy");
    expect(keys).toContain("agent.browser_domain_policy");
  });
});
```

- [ ] **Step 2: 运行测试验证失败**

Run: `npm test`
Expected: FAIL（`agent.browser_enabled` 不存在）

- [ ] **Step 3: 实现设置字段与校验**

`src/composables/agentSettingsFields.ts` 的 `SETTING_FIELDS` 末尾追加：

```ts
  { key: "agent.browser_enabled", label: "浏览器工具（CDP）", type: "select", options: ["false", "true"] },
  { key: "agent.browser_path", label: "浏览器可执行文件路径（留空自动探测）", type: "text" },
  { key: "agent.browser_approval", label: "浏览器操作审批策略", type: "select", options: ["navigation", "always"] },
  { key: "agent.browser_content_policy", label: "网页内容读取级别（全局默认）", type: "select", options: ["strict", "normal", "trusted"] },
  { key: "agent.browser_domain_policy", label: "域名覆盖（JSON：域名→级别）", type: "textarea" },
```

`src/components/AgentSettings.vue` 的 `save()` 中，在既有 JSON 校验后追加：

```ts
    JSON.parse(settings.value["agent.browser_domain_policy"] || "{}");
```

`placeholderFor` 增加分支：

```ts
  if (key === "agent.browser_domain_policy") {
    return '{"example.com":"trusted","*.foo.com":"normal"}';
  }
```

- [ ] **Step 4: 前端类型与状态**

`src/types/index.ts`：

```ts
export interface ToolExecution {
  id: string;
  name: string;
  arguments: string;
  source: string;
  status: "running" | "done" | "error";
  result?: string;
  error?: string;
  image_path?: string;
}
```

`src/composables/useAgent.ts` 的 `setToolState` 增加 `image_path` 透传：

```ts
  function setToolState(id: string, patch: Partial<ToolExecution>) {
    const cur = toolStates.value[id] ?? {
      id,
      name: "",
      arguments: "",
      source: "builtin",
      status: "running" as const,
    };
    toolStates.value = { ...toolStates.value, [id]: { ...cur, ...patch } };
  }
```

`ToolResultPayload` 增加字段并在 `agent-tool-result` 监听中透传：

```ts
interface ToolResultPayload {
  id: string;
  name: string;
  result: string;
  error?: string | null;
  image_path?: string | null;
}
```

监听回调改为：

```ts
      await listen<ToolResultPayload>("agent-tool-result", (e) => {
        const p = e.payload;
        setToolState(p.id, {
          status: p.error ? "error" : "done",
          result: p.error ?? p.result,
          error: p.error ?? undefined,
          image_path: p.image_path ?? undefined,
        });
      }),
```

- [ ] **Step 5: ChatView 审批级别按钮与截图缩略图**

`src/components/ChatView.vue` 顶部 import 追加：

```ts
import { convertFileSrc } from "@tauri-apps/api/core";
```

`<script setup>` 内新增：

```ts
const assetUrl = (p?: string) => (p ? convertFileSrc(p) : "");
```

工具卡片（`tool-card` 内、`tool-result-preview` 之后）追加缩略图：

```html
        <img
          v-if="ts.image_path && ts.status === 'done'"
          :src="assetUrl(ts.image_path)"
          class="tool-thumb"
          alt="browser screenshot"
        />
```

样式追加：

```css
.tool-thumb {
  max-height: 96px; border-radius: var(--radius-sm);
  border: 1px solid var(--tool-border); object-fit: contain;
}
```

审批卡片内 `approval-actions` 替换为：

```html
      <div class="approval-actions">
        <template v-if="pendingApproval.choices && pendingApproval.choices.length">
          <button class="btn-approve" @click="approveCommand(pendingApproval.id, true)">允许</button>
          <button
            v-for="c in pendingApproval.choices"
            :key="c.level"
            class="btn-approve"
            @click="approveCommand(pendingApproval.id, true, c.level)"
          >{{ c.label }}</button>
          <button class="btn-reject" @click="approveCommand(pendingApproval.id, false)">拒绝</button>
        </template>
        <template v-else>
          <button class="btn-approve" @click="approveCommand(pendingApproval.id, true)">批准</button>
          <button class="btn-reject" @click="approveCommand(pendingApproval.id, false)">拒绝</button>
        </template>
      </div>
```

- [ ] **Step 6: asset protocol 作用域（capabilities/default.json）**

在 `"permissions"` 之后、对象结尾前追加：

```json
  "assetProtocol": {
    "enable": true,
    "scope": ["$APPDATA/**", "$TEMP/**"]
  }
```

- [ ] **Step 7: 运行测试验证通过**

Run: `npm test && npm run typecheck`
Expected: PASS（新增 vitest 通过；typecheck 无错误）

- [ ] **Step 8: 提交**

```bash
git add src/composables/agentSettingsFields.ts src/components/AgentSettings.vue src/types/index.ts src/composables/useAgent.ts src/components/ChatView.vue src-tauri/capabilities/default.json src/composables/agentSettingsFields.test.ts
git commit -m "feat(agent): 浏览器设置字段、审批级别按钮与截图预览"
```

---

### Task 10: 文档同步与全量验收

**Files:**
- Modify: `docs/superpowers/specs/2026-08-08-browser-tools-design.md`（状态改“已实现”，按实际实现补注）
- Modify: `docs/agent-capabilities-design.md`（概述与内置工具列表补浏览器工具；修订说明 v1.4）
- Test: 全量验收命令

- [ ] **Step 1: 同步设计文档状态**

`docs/superpowers/specs/2026-08-08-browser-tools-design.md` 头部：

```markdown
状态：已实现（2026-08-08，按实施计划落地）
```

并在第 4.2 节补充：

```markdown
实施说明：采用设计文档 4.2 的主方案 chromiumoxide（CDP 协议客户端），
`browser/cdp.rs` 只暴露启动/求值/导航/截图四个内部方法，把 chromiumoxide
小版本 API 差异隔离在单文件内。
```

- [ ] **Step 2: 同步 agent-capabilities-design.md**

在第 1 节概述的内置工具列表追加：

```markdown
- **浏览器工具** — CDP 驱动系统 Chrome/Edge（可见窗口），支持打开/读取/点击/填表/滚动/截图/关闭，内容读取级别由用户选择（全局默认 + 域名覆盖 + 弹窗临时放宽）
```

头部修订说明追加一行：

```markdown
  修订说明 (v1.3 → v1.4): 新增浏览器工具能力（chromiumoxide 驱动 Chrome/Edge），详见 docs/superpowers/specs/2026-08-08-browser-tools-design.md
```

- [ ] **Step 3: 全量验收**

Run: `cargo test`
Expected: PASS

Run: `npm test`
Expected: PASS

Run: `npm run typecheck`
Expected: PASS

Run: `npm run build`
Expected: PASS（内置 typecheck + vitest + vite build）

如环境未装 Rust/Node 依赖导致网络失败，按提示申请批准后重跑；验收未全绿不得提交。

- [ ] **Step 4: 提交**

```bash
git add docs/superpowers/specs/2026-08-08-browser-tools-design.md docs/agent-capabilities-design.md
git commit -m "docs(agent): 浏览器工具设计文档同步为已实现，能力文档更新 v1.4"
```

---

## 自检记录

- **Spec 覆盖**：设计文档第 5 节 7 个工具 → Task 8；第 7.1 审批（navigation/always + 临时放宽）→ Task 6/8；7.2 内容策略三级 + 生效顺序 → Task 1/2/5/8；7.3 截图沙箱 → Task 7；8.1 设置键 → Task 1/9；8.2 前端与审批协议 → Task 6/9；9 错误处理 → Task 4/7/8（浏览器缺失、端口超时、连接关闭、截图路径越界）；10 测试 → Task 1-9；12 验收 → Task 10。
- **Placeholder 扫描**：无 TBD/TODO、无“类似 Task N”表述；chromiumoxide 为主实现（Task 4 含“核对本地 crate API”步骤，API 差异只在 `cdp.rs` 内适配）。
- **类型一致性**：`BrowserContentPolicy` / `BrowserApproval` 由 Task 1 定义并在 Task 2/5/6/7/8 复用；`ToolResult.image_path` 由 Task 7 定义、Task 8 使用、Task 9 前端消费；`approveCommand(id, approved, level?)` 由 Task 6 定义、Task 9 模板调用；`BrowserManager` 方法签名在 Task 7 定义、Task 8 工具与集成测试调用，命名一致。
