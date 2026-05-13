# 系统架构

`novel-splitter` 是一个 `Tauri + Vue 3 + Rust` 的桌面拆书应用，当前主路径已经收敛到数据库。

## 总体分层

- 前端：`Vue 3 + TypeScript + Vite + Tailwind CSS`
- 后端：`Rust + Tauri 2`
- 数据层：`SQLite`
- AI 层：`OpenAI` 兼容接口，通过 `reqwest` 调用

## 前端结构

前端主组件是 `src/App.vue`，当前使用顶部两个 tab：

- `书库`（默认入口）
- `拆书雷达`

前端侧的职责划分：

- `书库` 负责书库卡片网格，每张卡片提供「🔍 对标拆书」按钮，点击进入单书详情
- `拆书雷达` 负责榜单洞察、扫榜触发、报告列表
- 单书详情视图独立于 tab；顶部有「← 返回书库」按钮，切 tab 时自动重置 `selectedNovel`

卡片和详情页都直接消费数据库返回的 `selectedNovel` / `ai_reviews` 数据，不再依赖旧文件流主路径。

## 后端结构

后端核心模块位于 `src-tauri/src/`：

- `lib.rs`：Tauri 命令注册与主流程编排
- `ai.rs`：4 Agent 评估与 JSON 组装
- `db.rs`：SQLite 读写与聚合查询
- `analysis_engine.rs`：全量扫榜流水线
- `scheduler.rs`：定时任务
- `spiders/`：平台爬虫

### 当前 Tauri 命令清单

`lib.rs` 暴露的主要命令：

- `start_ai_analysis`
- `fetch_ai_models`
- `read_log_file`
- `clear_log`
- `ensure_workspace_dirs`
- `list_reports`
- `read_report`
- `get_workflow_config`
- `trigger_full_scan`
- `update_ai_config`
- `set_workspace_root`
- `evaluate_novel`
- `list_novels`
- `list_rising_novels`
- `list_risk_tags`

## 数据模型

数据库当前只有 4 张主表：

- `novels`
- `chapters`
- `scan_reports`
- `rank_history`

### `novels`

保存小说基础信息、标签、字数、`ai_reviews_json`、创建时间和更新时间。

### `chapters`

保存章节内容、`outline_json` 和章节序号。

### `scan_reports`

保存每次扫榜生成的宏观报告。

### `rank_history`

保存某次报告中的小说排名快照和变化量。

## AI 评估流

单书评估分成 4 个并发 Agent：

- `reader`
- `editor`
- `author`
- `analyst`

其中：

- `reader` / `editor` / `author` 只负责主观投票
- `analyst` 只负责客观拆书字段
- `compute_consensus` 只看前三个主观 Agent 的投票
- `breakdown` 写入 `novels.ai_reviews_json`

### `ai_reviews_json`

当前 JSON 结构包含：

- `agents`
- `consensus`
- `breakdown`
- `meta`

其中 `breakdown` 的 7 个字段是：

- `goldfinger_type`
- `protagonist_archetype`
- `opening_hook`
- `hook_density`
- `pacing_notes`
- `chapter_end_hook_types`
- `learning_points`

## 报告洞察来源

`拆书雷达` 顶部洞察块主要来自：

- 热门题材：按 `consensus` 加权后的标签聚合
- 关注点：三视角 `focus` 聚合
- 共识：`consensus` 分布
- 高频金手指：按 `breakdown.goldfinger_type` 前端聚合
- 主角人设：按 `breakdown.protagonist_archetype` 前端聚合
- 黑马：`list_rising_novels`
- 风险赛道：`list_risk_tags`

## 扫榜入口

- 扫榜下拉硬编码 7 个起点常用榜单：`yuepiao` / `hotsales` / `newbook` / `mvp` / `signnewbook` / `fanben` / `vipclick`
- 不再从 `workflow_config.json` 读取 `rank_urls`
- 「立即扫榜」按钮位于 `拆书雷达` 顶部洞察块下方

## 关键原则

- 前端详情页以数据库为准
- `consensus` 在 UI 上必须中文化
- 三个主观 Agent 不扩展出 `breakdown`
- 旧文件流不再作为主路径
