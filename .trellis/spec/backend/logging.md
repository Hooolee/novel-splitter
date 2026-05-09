# 日志记录规范

本文档覆盖 Rust 后端的日志记录模式。

## 日志工具

### 文件日志

日志写入项目根目录的 `app.log` 文件：

```rust
use std::fs::{OpenOptions, File};
use std::io::Write;

fn log_to_file(message: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("../app.log")
    {
        writeln!(file, "[{}] {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), message).ok();
    }
}
```

### 控制台输出

```rust
// 开发调试用
eprintln!("[DEBUG] 正在处理章节: {}", title);

// 错误输出
eprintln!("[ERROR] 下载失败: {}", err);
```

## 日志级别

| 级别 | 用途 | 示例 |
|------|------|------|
| `[INFO]` | 正常操作 | 下载开始、分析完成 |
| `[WARN]` | 可恢复问题 | 重试请求、章节跳过 |
| `[ERROR]` | 需要关注的失败 | 网络错误、解析失败 |
| `[DEBUG]` | 开发诊断 | 变量值、流程跟踪 |

## 日志内容规范

### 必记录的事件

- 下载开始/完成（含小说名、章节数）
- AI 分析开始/完成/错误
- 文件操作（创建、删除）
- 错误和异常

### 记录格式

```rust
// GOOD - 结构化消息
eprintln!("[INFO] download_start name={} chapters={}", name, count);

// GOOD - 错误包含上下文
eprintln!("[ERROR] scrape_failed url={} err={}", url, err);
```

### 不应记录

- API 密钥和令牌
- 用户本地文件路径中的敏感信息
- 大段 HTML/文本内容

## 日志查看

前端通过 `read_log_file` 命令读取日志，在日志查看器模态框中显示：

```rust
#[tauri::command]
async fn read_log_file() -> Result<String, String> {
    // 读取 app.log 内容返回给前端
}

#[tauri::command]
async fn clear_log() -> Result<(), String> {
    // 清空 app.log 文件
}
```

## 最佳实践

1. **有用信息** - 日志应包含足够信息用于排查问题
2. **不要过度** - 避免在循环中每步都记录
3. **区分严重性** - 正确使用日志级别
4. **中文日志** - 用户面向的消息用中文
5. **上下文** - 包含相关 ID 和状态

## 常见陷阱

- 使用 `println!` 导致 Tauri 控制台混乱 → 使用 `eprintln!` 或文件日志
- 日志文件无限增长 → 提供清空功能
- 忘记记录错误上下文 → 错误时同时记录相关参数
- 敏感信息泄露 → 检查日志内容后再提交
