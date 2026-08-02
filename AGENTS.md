# AGENTS.md

chatWhale：基于 Tauri v2 + Vue 3 + TypeScript 的桌面端 LLM 对话客户端。本文件是项目操作指令；功能说明见 README.md，设计细节见 docs/。

## 常用命令

与 package.json 的 scripts 保持一致，新增脚本时需同步更新本文件：

```bash
npm install            # 安装依赖
npm run dev            # Vite 浏览器模式开发
npm test               # 前端单元测试（vitest run，退出码须为 0）
npm run typecheck      # 类型检查（tsc --noEmit，退出码须为 0）
npm run build          # 生产构建（先 typecheck，再 npm test，最后 vite build）
npm run tauri dev      # Tauri 桌面开发模式
```

## 核心 Owner

- `src/composables/`：前端状态与业务逻辑（useChat、useConversations、useMarkdown 等）
- `src-tauri/src/`：Rust 后端（db、sse、lib 等）

## 文档路由

- 设计规范：docs/design-spec.md
- API 与协议：docs/apis/
- 功能指南：docs/guides/

## 风险边界

- API Key 仅保存在本地 localStorage（`chatwhale-api-key`），不得写入源码、日志或提交到仓库。
- 模型返回内容经 marked 渲染后由 `v-html` 注入（MessageBubble.vue）；marked 默认不做 HTML 净化，渲染模型内容时不得假设内容已净化，涉及 HTML 展示需先确认净化边界。
- 提交前按 README 的验收清单执行 `npm test`、`npm run typecheck` 与 `npm run build`（build 已内置 typecheck 与 vitest）；不使用外部 CI，验收在本地完成。
