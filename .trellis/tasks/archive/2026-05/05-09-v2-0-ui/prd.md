# V2.0 任务三：前端 UI 瘦身

## Goal

V2.0 流水线（任务一三段管线 + 任务二多 Agent 评估）已全面后端化，前端「下载 Tab」是 V1 时代遗物——给的是不进数据库的两套老命令，与 V2.0 数据流割裂。本任务把"下载 Tab"砍掉，让用户感知到的扫榜下载完全由后台静默驱动；保留单本下载能力（用户拆解非榜单书籍的真实诉求），但让单本也走完整 V2.0 管线（入库 + AI 提纯 JSON + 多 Agent 评估），并顺手给后端加阶段事件让 UI 有最低限度的进度反馈。为任务四的可视化大盘腾出 UI 空间和注意力预算。

## Decisions (locked)

| # | 决策 | 选项 |
|---|---|---|
| D1 | 「下载 Tab」 | 完全砍掉（含 ref/handlers/UI） |
| D2 | 单本下载入口 | library Tab 顶部 「+ 添加书籍」按钮 + 轻量模态框 |
| D3 | 手动扫榜入口 | 归并到顶栏「立即扫榜」按钮（镜像托盘菜单），调用 trigger_full_scan() 无 target_url |
| D4 | 单本数据流 | 走 V2.0 新管线（trigger_full_scan(target_url, platform)），不再走 V1 start_download |
| D5 | 后台进度可见 | analysis_engine.rs emit("pipeline-progress", {phase, message})，前端底部状态栏展示 |
| D6 | V1 老命令 | start_download / scan_and_download_rank：UI 不再调用；本任务保留命令定义（不删 Rust），交后续小步清理 |

## Requirements

### 后端
- **R1** `run_full_analysis_pipeline` 适配单本 URL：当传入的 URL 不是榜单页（按 URL 模式判断 / 或新增 mode 参数），跳过 Phase 1 扫榜分发，把单本直接构造成 books=[{book_id, title}]，进入 Phase 2/3/4
- **R2** `analysis_engine.rs` 在 Phase 1/2/3/4 完成节点 emit 事件 `pipeline-progress`：
  ```rust
  app.emit("pipeline-progress", PipelineProgress {
      phase: u8,            // 1=scan, 2=fetch, 3=outline, 4=evaluate
      status: String,       // "started" | "completed" | "failed"
      message: String,      // 中文描述
      progress: Option<(usize, usize)>,  // (done, total)
  });
  ```
- **R3** `trigger_full_scan(target_url, platform)` 已经存在，仅需确认它能正确路由到 R1 的单本路径
- **R4** V1 老命令 start_download / scan_and_download_rank 保持原状（本任务不删 Rust，避免污染范围）

### 前端
- **R5** 移除 `activeTab === 'download'` 分支（行 1053-1083）+ 顶部 Tab 切换器收为 2-Tab（library/reports，行 815-821 重排）
- **R6** 删除「下载 Tab」专属响应式：`platform`、`mode`、`url`、`count`、`rankUrl`、`rankCount`、`rankChapterCount`、`startTask` handler 中扫榜分支（保留单本 invoke 逻辑改为 trigger_full_scan）
- **R7** library Tab 顶部新增「+ 添加书籍」按钮，点击弹模态框（URL + 平台下拉），提交时调 `trigger_full_scan(url, platform)`
- **R8** 顶部（Tab 切换器旁/SETTINGS 旁）加「⚡ 立即扫榜」按钮，点击调 `trigger_full_scan(null, null)`；按钮带 loading 状态
- **R9** 底部状态栏新增 `listen('pipeline-progress', ...)`，把事件渲染到现有 downloadLog 数组（与 V1 download-progress 共存）；运行中状态显示 `正在扫榜·Phase 2/4`
- **R10** 沿用 `refreshTreeFiles()` 的周期刷新机制，让 library Tab 流水线完成后自动刷新

## Acceptance Criteria

- [ ] App.vue 不含 `activeTab === 'download'` 任何代码路径
- [ ] Tab 切换器只剩 2 个 Tab（书库/报告），切换动画间距正确
- [ ] library Tab 顶部「+ 添加书籍」可弹出模态框，提交后能成功跑完一次 V2.0 管线（单本 URL）
- [ ] 顶部「立即扫榜」按钮能成功触发全量扫榜
- [ ] 流水线运行时底部状态栏显示阶段（如 `Phase 2/4 · 抓取中…`），完成后变回「就绪」
- [ ] 完成后 library Tab 的文件树/书库列表自动刷新出新书
- [ ] `npm run build` 通过、`cargo build` 通过、`cargo run --bin pipeline_e2e` 通过
- [ ] 评估字段写入：单本走完管线后 `novels.ai_reviews_json` 不为 NULL（继承任务二 AC）

## Definition of Done

- 测试：UI 改动按 `.trellis/spec/guides/testing-responsibility.md` 自测；pipeline_e2e 跑过；如新增单本 URL 路径，pipeline_e2e 加一条断言
- Lint / typecheck：`npm run build` / `cargo clippy` 无新错
- Spec：`pipeline-progress` 事件协议写入 `.trellis/spec/frontend/tauri-integration.md` + `.trellis/spec/backend/ai-integration.md`（如适合）
- 不破坏：scheduler / 托盘"立即扫榜"行为不变；任务一/二的后端管线行为不变；V1 老命令仍可被 CLI/未来扩展调用

## Out of Scope

- 任务四的雷达图 / 爬榜曲线 / 多维检索 — 独立任务
- V1 老命令 start_download / scan_and_download_rank 的 Rust 删除 — 后续小步清理
- 老 `downloads/` 目录数据迁移 — 由任务一处理或不做
- library Tab 卡片重设计 — 留给任务四
- i18n、主题改版

## Decision (ADR-lite)

**Context**：V2.0 数据中心是 SQLite，前端「下载 Tab」却调 V1 命令写文件不入库，与新管线割裂。同时，scheduler 静默执行哲学和"用户能感知进度"的体验冲突。

**Decision**：
1. UI 一刀切——删 Tab、清 ref，library Tab 加 「+ 添加书籍」承接单本拆解需求
2. 单本走新管线，与扫榜共用 trigger_full_scan，对外只有一套数据流
3. 后端最小改动加阶段事件 pipeline-progress，让"默默后台"在用户主动触发时仍可感知

**Consequences**：
- 用户失去"只下载不分析"能力（V1 单本命令仍可 CLI 调用，但 UI 不暴露）
- 后端要改 run_full_analysis_pipeline 接受单本 URL（小幅扩展现有签名）
- 任务四在底部状态栏机制上可直接复用 pipeline-progress 事件
- 评估成本：单本下载现在变贵（多跑 AI 提纯 + 3 路 Agent ≈ 多 10-20s）

## Technical Notes

### 关键文件
- 前端：`src/App.vue`（行 33、815-821、1053-1083、1085-1100、1200-1211 + 相关 ref/handlers）
- 后端：`src-tauri/src/analysis_engine.rs`、`src-tauri/src/lib.rs`（行 188-196 trigger_full_scan + 行 197+ trigger_full_scan_internal）
- 父任务上下文：`.trellis/tasks/archive/2026-05/05-09-v2-0-ai-agent/prd.md`（任务三定义在行 37-38）
- 后端 spec：`.trellis/spec/backend/ai-integration.md`
- 前端 spec：`.trellis/spec/frontend/tauri-integration.md`、`state-management.md`、`components.md`、`quality.md`
- 测试责任 spec：`.trellis/spec/guides/testing-responsibility.md`

### 现有事件清单
- V1 download-progress（lib.rs 多处发，前端 256 行监听）— 单本走新管线后应被 pipeline-progress 取代
- V1 ai-analysis / ai-analysis-status（ai.rs，单本拆书用）— 不动
- V2 pipeline-progress（本任务新增）

### Implementation Plan（小步走）

| PR | 范围 | 关键改动 |
|---|---|---|
| **PR1** | 后端：单本 URL 路径 + 阶段事件 | run_full_analysis_pipeline 适配单本（识别非榜单 URL 跳过 Phase 1）；analysis_engine.rs 在 Phase 1/2/3/4 边界 emit pipeline-progress |
| **PR2** | 前端：UI 瘦身 | 删 download Tab + 相关 ref/handlers/事件监听；Tab 切换器 3→2；删 ProgressPayload 的 V1 监听（如不再用） |
| **PR3** | 前端：手动入口 + 进度展示 | library Tab「+ 添加书籍」模态框；顶栏「立即扫榜」按钮；底部状态栏 listen('pipeline-progress')；refreshTreeFiles 触发时机调整 |
| **PR4** | spec / e2e | tauri-integration.md 加 pipeline-progress 事件协议；pipeline_e2e.rs 加单本 URL 路径断言 |

依赖：PR1 < PR3（前端要事件协议落地）；PR2 / PR3 可并；PR4 收尾
