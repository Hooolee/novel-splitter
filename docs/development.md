# 开发上手

这份文档给已经进入代码库的人看，目标是快速改完、快速验证。

## 本地运行

### 前端开发

```bash
npm run dev
```

### 全量开发

```bash
npm run tauri dev
```

### 构建

```bash
npm run build
npm run tauri build
```

## 推荐验证顺序

1. `cargo check`
2. `cargo test`
3. `npm run build`

如果要验证离线闭环，再跑：

```bash
cargo run --bin pipeline_e2e -- --mock
```

这个 mock 路径会：

- 建测试数据库
- 写入测试小说和章节
- 走 `load_novel_for_review`
- 计算真实 `compute_consensus`
- 组装并回写 `ai_reviews_json`

## 新增爬虫

如果要加新平台，先看：

- `src-tauri/src/spiders/`
- `spiders/mod.rs`
- `analysis_engine.rs`

基本流程是：

1. 新增 spider 模块
2. 接入平台路由
3. 确认数据库 upsert 路径正常
4. 跑 `trigger_full_scan`

## 调整 AI 提示词

当前主评估在 `src-tauri/src/ai.rs`，核心常量是：

- `READER_PROMPT`
- `EDITOR_PROMPT`
- `AUTHOR_PROMPT`
- `ANALYST_PROMPT`

改 prompt 时要注意：

- `reader` / `editor` / `author` 只输出投票和观点
- `analyst` 只输出客观拆书字段
- `compute_consensus` 只看前三个主观 Agent

## 数据库配置

数据库在项目根目录下初始化，当前四张主表是：

- `novels`
- `chapters`
- `scan_reports`
- `rank_history`

`ai_reviews_json` 仍然是 `TEXT`，不需要做 schema 迁移。

## 常用命令清单

后端常用命令：

- `list_novels`
- `evaluate_novel`
- `list_rising_novels`
- `list_risk_tags`
- `trigger_full_scan`
- `get_workflow_config`

前端常用入口：

- `选题雷达`
- `对标拆书`

## 调试建议

- 先看 `cargo test` 是否通过
- 再看 `cargo run --bin pipeline_e2e -- --mock`
- 最后看前端是否正确读出 `breakdown`

如果只是文档或说明改动，不需要额外拉重的集成测试。
