# Tauri 命令开发规范

本文档覆盖 Tauri 命令定义、参数模式、事件发送和文件操作的最佳实践。

## 命令定义

### 基本模式

```rust
#[tauri::command]
async fn my_command(window: tauri::Window, param: String) -> Result<String, String> {
    // 实现逻辑
    Ok("success".to_string())
}
```

### 返回类型

所有命令返回 `Result<T, String>`。错误时返回用户可读的错误信息：

```rust
#[tauri::command]
async fn start_download(
    window: tauri::Window,
    url: String,
    chapter_count: usize,
    platform: String,
    spider_visible: bool,
) -> Result<(), String> {
    // 异步爬虫逻辑
    // 错误时返回错误描述
}
```

## 事件发送

后端通过 `window.emit()` 向前端发送实时事件：

```rust
// 发送进度事件
window.emit("download-progress", serde_json::json!({
    "current": current,
    "total": total,
    "chapter": chapter_title,
}))?;

// 发送 AI 分析流
window.emit("ai-analysis", serde_json::json!({
    "content": chunk_text,
}))?;

// 发送状态事件
window.emit("ai-analysis-status", serde_json::json!({
    "status": "complete", // "start" | "complete" | "error"
    "message": "分析完成",
}))?;
```

### 事件命名规范

| 事件名 | 方向 | 用途 | 数据格式 |
|--------|------|------|----------|
| `download-progress` | 后端→前端 | 下载进度更新 | `{ current, total, chapter }` |
| `ai-analysis` | 后端→前端 | AI 流式响应 | `{ content }` |
| `ai-analysis-status` | 后端→前端 | AI 状态变更 | `{ status, message }` |

## 文件操作命令

### 文件树

```rust
#[tauri::command]
async fn get_file_tree(path: String) -> Result<Vec<FileTreeNode>, String> {
    // 构建树状结构
    // 返回文件夹和文件的层级列表
}

#[derive(Serialize)]
struct FileTreeNode {
    name: String,
    path: String,
    is_dir: bool,
    children: Option<Vec<FileTreeNode>>,
}
```

### 文件读写

```rust
#[tauri::command]
async fn get_file_content(path: String) -> Result<String, String> {
    // 读取章节或元数据文件
}

#[tauri::command]
async fn export_chapter(chapter_path: String, content: String) -> Result<(), String> {
    // 保存 AI 分析结果
}

#[tauri::command]
async fn update_novel_metadata(novel_path: String, metadata: serde_json::Value) -> Result<(), String> {
    // 保存元数据到 info.json
}
```

### 删除操作

```rust
#[tauri::command]
async fn delete_novel(novel_path: String) -> Result<(), String> {
    // 递归删除小说文件夹
}

#[tauri::command]
async fn delete_chapter(chapter_path: String) -> Result<(), String> {
    // 删除单个章节文件
}
```

## 命令注册

所有命令必须在 `main.rs` 或 `lib.rs` 中注册：

```rust
.invoke_handler(tauri::generate_handler![
    start_download,
    scan_and_download_rank,
    start_ai_analysis,
    get_file_tree,
    get_file_content,
    export_chapter,
    update_novel_metadata,
    delete_novel,
    delete_chapter,
    read_log_file,
    clear_log,
])
```

## 最佳实践

1. **异步优先** - 所有 IO 操作使用 `async fn`
2. **错误处理** - 始终返回有意义的错误信息
3. **进度反馈** - 长时间操作通过事件发送进度
4. **序列化** - 使用 `serde_json::json!()` 构建事件数据
5. **窗口参数** - 需要发送事件时传 `window: tauri::Window`

## 常见陷阱

- 忘记注册命令 → 前端调用时返回未找到错误
- 同步阻塞 → 长时间操作阻塞主线程，必须使用 `async`
- 事件名拼写错误 → 前后端事件名不匹配
- 不序列化的类型 → 事件数据必须可序列化为 JSON
