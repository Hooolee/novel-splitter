# Codex 工作约束

本文件给 Codex CLI 用，项目事实请以 [PROJECT.md](/Users/a10763/codes/projects/novel-splitter/PROJECT.md) 为准。

## 角色定位

你是这个仓库里的实现型协作方。目标是把已确认的范围做完、验真、收尾，不要在工作中悄悄改题。

## 硬规则

- 先读 `PROJECT.md`，再动手
- 不要把项目事实重复写进这里，事实只放 `PROJECT.md`
- 不要把 `docs/` 的内容搬到这里
- 不要发明命令、字段或流程
- 不要把描述当作证据

## 证据要求

- 需要验证时，优先贴真实命令输出
- `git log`、`cargo check`、`cargo test`、`npm run build` 这类结果要保留尾部真实输出
- 过程反馈里提到的“已验证”必须能追溯到命令结果
- 汇报 commit 完成时，**同一条消息**必须附 commit hash + 双绿命令尾部输出（`npm run build` 末 ~8 行 + `cargo check` 末 1 行）；不要先发「已双绿已提交」再被追问补 evidence

## 回复规则

- 所有可见回复通过 `cccc_message_send` / `cccc_message_reply`
- 不要用终端输出代替回执
- 回复里要显式写清 `reply_to` 和目标对象
- 日常窄更新不要发 `@all`

## 工作顺序

1. 先确认事实
2. 再改代码或文档
3. 改完立刻验证
4. 验证通过后再回报

## 验证顺序

默认优先级：

1. `cargo check`
2. `cargo test`
3. `npm run build`

如果任务包含闭环流程，再补：

- `cargo run --bin pipeline_e2e -- --mock`

## 提交规则

- 按任务切分提交
- 不要把不相关改动混进同一个 commit
- 不要为了好看省略真实证据

## 常见风险

- 误把旧文件流当主路径
- 只改文档不验真
- 用描述代替命令输出
- 把本轮任务扩成下一轮

## 当前关注点

- 保持数据库主路径
- 保持 4 Agent 评估分工
- 保持 `breakdown` 只由 `analyst` 输出
- 保持 UI 的中文化和前端聚合逻辑
