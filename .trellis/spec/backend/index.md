# Rust + Tauri 后端开发规范

> **技术栈**: Rust + Tauri 2 + reqwest + scraper + tokio

## 文档索引

| 文件 | 说明 | 何时阅读 |
|------|------|----------|
| [目录结构](./directory-structure.md) | Rust 模块组织和目录布局 | 开始新功能时 |
| [Tauri 命令](./tauri-commands.md) | Tauri 命令定义、参数、事件模式 | 创建/修改后端命令时 |
| [爬虫开发](./spider-development.md) | 多平台爬虫 trait、路由、章节解析 | 添加新平台爬虫时 |
| [AI 集成](./ai-integration.md) | LLM API 调用、流式响应、提示词管理 | 修改 AI 分析功能时 |
| [日志](./logging.md) | 日志记录、调试、文件输出 | 调试、可观测性 |
| [性能](./performance.md) | 异步并发、请求控制、批量处理 | 性能优化时 |
| [类型安全](./type-safety.md) | Rust 类型系统、错误处理、序列化 | 类型相关决策 |
| [质量](./quality.md) | 提交流程检查清单 | 提交前 |

## 核心规则速览

| 规则 | 参考 |
|------|------|
| **所有 Tauri 命令返回 `Result<T, String>`** | [tauri-commands.md](./tauri-commands.md) |
| **爬虫实现统一 Trait 接口** | [spider-development.md](./spider-development.md) |
| **AI 流式响应通过事件发送** | [ai-integration.md](./ai-integration.md) |
| **使用 `eprintln!` / `log` 而非 `println!`** | [logging.md](./logging.md) |
| **异步操作使用 `tokio::spawn` / `async`** | [performance.md](./performance.md) |
| **文件路径使用 `PathBuf` 而非字符串拼接** | [type-safety.md](./type-safety.md) |
| **错误使用 `anyhow::Result` 或自定义错误类型** | [type-safety.md](./type-safety.md) |
| **爬虫并发控制使用 `tokio::semaphore`** | [performance.md](./performance.md) |

**语言**: 所有文档使用**中文**编写。
