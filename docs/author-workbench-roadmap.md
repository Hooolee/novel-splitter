# 作者工作台重构路线

## 目标

把当前项目从“采集器 + 报告浏览器”收敛为一个更贴近网文作者日常决策的工作台：

- 选题雷达：回答“最近什么值得写”
- 对标拆书：回答“这本书具体该学什么”

## 当前判断

- 当前仓库同时保留了 V1 文件流和 V2 数据库流，两套心智并存。
- 起点 V2 主流程已基本接通，番茄仍停留在旧实现或半接入状态。
- 作者最关心的结构化输出仍然不足，尤其缺少：
  - 金手指类型
  - 主角人设
  - 黄金三章节奏
  - 章末钩子
  - 可复用写法

## 已完成

### Phase 0：基础可用性

- 自动生成默认 `workflow_config.json`
- 单本流程不再强依赖 `workflow_config.json`
- AI 设置会同步写回 `workflow_config.json`
- 新增 `get_workflow_config` 命令，前端可读取榜单配置
- 新增 `scan-run-status` 事件，扫榜失败不再静默卡住
- 选题页接入榜单选项展示
- 页签文案开始向“选题雷达 / 对标拆书”收敛
- 单书详情页增加“重新评估”入口

## 下一批任务

### Phase 1：收敛产品主线

- 明确 V2 为唯一主流程
- 降级或移除 V1 文件树主入口
- 把 `currentMetadata/info.json` 相关展示从主路径上挪开
- 统一“书籍详情”从 DB 读取，不再混用文件元数据

### Phase 2：重做作者视角信息架构

- `选题雷达`
  - 热门题材
  - 高频金手指
  - 黑马书
  - 风险赛道
  - 最近报告列表
- `对标拆书`
  - 单书概览
  - 三视角评估
  - 拆书提要
  - 章节节奏
  - 写法可借鉴点

### Phase 3：补作者真正需要的结构化分析

- 扩展 AI 输出 schema：
  - `goldfinger_type`
  - `protagonist_archetype`
  - `opening_hook`
  - `hook_density`
  - `pacing_notes`
  - `chapter_end_hook_types`
  - `learning_points`
- 报告端补聚合洞察，而不是只列书单

### Phase 4：清理冗余

- 删除或隔离 V1 遗留接口：
  - `get_file_tree`
  - `get_file_content`
  - `update_novel_metadata`
  - `export_chapter`
- 清理旧 README 中与现状不符的说明
- 为 V2 主流程补最小闭环测试

## 风险点

- 番茄 V2 目前未真正接通，后续接入会影响主流程设计。
- 书籍详情如果继续混用 `info.json` 与 DB，后面 UI 会越来越难维护。
- 多 Agent 评估现在更偏“好不好看”，还不够“作者能学什么”。

## 推荐执行顺序

1. 先做 Phase 1，统一主线，减少认知负担。
2. 再做 Phase 2，调整 UI 结构。
3. 然后做 Phase 3，把分析输出改成作者可用结论。
4. 最后做 Phase 4，删除冗余与旧文档。
