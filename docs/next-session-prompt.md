# 下一次唤醒 Codex

明天在另一台电脑上打开同一个仓库后，可以直接把下面这段发给 Codex：

```text
继续昨天的“作者工作台重构”。
仓库：/Users/llf/codes/novel-splitter
请先阅读 docs/author-workbench-roadmap.md 和 docs/next-session-prompt.md，
再检查 src/App.vue、src-tauri/src/lib.rs、src-tauri/src/scheduler.rs 的当前状态。
昨天已经完成：
1. workflow_config 默认生成与失败回传
2. 页签收敛为“对标拆书 / 选题雷达”
3. 单书详情增加“重新评估”
请从下一步开始：继续收敛 V2 主线，弱化 V1 文件流，把书籍详情统一到 DB 数据，再补作者需要的拆书结构。
完成后跑 cargo check 和 npm run build。
```

## 额外说明

- 如果新电脑上的仓库路径不同，把第一行里的路径改成新机器的实际路径即可。
- 如果你还在同一个对话线程里，简化成这句也可以：

```text
继续昨天的作者工作台重构，从 roadmap 的下一步开始。
```
