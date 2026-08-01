# Better Harness Task-Loop Report

## At a Glance

- Loop Effectiveness: 42/100 (changes only after comparable later task outcomes)
- Asset Health / Repair Progress: 0/100 (0 verified, 0 partial, 5 pending)
- Demonstrated autonomy radius: not observed (not observed; not observed confidence)
- Strongest loop: Not enough evidence difference to name one.
- Largest observed leak: Use the priority moves; no single loop is uniquely weakest.
- Top expected gain: No priority benefit is available in this evidence boundary.

## What You Can Rely On Today

- README 明确安装与运行路径（npm install / npm run tauri dev|build），Tauri capabilities 显式声明前端权限面。
- 设计文档较完整（docs/design-spec.md 与 docs/guides、docs/apis），提供可发现的规范、API 与功能文档。
- 会话证据覆盖完整（8/8 合格会话全量纳入），git 提交记录与会话诉求一一对应（语法高亮、模型名称、分享导出、文件上传均已交付提交）。

## What You Gain Next

- No priority Harness move is available in this evidence boundary.



### Why these moves matter

### 改动后没有可运行的相关验证：类型检查未接线、无测试、无 CI
- Priority: Medium · Evidence: not observed in this boundary
- Reason: 会话证据显示窗口内唯一有编辑的 Episode 为 changed-without-check，0 个与最终变更相关的已审查检查；项目侧 0 个测试文件，package.json 无 typecheck/lint/test 脚本（vue-tsc 在 devDependencies 但未接线），build 只执行 vite build，且仓库无 CI。事实链：Agent 变更后无项目拥有的快速验证可运行，回归只能靠后续运行暴露。修复 owner：package.json 脚本与根指令路由。
- Expected Output:
  1. 在 package.json 增加 typecheck 脚本（vue-tsc --noEmit）并在 build 前执行
  2. 后续变更 Episode 出现与最终变更相关的已审查检查

### SSE 流式失败只有界面文本，没有日志与关联标识，无法复现-归因-验证
- Priority: Medium · Evidence: not observed in this boundary
- Reason: 会话中出现 API 400 类流式错误（Invalid assistant message）后直接 handoff，错误只写入消息气泡 UI；项目侧 ChatView.vue 由前端直接 fetch /chat/completions，src-tauri/src/sse.rs 的代理函数未注册为 Tauri command（死代码），应用无日志收集、无请求 id 关联。后果：同类接口错误无法复现、诊断与复验，Agent 无法形成修复闭环。修复 owner：前端 SSE 请求路径的可观测性路由。
- Expected Output:
  1. 每次流式请求生成稳定请求 id 并保留在错误文本与日志中
  2. 失败的 HTTP 状态与截断响应体可在日志中检索

### 交付只有 handoff 信号，无验收证据、无门禁、无恢复路由
- Priority: Medium · Evidence: not observed in this boundary
- Reason: 窗口内 0 个结构化完成、0 个用户纠正、0 个与最终变更相关的已审查检查，11 个结果信号全部是 assistant handoff；项目直接提交 main、无 CI/PR 门禁、无回滚或恢复文档。后果：交付声明无法被证据验证，错误变更可直达主线且无法快速恢复。修复 owner：验收与恢复路由（根指令 + 提交前检查）。
- Expected Output:
  1. 根指令或 README 记录验收步骤（typecheck + build + 冒烟）与恢复命令
  2. 提交前执行 typecheck 与 build

### 缺少根 AGENTS.md，Agent 只能从 README 与文档自行推断命令、owner 与边界
- Priority: Low · Evidence: not observed in this boundary
- Reason: 项目级指令文件为 0（agentInstructions status=missing），Agent 无法从项目指令获知安装/运行/验证命令、核心 owner（src/composables、src-tauri/src）与文档路由（docs/design-spec.md、docs/apis），只能靠通用知识与 README 推断；后果是导航成本高、范围边界易漂移，且风险边界（本地 API Key、未净化 HTML 渲染）无人显式声明。修复 owner：根 AGENTS.md。
- Expected Output:
  1. 包含 npm 命令、typecheck 命令、核心 owner 与文档路由
  2. AGENTS.md 不超过 80 行，命令与 package.json 一致

### 重复工作与学习机会无法判定：根扫描缺失且项目级可复用资产为零
- Priority: Low · Evidence: not observed in this boundary
- Reason: 会话证据信封未提供 requestRoots 根扫描，候选组合受 5 条上限限制且 UI 类候选集中在单一 contextGroup，无法形成两个不同上下文组的可比 Episode，因此 30 天窗口内是否存在重复流程或知识需求不可判定；同时项目级 Skills=0，即使有重复需求也没有项目 owner 可对账。后果：生命周期机会识别处于证据边界未决状态，可能遗漏可复用改进。修复 owner：harness 会话证据收集路由的根扫描输出。
- Expected Output:
  1. 证据信封补齐 requestRoots 与 requestRootBudget，或显式标记扫描不完整
  2. 报告不再声称存在或不存在重复工作流

## Five Lifecycle Dimensions

| Dimension | What the evidence proves | Evidence boundary | Summary | Boundary / blocker |
| --- | --- | --- | --- | --- |
| 任务理解 | Not observed yet | not observed in this boundary | 项目缺少 AGENTS.md/指令路由，Agent 只能从 README 与设计文档自行推断 owner、命令与边界；规划类诉求有出现，但验收边界缺乏可恢复的项目载体。 | not observed |
| 可控执行 | Not observed yet | not observed in this boundary | README 提供 npm install / tauri dev 等启动路径，Tauri capabilities 显式声明前端权限面；但无 doctor/health/reset/隔离命令，且本次审查未实际执行运行验证。 | not observed |
| 改动验证 | Not observed yet | not observed in this boundary | 窗口内唯一有编辑的 Episode 为 changed-without-check，0 个与最终变更相关的已审查检查；项目无测试文件、无 typecheck/lint 脚本接线、build 不跑类型检查、无 CI。 | not observed |
| 可靠交付 | Not observed yet | not observed in this boundary | 窗口内 0 个结构化完成、0 个用户纠正，交付证据停留在 assistant handoff；项目直接提交 main，无 CI/PR 门禁，无回滚/恢复路由。 | not observed |
| 经验沉淀 | Not observed yet | not observed in this boundary | 重复流程根扫描缺失（无 requestRoots、候选组合受上限且集中在单一 contextGroup），项目级 Skills=0，无法判定窗口内是否存在可复用的重复需求；本次为证据边界未决，而非干净无候选窗口。 | not observed |

## The 15 Small Checks

| Dimension | Small check | What the evidence proves | Evidence boundary |
| --- | --- | --- | --- |


## Evidence and Boundaries

- Episode coverage: 0 episodes, 0 edited, 0 closed, 0 repaired-and-passed
- Model: agent-work-loop-v4
- Session selection: not observed; 0 sessions analyzed of 0 eligible sessions; not observed confidence
- Delivery grades observed: not observed
- Source gaps: not observed
- Learning comparison: Not observed; 0 declared intervention(s)
