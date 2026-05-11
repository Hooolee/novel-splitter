# V2.0 任务四a：library Tab DB 化 + 多维检索

> 本任务是 **V2.0 任务四（动态可视化大盘）的第一个 sub-task**，按 brainstorm 决议拆为 4a/4b/4c：
>  - **4a 本任务**：library Tab 从文件树 → DB 卡片网格，加 tag/consensus 等多维筛选
>  - **4b follow-up**：多维雷达图（依赖 4a 卡片作为容器）
>  - **4c follow-up**：爬榜曲线（依赖 4a 卡片，且需 rank_history 积累 N 天数据）
>
> 4a 完成后另开任务单独 brainstorm 4b、4c。

## Goal

任务一/二把扫榜结果完整入库（novels.ai_reviews_json、rank_history、chapters.outline_json），任务三让单本下载也走数据库——但前端 library Tab 仍是文件树渲染，与数据库的丰富信息完全脱节。本任务把 library Tab 转向**数据库驱动**，做成"灵感书库"的入口：DB cards 网格 + tag/consensus 多维筛选 + 排序。为 4b/4c 的图表能力铺好卡片容器。

## What I already know

- 父规划：见本任务 prd 头部脚注
- **DB schema 已就绪**（src-tauri/src/db.rs，任务一/二建好）：
  - `novels`: id, book_id, platform, title, author, tags, word_count, ai_reviews_json, created_at, updated_at
  - `rank_history`: (report_id, novel_id) → rank, rank_change
- **ai_reviews_json** 提供：agents（reader/editor/author 的 vote+focus+comment）+ consensus（5 枚举）+ meta
- **前端现状（任务三后）**：
  - library Tab 渲染 `treeFiles`（来自 `get_file_tree` 命令，扫 downloads/ 文件夹）
  - 没有按 DB 查 novels 的 Tauri 命令
  - 已暴露 `trigger_full_scan` 单本/榜单入口、listen `pipeline-progress`
- **任务三 D6 遗留**：V1 老命令 `start_download` / `scan_and_download_rank` 保留但不挂 UI

## Assumptions (temporary)

- library Tab 改为 DB-only，文件树不再回退（如果用户路径下没 DB 数据则空态提示）
- 卡片网格初版用 Tailwind grid，无虚拟列表（数据预期 < 200 行）
- 筛选状态不持久化（每次切 Tab 重置）

## Open Questions

- Q1（关键）：library Tab 数据源是 DB-only 还是 DB + 文件树双轨？
- Q2 卡片信息密度：极简（标题/tags/consensus 徽章）vs 富文本（含 focus 关键词、扫榜次数、最近上榜排名等）
- Q3 筛选状态是否持久化到 localStorage
- Q4 当前 file-tree 的章节预览/原文阅读功能保留还是迁移

## Requirements (evolving)

- R1 后端新增 Tauri 命令 `list_novels(filter?: { tags?: string[], consensus?: string[], platform?: string })`：从 DB 返回 novels 数组，附 parsed ai_reviews（不返回原始 outline_blob，避免 payload 过大）
- R2 后端新增/复用查询：取每本书的最近上榜信息（latest rank_history join）
- R3 前端 library Tab UI 改造为响应式 grid：每卡显示标题、tags、consensus 徽章、Agent 投票图标行（3 个 vote icon）
- R4 前端 filter bar：tag 多选下拉、consensus 多选、排序（最近更新/最近上榜/扫榜次数）
- R5 卡片点击 → 现有右侧"原文预览/分析结果"分栏复用（保留章节预览能力）
- R6 文件树视图保留为「磁盘视图」次级入口（或仅出现在没 DB 数据时的兜底，Q1 决定）

## Acceptance Criteria (evolving)

- [ ] library Tab 默认展示 DB novels（即使 downloads/ 为空）
- [ ] tag 多选筛选生效（多选取交集 vs 并集需在 Q4 后确定）
- [ ] consensus 筛选生效（5 枚举多选）
- [ ] 卡片点击后右侧显示该书章节列表 + 元数据
- [ ] 没有 DB 数据时显示「暂无书籍·先到报告 Tab 扫榜或加单本」
- [ ] npm run build / cargo build 通过

## Definition of Done

- 测试：UI 自测；新增后端 query 函数加单元测试（仿 tests.rs 的 DB 写读校验）
- Lint / typecheck：npm run build / cargo clippy 无新错
- Spec：library 卡片组件写入 `.trellis/spec/frontend/components.md`；筛选模式写入 `state-management.md`
- 不破坏：trigger_full_scan / pipeline-progress / 单本走新管线 等任务三能力不变

## Out of Scope

- 雷达图 / 爬榜曲线（4b/4c）
- 图表库选型（4b/4c 时决定）
- 虚拟列表 / 分页（数据量小时不需要）
- 跨 novel 对比详情页
- macro_insights_json 的可视化
- 词云 / 题材饼图（V2.1）

## Technical Notes

- 关键文件：
  - 后端：`src-tauri/src/db.rs`（新增 query 函数）、`src-tauri/src/lib.rs`（新增 Tauri 命令）
  - 前端：`src/App.vue`（library Tab 区域 + ref/handlers + 新增 NovelCard 内联或抽组件）
- 父规划：`.trellis/tasks/05-11-v2-0/prd.md` 头部脚注
- 相关参考：
  - `.trellis/tasks/archive/2026-05/05-09-v2-0-ai-agent/prd.md` — ai_reviews_json schema 在第 95-113 行
  - `.trellis/tasks/archive/2026-05/05-09-v2-0-ui/prd.md` — library Tab 任务三状态
- Spec：`.trellis/spec/frontend/components.md`、`state-management.md`、`tauri-integration.md`、`type-safety.md`、`quality.md`、`guides/cross-layer-thinking-guide.md`

## Follow-up sub-tasks

- **4b 多维雷达图**：依赖 4a 的卡片做容器；需选 ECharts vs Chart.js vs SVG；雷达维度需从 ai_reviews_json.agents.*.vote 派生（vote → score）；建议在 4a 完成后另开任务 brainstorm
- **4c 爬榜曲线追踪**：依赖 4a 的卡片做容器；需 rank_history 积累 N 天（建议 ≥ 3 个 scan_reports）；图表库选型与 4b 对齐
