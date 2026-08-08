# chatWhale 🐋

跨平台桌面端大模型对话客户端，对接兼容 OpenAI / DeepSeek API 格式的本地推理服务。基于 Tauri v2 + Vue 3 构建，提供专业、高效的 AI 对话体验。

![chatWhale 界面截图](docs/screenshot.png)

## 功能特点

### 核心对话

- **流式 SSE 响应** — 通过 fetch ReadableStream 消费 SSE，逐 token 实时渲染模型输出
- **思考模式 (Thinking Mode)** — 支持 reasoning_content 的流式展示，可折叠的深度思考面板，暖金色调
- **工具调用 (Tool Calls)** — 函数调用卡片，展示参数 JSON 和返回结果，可折叠
- **Markdown 渲染** — 标题、列表、粗体、内联代码、代码块（语法高亮）、数据表格
- **代码块高亮** — 基于 highlight.js，语法着色随主题自适应（浅色主题深色高亮 / 深色主题亮色高亮）
- **FIM 补全支持** — Fill-in-the-Middle 补全 API 支持
- **对话前缀续写** — prefix 参数支持，强制模型接续 assistant 消息前缀
- **KV Cache 可视化** — 展示缓存命中/未命中 token 数

### 对话管理

- **多会话管理** — 创建、切换、删除对话，按时间分组（今天 / 本周 / 更早）
- **工作空间管理** — 新建（可复制来源设置与 MCP）、重命名、归档/恢复、彻底删除（二次确认）；会话、Agent 设置与 MCP 均按空间隔离
- **本地持久化** — 基于 SQLite（Rust rusqlite）存储会话与工作空间数据，刷新不丢失；API Key 仅保存在 localStorage
- **对话导出** — 一键导出对话为 Markdown 文件（含思考内容）
- **对话分享** — 一键复制对话内容到剪贴板

### Agent 模式

- **Agent 开关** — 输入区工具图标切换普通/Agent 模式；浏览器（非 Tauri）环境禁用并提示需要桌面运行环境
- **工具调用回路（Rust 后端）** — LLM 返回 tool_calls 后自动执行并把结果回传，循环直至完成；支持取消与单实例约束
- **内置工具** — `read_file` / `write_file` / `list_directory` / `search_files` / `execute_command`，路径沙箱 + 敏感文件 deny-list + 结果脱敏/截断
- **Skills 系统** — 加载 SKILL.md（全局 `~/.chatwhale/skills` 与项目 `.skills/`），按 triggers 关键词匹配注入指令与声明工具，最多 3 个
- **AGENT.md 支持** — 自动读取工作区根目录 AGENT.md 注入 system prompt，首次加载需用户确认
- **MCP 集成** — stdio 传输，工具按 `mcp__<server>__<tool>` 命名映射，按工作空间管理
- **命令审批** — always / whitelist / never 策略，审批卡片即时批准或拒绝，超时按拒绝处理
- **Agent 设置** — 工作目录、Skills 目录、MCP、审批策略、超时、结果上限、敏感路径（按工作空间作用域）

### 主题系统

提供 5 套完整配色方案，覆盖 25+ CSS 自定义属性，一键切换即时生效：

| 主题 | 类型 | 底色 | 主色调 |
|------|------|------|--------|
| 霜白 Frost | 最浅 | #f2f4f8 | 靛蓝 #5068c8 |
| 晨露 Morning Dew | 较浅 | #ece7de | 墨绿 #2d9b8e |
| 极光 Aurora | 适中 | #1a5838 | 翡翠绿 #74fcc0 |
| 薄暮 Dusk | 较深 | #2a2630 | 珊瑚橘 #d4745c |
| 深海 Deep Ocean | 最深 | #080d18 | 鲸青 #4fc3b4 |

### 参数控制

- thinking 开关（enabled / disabled）
- reasoning_effort 强度（high / max）
- temperature 温度（0 ~ 2.0）
- max_tokens 上限
- 文件附件上传（支持 40+ 种文本 / 代码格式，最大 10 MB）

### 设置与模型管理

- API Base URL 和 API Key 配置
- 账户余额查询
- 可用模型列表获取与切换

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri v2 |
| 前端框架 | Vue 3 + TypeScript |
| 构建工具 | Vite |
| Markdown | marked |
| 语法高亮 | highlight.js |
| 状态管理 | Composition API + localStorage |
| 后端 (Rust) | reqwest + rusqlite + tokio |

## 安装与运行

### 环境要求

- Node.js >= 18
- npm >= 9
- Rust >= 1.88（如需 Tauri 完整构建）

### 快速启动（浏览器模式）

```bash
git clone <repo-url>
cd chatwhale
npm install
npx vite --port 1422 --host 127.0.0.1
open http://127.0.0.1:1422
```

### Tauri 完整构建

```bash
npm install
npm run tauri dev     # 开发模式
npm run tauri build   # 生产构建
```

## 使用说明

1. 启动应用后，点击侧边栏齿轮图标打开设置
2. 填入 API Base URL 和 API Key，点击保存
3. 在底部参数栏调整 thinking / effort / temperature 等参数
4. 输入问题，Enter 发送（Shift+Enter 换行）
5. 实时查看流式响应、思考过程和工具调用

## API 兼容性

兼容 OpenAI Chat Completions API 格式，支持以下端点：

| 端点 | 说明 |
|------|------|
| POST /v1/chat/completions | 对话补全（SSE 流式） |
| GET /v1/models | 列出可用模型 |
| GET /v1/user/balance | 查询账户余额 |

已测试兼容 DeepSeek API (api.deepseek.com)。

## 开发与交付

### 验收清单（提交前必做）

每次变更提交前，按顺序执行以下命令并确认全部通过：

```bash
npm run typecheck   # 类型检查，要求退出码为 0
npm test            # 前端单元测试（vitest run），要求退出码为 0
npm run build       # 生产构建（已包含 typecheck + vitest + vite build）
cd src-tauri && cargo test   # Rust 后端单元与 MCP 集成测试，要求退出码为 0
```

如需单独运行前端单元测试：`npm test`（vitest run，Agent 相关回归测试位于 `src/composables/*.test.ts`）。

关键路径冒烟（本地手动验证，至少覆盖本次变更涉及的路径）：

1. 启动应用（`npm run tauri dev` 或浏览器模式 `npx vite --port 1422 --host 127.0.0.1`）
2. 发送一条消息，确认 SSE 流式响应逐 token 渲染、无控制台错误
3. 按变更范围验证对应功能，例如对话导出 / 分享、文件上传、主题切换
4. 工作空间：切换工作空间 → 会话列表按空间过滤；新建空间可复制来源设置与 MCP；归档后只读横幅并禁止新建/发送，恢复后可对话；彻底删除需输入空间名二次确认
5. Agent 模式（桌面环境）：开启 Agent 开关发送消息 → 观察工具调用卡片与命令审批卡片 → 批准/拒绝后循环继续，最终 `agent-done` 正常落库；浏览器模式下开关禁用并提示；Agent 运行中再次发送应被拒绝/等待

验收全部通过后才能提交；本项目不使用外部 CI 服务，验收在本地提交前完成。

### 恢复说明

```bash
git log --oneline                    # 先定位需要回退的提交
git revert <commit>                  # 已发布（已 push）的错误提交：生成反向提交并保留历史，之后 git push
git reset --hard <commit>            # 本地未发布提交：重置到目标提交，丢弃其后的提交
git restore .                        # 仅丢弃工作区未提交的改动
```

注意：`git reset --hard` 与 `git restore` 会永久丢弃未提交的改动；如需保留可先 `git stash`。
