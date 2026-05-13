# Claude Code 工作约束

本文件给 Claude Code 用，项目事实请以 [PROJECT.md](/Users/a10763/codes/projects/novel-splitter/PROJECT.md) 为准。

## 定位

这里是一个数据库驱动的拆书桌面应用。你只负责把确认过的范围做完、验真、收口，不要重写项目事实。

## 约束

- 先读 `PROJECT.md`
- 不要在这里复制项目事实
- 不要重复 `docs/` 里的长内容
- 不要发明命令、字段或流程
- 不要把描述当证据

## 协作规则

- 可见回复必须走 `cccc_message_send` / `cccc_message_reply`
- 回执前先确认 `reply_to` 和目标对象
- 日常更新不要用 `@all`
- 终端输出不算消息送达

## 验证规则

- 先验证再汇报
- 需要证据时贴真实命令输出
- `cargo check`、`cargo test`、`npm run build` 是默认验证顺序
- 如果改动涉及流程闭环，再补 `cargo run --bin pipeline_e2e -- --mock`

## 写作规则

- 中文写作
- 结论短、事实清楚
- 不要把“已完成”写成没有证据的结论
- 不要让描述替代 head / tail 的真实输出

## 提交规则

- 一个任务一组 commit
- 不要混入无关文件
- 不要为了省事合并不同层次的改动

## 风险提醒

- 旧文件流不是主路径
- `breakdown` 只属于 `analyst`
- `consensus` 在 UI 上必须中文化
- 文档要跟当前代码，不跟旧记忆

## 事实入口

- 系统架构看 [docs/architecture.md](/Users/a10763/codes/projects/novel-splitter/docs/architecture.md)
- 方法论看 [docs/methodology.md](/Users/a10763/codes/projects/novel-splitter/docs/methodology.md)
- 演进史看 [docs/changelog.md](/Users/a10763/codes/projects/novel-splitter/docs/changelog.md)
- 上手流程看 [docs/development.md](/Users/a10763/codes/projects/novel-splitter/docs/development.md)
