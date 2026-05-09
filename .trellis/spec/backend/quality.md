# 提交流程检查清单

提交后端代码前执行以下检查。

## 类型安全

- [ ] 无 `unwrap()` / `expect()` （仅在原型或测试中可用）
- [ ] 所有结构体派生 `Serialize` / `Deserialize`
- [ ] JSON 字段使用 `#[serde(default)]` 处理可选字段
- [ ] 错误消息包含上下文信息（URL、文件名等）
- [ ] 模块可见性最小化（默认 `private`）

## 错误处理

- [ ] 所有 Tauri 命令返回 `Result<T, String>`
- [ ] 网络请求有超时设置
- [ ] 错误信息用户可理解（中文）
- [ ] 关键操作有日志记录
- [ ] 临时错误有重试机制

## 爬虫

- [ ] 实现了 `NovelSpider` trait 统一接口
- [ ] 选择器提取逻辑有注释
- [ ] 请求有并发控制或间隔
- [ ] 章节下载失败不影响其他章节
- [ ] 网页结构变化时的容错处理

## 性能

- [ ] 长时间操作为异步（`async fn`）
- [ ] 独立请求可并行执行
- [ ] 无阻塞主线程的操作
- [ ] 文件资源正确关闭（RAII）

## 日志

- [ ] 错误有关联的日志记录
- [ ] 无 `println!`（使用 `eprintln!` 或文件日志）
- [ ] 无敏感信息（API key 等）在日志中

## 代码组织

- [ ] 代码在正确的位置（命令在 `lib.rs`，爬虫在 `spiders/`）
- [ ] 新命令已注册到 `invoke_handler`
- [ ] 无用代码已移除（注释代码、调试输出）
- [ ] 复杂任务有测试用例，执行前明确测试条件和预期结果
- [ ] 导入按标准库 / 外部 / 内部组织

## 快速参考

### 命令定义
```rust
#[tauri::command]
async fn my_command(param: String) -> Result<String, String> {
    Ok("success".to_string())
}
```

### 错误处理
```rust
.map_err(|e| format!("操作失败: {}", e))?;
.ok_or("未找到资源")?
```

### 并发控制
```rust
let semaphore = Arc::new(Semaphore::new(5));
let _permit = semaphore.acquire().await;
```

### 事件发送
```rust
window.emit("event-name", serde_json::json!({"key": "value"}))?;
```
