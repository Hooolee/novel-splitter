# 项目背景

`novel-splitter` 是一个基于 `Tauri + Vue 3 + Rust` 的桌面拆书工具。当前主线已经收敛到数据库驱动，前端只保留 `选题雷达` 和 `对标拆书` 两个入口。

## 系统组成

- 前端：`Vue 3 + TypeScript + Vite + Tailwind CSS`
- 后端：`Rust + Tauri 2`
- 数据层：`SQLite`
- AI 层：`OpenAI` 兼容接口，通过 `reqwest` 调用

## 主线流程

1. 用户在 `选题雷达` 触发 `trigger_full_scan`
2. 爬虫抓榜并把小说、章节、榜单快照写入数据库
3. `multi_agent_review` 并发调用 4 个 Agent：
   - `reader`
   - `editor`
   - `author`
   - `analyst`
4. 三个主观 Agent 负责投票与观点
5. `analyst` 负责客观拆书字段
6. 最终结果写回 `novels.ai_reviews_json`

## 关键数据结构

数据库有 4 张主表：

- `novels`
- `chapters`
- `scan_reports`
- `rank_history`

`ai_reviews_json` 当前包含：

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

## 当前界面事实

- `选题雷达` 负责榜单洞察、扫榜触发、报告列表
- `对标拆书` 负责书库卡片、单书详情、三视角评估、拆书占位卡片
- `consensus` 在 UI 上需要中文映射
- 详情页和报告页都以数据库为准

## 关键命令

后端常用命令：

- `list_novels`
- `evaluate_novel`
- `list_rising_novels`
- `list_risk_tags`
- `trigger_full_scan`
- `get_workflow_config`
- `update_ai_config`
- `list_reports`
- `read_report`
- `start_ai_analysis`
- `fetch_ai_models`

## 关键资产指针

- [系统架构](/Users/a10763/codes/projects/novel-splitter/docs/architecture.md)
- [拆书方法论](/Users/a10763/codes/projects/novel-splitter/docs/methodology.md)
- [演进史](/Users/a10763/codes/projects/novel-splitter/docs/changelog.md)
- [开发上手](/Users/a10763/codes/projects/novel-splitter/docs/development.md)

## 当前状态

- 主线已完成 V2 收敛
- 旧文件流不再是主路径
- 当前评估以 4 Agent 为准
- 当前报告以数据库聚合为准
