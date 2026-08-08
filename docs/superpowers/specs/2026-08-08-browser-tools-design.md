# chatWhale 浏览器工具（CDP 驱动 Chrome/Edge）设计

日期：2026-08-08
状态：待审阅

## 1. 背景与目标

chatWhale 的 Agent 目前只有本地文件/命令能力（`read_file`、`write_file`、`list_directory`、`search_files`、`execute_command`），无法获取网页内容，更无法操作真实浏览器。用户提出需要：

1. 获取给定网页的内容（必须支持 JavaScript 动态渲染的页面）；
2. 打开**可见的浏览器窗口**，由 Agent 执行交互式操作（点击、滚动、填表等）。

已与用户确认采用**方案 1：Rust 后端直连 CDP（Chrome DevTools Protocol）驱动系统 Chrome/Edge**。能力上限最高，且能复用现有 Agent 的工具调用回路、审批机制、结果脱敏与事件推送，改造面可控。

## 2. 已确认决策

| # | 决策项 | 结论 |
|---|--------|------|
| 1 | 技术路径 | Rust 后端 + chromiumoxide（CDP）驱动系统 Chrome/Edge，可见窗口 |
| 2 | 工具集 | v1 共 7 个 `browser_*` 工具（见第 5 节）；`browser_eval`、多标签管理、下载/上传不做 |
| 3 | 审批策略 | 默认 `navigation`：仅 `browser_open` 需审批；可配 `always`（所有浏览器操作审批） |
| 4 | 登录态 | `--user-data-dir` 按工作空间隔离，登录态跨 Agent 会话保留 |
| 5 | 浏览器生命周期 | Agent 结束后保持打开（可见窗口即用户在场确认），Tauri 退出时由 Drop 清理，防孤儿进程 |
| 6 | 前置开关 | 新增 `agent.browser_enabled`，默认关闭，用户显式开启后才注册浏览器工具 |

## 3. 现状梳理

### 3.1 工具注册与执行

- `ToolRegistry::with_builtins`（`src-tauri/src/agent/tools.rs`）注册 5 个内置工具；`Tool` trait 提供 `needs_approval` 钩子，审批走 `ApprovalManager`（`approval.rs`）。
- 工具结果统一经 `finalize_result`（`redact_secrets` 脱敏 + 按 `agent.max_result_bytes` 截断）。
- Agent 编排（`agent/mod.rs`）循环执行 tool_calls：内置工具走 `registry.execute`，MCP 工具走 `mcp.call_tool`；每个工具触发 `agent-tool-start` / `agent-tool-result` 事件。
- 同一轮 tool_calls 串行执行；MCP 同一 server 内部用 `tokio::sync::Mutex` 串行化，防止响应错位。

### 3.2 设置与前端

- `AgentSettings`（`types.rs`）+ `AGENT_SETTING_KEYS` 定义设置键，前端 `agentSettingsFields.ts` 的 `SETTING_FIELDS` 渲染设置表单，`AgentSettings.vue` 读写 SQLite。
- 前端 `useAgent.ts` 监听 `agent-*` 事件并渲染工具卡片、审批弹窗；`agent-approval-request` 携带 `id / tool_name / command / policy`，用户回执走 `agent_approve`。
- Agent 单实例：`AppState.agent: Mutex<Option<AgentRuntime>>`；Tauri 退出时无显式浏览器清理（当前无浏览器进程）。

## 4. 架构设计

### 4.1 新增模块

```
src-tauri/src/agent/browser/
├── mod.rs        # BrowserManager：生命周期 + 串行化 + 会话缓存
├── cdp.rs        # chromiumoxide 封装：启动参数、CDP 连接、导航/读取/交互执行
├── extract.rs    # 页面内容提取（正文文本 / Markdown / 链接，排除输入框与密码字段）
└── locator.rs    # Chrome/Edge 可执行文件探测（macOS/Windows/Linux + 设置覆盖）
```

- `BrowserManager` 放入 `AppState`（`Arc<BrowserManager>`），跨 Agent 运行存活：浏览器窗口、Profile、登录态不因一次 Agent 结束而丢失。
- `ToolContext` 增加 `browser: Arc<BrowserManager>` 字段；`browser_*` 工具通过它执行。
- 所有浏览器操作经 `BrowserManager` 内部 `tokio::sync::Mutex` 串行化（与 MCP 同模式，防 CDP 响应错位）。

### 4.2 依赖

- 新增 `chromiumoxide`（异步 CDP 客户端，运行时 feature 用 tokio；具体版本与 feature 名在实施时以 crates.io 最新稳定版核实）。
- 若 chromiumoxide 与目标 Chrome/Edge 版本出现兼容性问题（如 OOPIF、Target 域变更），备选方案：手写极简 CDP WebSocket 客户端（仅覆盖 Navigation/DOM/Screenshot/Input 四个域，规模可控）。
- 可选：`url` crate 用于 URL 解析与规范化。

### 4.3 浏览器启动参数

由 `cdp.rs` 固定拼接，不接受用户参数注入：

```text
--user-data-dir=<app_data>/browser-profiles/<workspace_id>/
--remote-debugging-port=0（随机端口）
--remote-allow-origins=<仅本客户端来源>
--no-first-run --no-default-browser-check
```

- 不加 `--headless`，保证可见窗口。
- `browser_path` 设置项覆盖自动探测结果；探测失败给出明确错误（见第 9 节）。

## 5. 工具集（v1）

命名空间 `browser_`，均需 `agent.browser_enabled = true` 才注册；未开启时 Agent 不暴露这些工具。

| 工具 | 参数 | 行为与返回 |
|------|------|------------|
| `browser_open` | `url`（必填）、`new_tab`（可选，默认 false） | 启动浏览器（首次）并导航；**需审批**。返回标题 + 当前 URL + 页面摘要 |
| `browser_read` | `mode`（"text" / "markdown" / "links"，默认 text）、`selector`（可选）、`timeout_ms`（可选） | 等待渲染后读取页面/指定区域内容；返回文本或链接列表 |
| `browser_click` | `selector`（CSS）或 `text`（可见文本），二选一 | 点击元素，等待导航/渲染；返回点击后的 URL 与标题 |
| `browser_fill` | `selector`、`value` | 填写表单字段；**拒绝 password 类型字段**，返回成功/失败 |
| `browser_scroll` | `direction`（"down"/"up"/"top"/"bottom"）、`amount`（可选像素） | 滚动页面；返回滚动后位置摘要 |
| `browser_screenshot` | `path`（可选，workspace 内保存路径） | 截图；默认保存到临时目录并返回文件路径，结果同时带 `image_path` 供前端缩略图渲染 |
| `browser_close` | 无 | 关闭标签页；全部关闭时结束浏览器进程并清理会话缓存 |

工具参数均声明 JSON Schema（复用 `ToolDef::new`）；参数校验失败返回结构化错误，不 panic。

## 6. 生命周期与数据流

### 6.1 启动

1. 首次 `browser_*` 调用：`locator` 探测可执行文件（`browser_path` 设置 > 常见安装路径 > PATH）→ 失败则返回错误；
2. 启动子进程（可见窗口、独立 Profile、随机端口）；
3. `chromiumoxide` 连接 CDP；
4. 会话缓存到 `BrowserManager`（按 workspace_id），后续调用直接复用。

### 6.2 执行

- 每次工具调用：`BrowserManager` 加锁 → 执行 CDP 操作 → 结果经 `finalize_result` 统一脱敏/截断 → 触发 `agent-tool-result` → 塞回 messages。
- `browser_read` / `browser_click` 内部等待 `document.readyState` 与网络空闲；SPA 场景由 `selector` + `timeout_ms` 显式等待目标元素。

### 6.3 结束与清理

- Agent 结束（done / cancel / error / max_iterations）：**不**关闭浏览器；下次 Agent 复用同一窗口与登录态。
- `browser_close` 工具：关闭标签页；无标签页时结束浏览器进程并移除缓存。
- Tauri 退出：`BrowserManager` 的 `Drop` 统一 kill 子进程（防孤儿进程），清理临时调试文件。
- 用户手动关闭浏览器窗口：CDP 连接断开，标记 unhealthy；下次调用重连一次，失败则报错并提示重新打开。

### 6.4 事件

复用现有 `agent-tool-start` / `agent-tool-result`，`source` 为 `builtin`；`agent-tool-result` 增加可选 `image_path` 字段（截图场景）。

## 7. 安全模型

### 7.1 审批

- 复用 `ApprovalManager`，policy 标识为 `browser_navigate` / `browser_operate`；
- 默认 `navigation`：仅 `browser_open`（含 `new_tab`）审批，弹窗展示完整 URL；
- 可配 `always`：`browser_click` / `browser_fill` / `browser_scroll` 等操作也逐次审批；
- 可见窗口是用户在场确认的天然信号，因此默认不逐操作审批；审批策略由设置 `agent.browser_approval` 控制。

### 7.2 敏感数据

- `browser_read` 提取时排除 `input` / `textarea` / `select` 的 value（含 token、密码），`password` 字段一律跳过；
- 跳过 `script` / `style` / `noscript` / `canvas` / `svg` 文本；
- 工具结果统一过 `redact_secrets`（现有实现）；
- `browser_screenshot` 保存路径走 `resolve_workspace_path` 沙箱，越界拒绝；临时目录截图不落 workspace。

### 7.3 Prompt Injection

- 网页内容是不可信来源：`browser_read` 结果前置 `[browser 页面内容，不可信]` 标记，system prompt 明确该规则（与现有 Skills 注入防线一致）；
- 结果长度受 `agent.max_result_bytes` 限制，防止超大页面撑爆上下文。

### 7.4 端口与进程

- 调试端口随机绑定 localhost，加 `--remote-allow-origins` 限制，避免本机其他进程连入；
- 启动参数固定拼接，`browser_path` 仅来自用户设置，不接受工具参数注入；
- Profile 目录属于敏感数据（Cookie/登录态），存放于应用数据目录且按 workspace 隔离，纳入脱敏与审批说明。

## 8. 设置与前端改动

### 8.1 新增设置键

加入 `AGENT_SETTING_KEYS` 与 `AgentSettings` 结构体：

```text
agent.browser_enabled   = "false"      # 总开关
agent.browser_path      = ""           # 留空自动探测
agent.browser_approval  = "navigation" # navigation | always
```

`AgentSettings` 解析逻辑（`load_agent_settings`）同步新增字段；`types.rs` 提供 `parse_browser_approval` 与默认值。

### 8.2 前端

- `agentSettingsFields.ts`：新增 3 个字段（开关用 select：`true`/`false`，路径用 text，审批策略用 select）；
- `AgentSettings.vue`：浏览器路径行复用现有"选择目录"交互（浏览器可执行文件路径为文本输入，不做目录选择）；
- `useAgent.ts` 的 `ToolResultPayload` 增加可选 `image_path`；工具卡片渲染截图缩略图；
- 截图展示走 Tauri `asset:` 协议，需在 capabilities 中配置对应 fs scope（临时目录/截图目录）；
- 浏览器模式（vite dev）仍禁用 Agent，逻辑不变。

## 9. 错误处理

| 场景 | 行为 |
|------|------|
| 未开启 `browser_enabled` | 工具不注册，模型无法调用（等同未知工具） |
| 找不到 Chrome/Edge | 明确报错：提示安装或设置 `agent.browser_path` |
| 调试端口冲突 | 随机端口重试（最多 3 次） |
| 浏览器被用户关闭 / 连接断开 | 标记 unhealthy，下次调用重连一次；仍失败返回错误 |
| 导航超时 / 页面崩溃 | 返回结构化错误，不 panic；取消点沿用 `CancellationToken` |
| CDP 调用超时 | 使用现有 `agent.tool_timeout`（默认 30s） |

## 10. 测试策略

### 10.1 单元测试

- `extract.rs`：HTML → text / markdown / links 转换；输入框与密码字段排除；`selector` 区域读取；
- `locator.rs`：各平台候选路径列表、设置覆盖优先级；
- 工具参数校验与审批策略解析（`navigation` / `always`）；
- `BrowserManager` 状态机：unhealthy → 重连 → failed。

### 10.2 集成测试（`src-tauri/tests/`）

- 本地临时 HTTP 服务提供 fixture 页面（含 JS 动态渲染），模拟真实网页；
- 探测到系统 Chrome/Edge 时执行真实 CDP 流程：`open → read → click → screenshot`，断言内容与截图文件存在；无浏览器则 skip；
- 风格沿用现有 fake MCP server 集成测试，不依赖外网。

### 10.3 前端测试

- vitest：设置字段定义、`image_path` 工具卡片渲染（截图占位逻辑）。

## 11. 后续增强（v1 明确不做）

- `browser_eval`（任意 JS 执行，需更强沙箱与审批设计）；
- 多标签页管理、标签页列表/切换；
- 下载、上传、剪贴板；
- 网络请求拦截与修改；
- 可读性提取（Readability 风格正文抽取）增强；
- headless 模式开关、远程浏览器服务（浏览器在另一台机器）。

## 12. 验收标准

1. `cargo test`、`npm test`、`npm run typecheck`、`npm run build` 全绿（AGENTS.md 验收口径）；
2. 开启 `agent.browser_enabled` 后，Agent 可对含 JS 渲染的本地 fixture 页面完成：打开 → 读取 → 点击 → 填表（非密码）→ 滚动 → 截图全流程；
3. `browser_open` 触发审批弹窗并展示 URL；`always` 策略下所有浏览器操作均触发审批；
4. 读取结果不含输入框/密码值，结果带不可信标记且按上限截断；
5. 截图保存在 workspace（指定路径）或临时目录（默认），前端工具卡片显示缩略图；
6. 登录态跨会话保留（同一 workspace 再次运行仍可见已登录状态）；不同 workspace Profile 隔离；
7. Agent 结束后浏览器窗口保持打开；Tauri 退出后无残留浏览器进程。
