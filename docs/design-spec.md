# chatWhale 桌面客户端设计方案

## 1. 项目概述

chatWhale 是一款跨平台（macOS / Windows / Linux）的桌面端大模型对话客户端，对接兼容 OpenAI/DeepSeek API 格式的本地推理服务。核心目标是提供专业、高效的 AI 对话体验，同时保持轻量级和原生性能。

## 2. 技术选型

| 层级 | 技术 | 说明 |
|------|------|------|
| 桌面框架 | **Tauri v2** | 利用系统原生 WebView，安装包 ~5MB，远小于 Electron |
| 前端框架 | **Vue 3 + TypeScript** | 组件化开发，响应式数据流 |
| 后端（Rust） | reqwest + rusqlite + tokio | SSE 流式代理、SQLite 本地存储、系统快捷键 |
| 构建工具 | Vite | 快速 HMR，与 Tauri 深度集成 |
| 打包目标 | `.dmg` / `.msi` / `.AppImage` | 一条命令出三平台安装包 |

### 选型理由

- **Tauri v2** 相比 Electron 体积缩小 30 倍以上，且 Rust 后端天然适合处理 SSE 长连接流式响应
- **Vue 3** 的 Composition API 和响应式系统适合管理聊天消息列表、流式 token 追加等场景
- **Rust** 后端负责 API Key 安全管理、本地 SQLite 对话存储，保持沙箱安全模型

## 3. 系统架构

```
┌─────────────────────────────────────────────┐
│              Vue 3 前端 (WebView)             │
│  ┌──────────┐ ┌──────────┐ ┌─────────────┐  │
│  │ 聊天视图  │ │ 模型管理  │ │ 设置/主题    │  │
│  └──────────┘ └──────────┘ └─────────────┘  │
├─────────────────────────────────────────────┤
│              Tauri IPC Bridge                │
├─────────────────────────────────────────────┤
│               Rust 后端                       │
│  ┌──────────┐ ┌──────────┐ ┌─────────────┐  │
│  │ SSE 代理  │ │ SQLite   │ │ 系统快捷键   │  │
│  │ (reqwest) │ │ 对话存储  │ │ 托盘/通知    │  │
│  └──────────┘ └──────────┘ └─────────────┘  │
└─────────────────────────────────────────────┘
```

### 数据流（对话请求）

1. 用户输入消息 → Vue 前端组装 messages 数组
2. 通过 Tauri invoke → Rust 后端发起 SSE 请求到 chatWhale API
3. Rust 逐 token 通过 Tauri Event 推送 → Vue 实时更新渲染
4. 对话完成后 Rust 将完整记录写入 SQLite

会话数据按 `workspace_id` 过滤加载；切换工作空间时重新拉取该空间下的会话列表，旧空间会话不混入新空间。

## 4. 界面布局

采用经典的**侧边栏 + 主内容区**双栏布局：

```
┌──────────────┬──────────────────────────────────┐
│  侧边栏 280px │          主内容区                  │
│              │                                  │
│  🐋 chatWhale│  会话标题          [模型] [分享] [导出]│
│  ─────────── │  ─────────────────────────────────│
│ ▸ 工作空间 ▾ │                                  │
│  [+] 新建对话 │                                  │
│              │   👤 用户消息气泡                   │
│  今日        │                                  │
│  ● 天气查询  │   🐋 深度思考 (可折叠)               │
│    代码示例  │      模型推理过程...                 │
│              │                                  │
│  ● 数据分析  │   🔧 get_weather (工具调用卡片)       │
│              │      参数 / 返回结果                │
│  本周        │                                  │
│  ● API 设计  │   🐋 模型最终回答                   │
│  ● 性能调优  │      Markdown / 表格 / 代码         │
│              │                                  │
│  更早        │                                  │
│  ● Rust 入门 │                                  │
│  ● K8s 部署  │                                  │
│              │                                  │
│  ─────────── │  ─────────────────────────────────│
│  界面主题     │  思考:enabled effort:high temp:1.0 │
│  [霜][晨][极] │  ┌─────────────────────── [📎][➤]┐│
│  [薄][深]    │  │ 输入消息...                    ││
│  ─────────── │  └───────────────────────────────┘│
│  ● deepseek  │  chatWhale 可能产生不准确信息       │
└──────────────┴──────────────────────────────────┘
```

### 侧边栏组件

| 组件 | 功能 |
|------|------|
| 品牌标识 | chatWhale Logo + 名称 |
| 工作空间切换器 | 展示当前工作空间名称/路径与色点，切换空间、进入管理弹窗 |
| 空间管理 | 新建（可复制来源设置与 MCP）、重命名、归档/恢复、彻底删除（二次确认） |
| 新建对话按钮 | 创建空白会话 |
| 对话列表 | 按「今天/本周/更早」分组，支持搜索和重命名 |
| 主题切换器 | 5 套配色一键切换，自动保存到 localStorage |
| 模型选择器 | 调用 `/models` API 获取可用模型列表，显示当前模型和运行状态 |

### 主内容区组件

| 区域 | 组件 | 说明 |
|------|------|------|
| 顶部标题栏 | 会话标题 + 模型徽章 + 分享/导出按钮 | 52px 高度 |
| 对话滚动区 | 用户消息气泡 | 右对齐，深蓝底圆角气泡 |
| | Markdown 渲染 | 标题、列表、粗体、内联代码 |
| | 深度思考面板 | 可折叠，展示 `reasoning_content`，暖金色调 |
| | 工具调用卡片 | 可折叠，展示函数名/参数 JSON/返回结果，蓝色调 |
| | 代码块 | 语法高亮 + 语言标签 + 一键复制按钮 |
| | 数据表格 | Markdown 表格渲染 |
| 底部输入区 | 参数控制栏 | thinking 开关、effort 强度、temperature 滑块、max_tokens |
| | 输入框 | 自适应高度（上限 200px），Enter 发送 / Shift+Enter 换行 |
| | 附件/发送按钮 | 附件上传 + 发送 |

## 5. 主题系统

提供 5 套配色方案，按亮度从浅到深排列：

| 主题 | 类型 | 底色 | L 值 | 主色调 | 设计意象 |
|------|------|------|------|--------|----------|
| **霜白** Frost | 最浅 | `#f2f4f8` | 96% | 靛蓝 `#5068c8` | 冷白干净，适合明亮环境 |
| **晨露** Morning Dew | 较浅 | `#ece7de` | 91% | 墨绿 `#2d9b8e` | 暖米柔和，适合长时间阅读 |
| **极光** Aurora | 适中 | `#1a5838` | 30% | 翡翠绿 `#74fcc0` | 深翡翠绿底，有色彩不沉闷 |
| **薄暮** Dusk | 较深 | `#2a2630` | 16% | 珊瑚橘 `#d4745c` | 暗紫灰，适合低光环境 |
| **深海** Deep Ocean | 最深 | `#080d18` | 5% | 鲸青 `#4fc3b4` | 海军蓝黑，夜间沉浸式 |

每套主题独立定义 25+ 个 CSS 自定义属性（`data-theme`），覆盖背景层级、文字层级、代码块、思考面板、工具调用、用户气泡等全部组件。切换时即时生效，通过 `localStorage` 持久化偏好。

## 6. 核心功能详细设计

### 6.1 思考模式 (Thinking Mode)

- 参数控制：`thinking.type` (enabled/disabled) + `reasoning_effort` (high/max)
- UI 展示：`reasoning_content` 以可折叠面板展示，暖金色调区分于正文
- 拼接规则：无工具调用的轮次中 `reasoning_content` 不回传；有工具调用的轮次必须完整回传
- 流式渲染：SSE 中 `delta.reasoning_content` 逐 token 追加到思考面板

### 6.2 工具调用 (Tool Calls)

- 参数控制：`tools` 数组 + `tool_choice` (auto/none/required)
- UI 展示：每个 tool call 以独立卡片展示，含函数名、参数 JSON、返回结果
- 执行循环：模型返回 tool_calls → 前端执行业务逻辑 → tool role 消息回传 → 模型继续
- strict 模式：`base_url="/beta"` + `strict: true`，严格的 JSON Schema 校验

### 6.3 流式响应 (SSE)

- Rust 端使用 `reqwest` 的 `stream` feature 建立 SSE 连接
- 通过 Tauri Event 系统逐 chunk 推送，前端增量更新
- 流式参数：`stream: true` + `stream_options.include_usage: true`
- 处理 `finish_reason`：stop / length / content_filter / tool_calls / insufficient_system_resource

### 6.4 FIM 补全

- 适用于代码编辑场景的 Fill-in-the-Middle 补全
- 参数：`prompt` + `suffix`，模型返回中间内容
- UI：类似 Copilot 的内联灰色建议，Tab 键接受

### 6.5 KV Cache 管理

- 显示缓存命中/未命中 token 数（`prompt_cache_hit_tokens` / `prompt_cache_miss_tokens`）
- 提供缓存清除界面

### 6.6 对话前缀续写 (Prefix Completion)

- 参数：`prefix: true`，强制模型接续 assistant 消息前缀
- 思考模式下可传入 `reasoning_content` 作为前缀思维链
- 代码补全场景的内联编辑器模式

### 6.7 本地存储

- SQLite 数据库（Rust 端 `rusqlite` + `bundled` feature）
- 存储内容：对话记录（messages 数组）、会话元数据、用户偏好设置
- 支持：会话搜索、导出 Markdown/JSON、批量删除

## 7. 跨平台适配

| 关注点 | macOS | Windows | Linux |
|--------|-------|---------|-------|
| WebView 引擎 | WKWebView (内建) | WebView2 (自动安装) | WebKitGTK |
| 系统托盘 | TrayIcon API | TrayIcon API | TrayIcon + libappindicator |
| 全局快捷键 | globalShortcut | globalShortcut | globalShortcut |
| 通知 | 原生通知中心 | 原生通知 | notify-rust |
| 开机自启 | LaunchAgent | 注册表 Run 键 | `~/.config/autostart/*.desktop` |
| 打包格式 | `.dmg` | `.msi` / `.nsis` | `.AppImage` / `.deb` |

## 8. 项目初始化命令

```bash
# 创建 Tauri + Vue 项目
npm create tauri-app@latest chatwhale-gui -- --template vue-ts

cd chatwhale-gui

# 前端关键依赖
npm install @vueuse/core marked highlight.js katex @tanstack/vue-virtual

# Rust 端依赖 (src-tauri/Cargo.toml)
# tauri = { version = "2", features = ["tray-icon"] }
# reqwest = { version = "0.12", features = ["stream"] }
# rusqlite = { version = "0.31", features = ["bundled"] }
# serde = { version = "1", features = ["derive"] }
# serde_json = "1"
# tokio = { version = "1", features = ["full"] }

# 开发运行
npm run tauri dev

# 构建三平台安装包
npm run tauri build
```

## 9. UI 设计 Demo

参考文件：[ui-design-demo.html](./ui-design-demo.html)

Demo 包含完整的静态界面预览，可直接在浏览器中打开体验五套主题切换、思考面板折叠、工具调用卡片展开/收起、代码复制等交互效果。
