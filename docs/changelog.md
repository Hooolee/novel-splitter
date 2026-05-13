# 演进史

这份记录只保留已经落地的主线变化。

## V2.0 主线

### 初始阶段

- 从旧文件流转向数据库流
- 引入 `novels` / `chapters` / `scan_reports` / `rank_history` 四表
- 书库和报告页开始从数据库读数据

### 分析阶段

- 三视角评估由 `reader` / `editor` / `author` 组成
- `consensus` 统一收敛为 5 态
- 详情页改为 DB 主路径

### 信息架构阶段

- `选题雷达` 和 `对标拆书` 成为两个主 Tab
- 报告页顶层洞察块从书库迁到报告页
- 报告页新增黑马和风险赛道

### AI schema 阶段

- 新增 `analyst` Agent
- `ai_reviews_json` 加入 `breakdown`
- 详情页三段占位卡片开始回填客观拆书字段

### 清理阶段

- 删除旧文件流 Tauri 命令
- 文档转向数据库和 4 Agent 结构
- `pipeline_e2e` 增加离线 `--mock` 闭环

## 4 Phase 作者工作台

### Phase 1

- 单书详情切到数据库
- 弱化旧文件流

### Phase 2

- 重新组织作者视角信息架构
- 增加黑马和风险赛道
- `consensus` 中文化

### Phase 3

- 扩展 `breakdown`
- 新增 `analyst`
- 报告页增加高频金手指和主角人设

### Phase 4

- 删除旧文件流命令
- 文档对齐 V2
- 增加 mock 闭环测试

## 现在的状态

- 当前主线以数据库为准
- 当前详情页以 `selectedNovel` 和 `ai_reviews` 为准
- 当前评估以 4 Agent 为准
- 当前报告以数据库聚合为准
