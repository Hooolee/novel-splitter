# V2.0 任务四b：灵感书库情报面板

> **方向修正**：原始任务是「多维雷达图」，brainstorm 后用户指出真实需求不是做可视化报告，而是**快速了解近期可写什么题材、哪些金手指/设定读者喜欢**。本任务从「雷达图」转向「情报聚合统计面板」。

## Goal

V2.0 流水线已积累多本小说的评估数据（tags + ai_reviews），但用户只能一本本点开卡片看各 Agent 评语，缺乏一个**聚合视图**来回答：「这批扫到的书里哪个 tag 最热？读者对哪种设定评价最高？分歧最大的是什么题材？」。本任务在 library Tab 左侧加一个统计面板，自动从现有数据提炼热门题材和共识分布。

## What I already know

- 依赖 4a：已有 `novels[]` 数据（含 tags、ai_reviews、consensus）在前端 `loadNovels()` 后可用
- 数据源：
  - `novels[].tags`：逗号分隔的标签（如 "玄幻,系统,重生"）
  - `novels[].ai_reviews.consensus`：`all_yes | majority_yes | divergent | majority_no | all_no`
  - `novels[].ai_reviews.agents.{reader,editor,author}.focus[]`：每个 Agent 的关注点（如 "文字水平", "节奏把控", "金手指设定"）
- consensus 权重映射：all_yes=2, majority_yes=1.5, divergent=0, majority_no=-1, all_no=-2
- **不需要图表库** — 纯 Vue + Tailwind 即可实现

## Assumptions (temporary)

- 统计面板放在 library Tab 左侧 resources 区与卡片网格之间（在 filter bar 之上或之下）
- 计算全部在前端用 computed 完成（不新增后端查询），数据量 < 200 本，性能可接受
- 面板包含 2-3 个板块：热门 tag、Agent 关注热点、共识分布

## Requirements (evolving)

- R1 新增 "libraryTabInsights.vue" 下（入口的组件）统计面板内联在 App.vue 或抽为小组件
- R2 **热门 tag 排行榜**：按 consensus 加权统计 tag 出现频次，显示 top 10-15
- R3 **Agent 关注热点**：从 `focus[]` 中提取高频关键词（如 "文笔", "节奏", "金手指"），统计出现次数
- R4 **共识速览**：显示当前库中 all_yes / majority_yes / divergent 等分布（薄饼条状图或徽章行）
- R5 面板内容跟随 `novels[]` 自动更新（响应式 computed）
- R6 面板可折叠（默认展开），避免占用太多筛选区空间

## Acceptance Criteria (evolving)

- [ ] 统计面板在 library Tab 显示，无任何额外后端改动
- [ ] 热门 tag 排行按共识加权排序
- [ ] 共识分布可视化（5 段条状图或徽章计数）
- [ ] npm run build 通过

## Definition of Done

- 纯前端任务：不涉及 Rust
- 不破坏 4a 的 filter bar / card grid / right panel
- Spec：面板组件模式写入 `.trellis/spec/frontend/components.md`

## Out of Scope

- 任何图表库依赖
- 后端聚合查询（全前端 computed）
- 跨扫榜对比趋势（4c）
- tag 多选关联（4a 已有的 filter 功能）
- 雷达图 / 爬榜曲线（4c）

## Technical Notes

### Consensus 权重映射（用于 tag 加权）

```
all_yes      = +2
majority_yes = +1
divergent    =  0
majority_no  = -1
all_no       = -2
null(未评估)  =  0
```

### 计算逻辑（前端 computed）

```
1. 遍历 novels[]
2. 对每本书拆 tags，按 consensus 权重累加 → tagWeightMap: Map<tag, number>
3. 对每本书的 agents.*.focus[] 拆关键词，计数 → focusCountMap: Map<string, number>
4. consensus 分布：统计各 consensus 值的书籍数量
```

### 布局
- 位置：左侧 library Tab → filter bar 上方的折叠面板
- 高：默认收缩到行高，展开后不超过 viewport 30%
- 紧凑型排版：tag 排行一行两列（tag 名 + 热度条）

### 相关文件
- `src/App.vue`（library Tab 区域）
- `src/components/NovelCard.vue`（接口类型 NovelListRow、AiReviews 等）
- 4a PRD：`.trellis/tasks/archive/2026-05/05-11-v2-0/prd.md`

### 研究引用
- Chart.js 调研记录放弃：`.trellis/tasks/05-11-v2-0-b/research/chart-lib.md`
