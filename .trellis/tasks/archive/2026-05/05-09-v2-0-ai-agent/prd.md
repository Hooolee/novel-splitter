# V2.0 流水线改造 + AI 多 Agent + 前端看板

## Goal

将单体循环的扫榜分析引擎拆分为三段式任务驱动流水线，引入 AI 多 Agent 评估，并升级前端为情报看板。

## What I already know

- 当前 `analysis_engine.rs::run_full_analysis_pipeline()` 是**单体循环**：扫榜→逐个抓元数据→逐个下载章节→逐个 AI 分析→宏观总结，所有步骤串行
- 数据库已就绪（novels、chapters、scan_reports、rank_history 四张表）
- 前端已有 3 个 Tab（书库/报告/下载），有 AI 配置、文件树、报告展示
- `ai.rs` 已有流式分析 (`stream_analysis`) 和同步调用 (`call_ai`)
- 番茄榜单暂未实现（代码中直接 return Err）
- 扫榜并发为 1，效率低
- AI prompt 目前输出大段 Markdown 描述，未结构化
- 系统托盘已支持"立即全量扫榜"，调度器按 cron 触发

## Assumptions (temporary)

- 优先完成后端管线改造
- 多 Agent 评估在后端管线改造之后做
- 前端可视化最后做

## Requirements (evolving)

参见 todo.md 的四个任务：

### 任务一：任务驱动型异步流水线
1. 扫榜分发 (Producer) — 只抓取榜单 top 30 的 book_id 和书名
2. 抓取 Worker — Semaphore(3) 并发拉取前三章，存入 Chapters 表
3. AI 提纯 Worker — Semaphore(3) 并发调用 LLM，输出 JSON 细纲

### 任务二：多视角智能体深度评估
1. 毒舌读者 Agent / 资深主编 Agent / 白金作者 Agent
2. 汇总评分存入 ai_reviews_json 字段

### 任务三：前端 UI 瘦身
1. 移除/合并"下载"Tab，后台默默执行

### 任务四：动态可视化大盘
1. 爬榜曲线追踪（Rank_History）
2. 多维雷达图（Agent 评分）
3. 灵感书库多维检索

## Acceptance Criteria (evolving)

- [ ] 流水线三段之间互不阻塞
- [ ] 并发 Worker 可配置
- [ ] AI 输出 JSON 细纲存入 chapters.outline_json
- [ ] 番茄榜单可正常扫榜
- [ ] AI 多 Agent 评估正常生成

## Out of Scope (初版)

- 前端雷达图 / 爬榜曲线（放在后续版本）
- 灵感书库多维检索（放在后续版本）

## Technical Notes

### 关键文件
- `src-tauri/src/analysis_engine.rs` — 核心管线（需大改）
- `src-tauri/src/db.rs` — 数据库操作
- `src-tauri/src/ai.rs` — AI 调用
- `src-tauri/src/lib.rs` — 命令定义
- `src-tauri/src/scheduler.rs` — 定时调度
- `src/App.vue` — 前端 UI

### 约束
- Rust + Tauri 2 桌面应用
- SQLite 数据库（rusqlite）
- 番茄榜单暂不可用
- 当前管线是串行单体，需拆为三段

---

## 任务二最终设计（2026-05-09 brainstorm 收敛）

### Goal（refined）
为已下载的小说自动生成「毒舌读者 / 资深主编 / 白金作者」三视角评估，结果以**分歧驱动**的 JSON 形态写入 `novels.ai_reviews_json`，供后续前端书库展示和情报聚合使用。

### Decision (ADR-lite)
- **Context**：流水线已完成章节级 outline_json 提纯（Phase 3），需要小说级别的多视角评估增强用户判断力。
- **Decision**：
  1. **仅做后端**，本任务**不动前端 UI**（前端展示挪到任务三/四）
  2. **Schema：分歧驱动** — 每 Agent 输出 `vote ∈ {yes, no, maybe}` + 三个 `focus[]` 关注点，外加汇总 `consensus`
  3. **触发：双轨** — 自动作为 `run_full_analysis_pipeline` 的 **Phase 4**（对当次扫到的所有书做） + 暴露独立 Tauri 命令 `evaluate_novel(novel_id)` 供手动单本重跑
  4. **输入：拼接 chapters.outline_json** + 元数据（title/tags），截断 ~6000 chars
  5. **三 Agent 并行调用**（`tokio::join!` 三路），沿用全局 `Semaphore(3)`
  6. **错误处理**：单 Agent 失败 → 该 agent 字段为 null，剩余 vote 仍计算 consensus；三 Agent 全失败 → ai_reviews_json 留 NULL（不写半成品）
- **Consequences**：
  - 不直接产出可视化数据（如雷达图所需的连续评分维度），任务四需要额外提取 vote 频次
  - 「分歧驱动」语义贴近读者社群讨论形态，可统计性弱（投票计数 + 关注点 tag 频次）
  - Phase 4 增加管线耗时 ≈ 每本 1 路 RTT × 3 并行 ≈ 5-15s/书

### 输出 Schema（novels.ai_reviews_json，UTF-8 JSON 字符串）

```json
{
  "agents": {
    "reader":  { "vote": "yes|no|maybe", "focus": ["…", "…", "…"], "comment": "（可选 1 句主观）" },
    "editor":  { "vote": "…",            "focus": [...],           "comment": "…" },
    "author":  { "vote": "…",            "focus": [...],           "comment": "…" }
  },
  "consensus": "all_yes | all_no | majority_yes | majority_no | divergent",
  "meta": {
    "model": "<config.model>",
    "generated_at": "<ISO-8601>",
    "input_chapters": <int>
  }
}
```

失败的 Agent 字段写为 `null`：`"reader": null`，但 `meta` 始终存在。

### Requirements

- **R1** `ai.rs` 新增三个 Agent prompt 常量（`READER_PROMPT` / `EDITOR_PROMPT` / `AUTHOR_PROMPT`），每个要求严格输出单 Agent 的 JSON `{ vote, focus, comment }`
- **R2** `ai.rs` 新增 `pub async fn multi_agent_review(config, title, tags, outline_blob) -> Result<String>`，内部 `tokio::join!` 三路 `call_ai(response_json=true)`，组装 `agents{} + consensus + meta`，返回最终 JSON 字符串
- **R3** `analysis_engine.rs` 新增 `run_multi_agent_phase(books, ai_config, semaphore) -> usize`：
  - 对每本书：从 DB 取所有 chapters.outline_json 拼接（标头 `## 第N章 标题`），再附 `## 元数据`（title/tags）
  - 调用 `multi_agent_review`，结果 UPDATE 进 `novels.ai_reviews_json`
  - 沿用 `Semaphore(3)` 并发
- **R4** `run_full_analysis_pipeline` 末尾追加 Phase 4 调用（在快照 / 报告生成之前）
- **R5** `lib.rs` 新增 Tauri 命令 `evaluate_novel(novel_id: i64) -> Result<String, String>`：手动重跑单本评估
- **R6** 单 Agent JSON 解析失败 → 该 agent 字段记 `null`，其余正常写入
- **R7** 三 Agent 全失败 → 跳过该本书的 UPDATE（保留旧值或 NULL），eprintln 警告
- **R8** `pipeline_e2e.rs` 增加断言：跑完后至少一本书 `ai_reviews_json IS NOT NULL` 且 JSON 合法

### Acceptance Criteria

- [ ] 跑完 pipeline 后，当次扫到的 novels 行至少 1 本 `ai_reviews_json` 不为 NULL
- [ ] JSON 严格符合上述 schema（pipeline_e2e 校验：解析后存在 `agents.reader|editor|author` 键 + `consensus` ∈ 5 枚举值 + `meta.model` 非空）
- [ ] 三 Agent 同时失败时，该书 row 不写入半成品（`ai_reviews_json` 维持原状）
- [ ] 单 Agent JSON 校验失败：跳过该 Agent（其它仍写入），eprintln 警告
- [ ] `evaluate_novel(novel_id)` 命令可独立运行，返回新生成的 JSON 字符串
- [ ] `cargo build` 通过、`cargo run --bin pipeline_e2e` 通过

### Out of Scope（任务二）

- 前端展示（书库卡片、雷达图、Tab 调整）
- 多本批量评估的进度事件 UI 反馈
- 评估结果历史版本（每次直接覆盖）
- 自定义 prompt（写死，与 OUTLINE_ANALYSIS_PROMPT 一致风格）
- 番茄平台支持（仍为 not_implemented）

### Implementation Plan（小步走）

1. **PR1**：`ai.rs` — 三个 Agent prompt + `multi_agent_review()` 函数（含单元测试模拟 LLM 返回）
2. **PR2**：`analysis_engine.rs` — `run_multi_agent_phase()` + `compute_consensus()` 辅助
3. **PR3**：`run_full_analysis_pipeline` 末尾接入 Phase 4
4. **PR4**：`lib.rs` 暴露 `evaluate_novel` 命令
5. **PR5**：`pipeline_e2e.rs` 增加 Phase 4 校验

### Definition of Done

- 代码：Tests 添加、`cargo build`/`cargo clippy` 通过、`pipeline_e2e` 通过
- Spec：若发现新约定（JSON schema、failure mode），通过 `trellis-update-spec` 更新
- 不破坏：任务一已完成的三段管线行为不变，前端不动

