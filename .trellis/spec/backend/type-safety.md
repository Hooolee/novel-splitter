# 类型安全和错误处理

本文档覆盖 Rust 类型系统最佳实践、错误处理和序列化模式。

## 核心规则

### 1. 使用强类型

避免裸字符串和魔法值：

```rust
// BAD - 魔法字符串
fn process_chapter(path: &str, content: &str) { ... }

// GOOD - 使用类型别名
type ChapterPath = String;
type ChapterContent = String;

// BETTER - 使用结构体
struct ChapterInput {
    path: ChapterPath,
    content: ChapterContent,
}
```

### 2. 使用 PathBuf 而非字符串拼接

```rust
use std::path::PathBuf;

// BAD - 字符串拼接
let path = format!("{}/{}.txt", novel_dir, index);

// GOOD - PathBuf
let mut path = PathBuf::from(&novel_dir);
path.push(format!("{}.txt", index));
```

### 3. 错误处理

```rust
// BAD - unwrap 可能 panic
let html = client.get(url).send().await.unwrap();

// GOOD - 返回错误
let html = client.get(url).send().await
    .map_err(|e| format!("网络请求失败: {}", e))?;

// GOOD - 自定义错误类型
#[derive(Debug)]
enum AppError {
    Network(String),
    Parse(String),
    Io(String),
}
impl std::fmt::Display for AppError { ... }
```

### 4. Option 安全处理

```rust
// BAD - 非安全展开
let title = document.select(&selector).next().unwrap();

// GOOD - 提供默认值或返回错误
let title = document.select(&selector)
    .next()
    .map(|el| el.text().collect::<String>())
    .unwrap_or_default();

// GOOD - 必需值使用 ?
let title = document.select(&selector)
    .next()
    .ok_or("未找到标题元素")?
    .text()
    .collect::<String>();
```

## 序列化

### Serde 派生

```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct NovelInfo {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub word_count: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chapter {
    pub index: usize,
    pub title: String,
    pub content: String,
}
```

### JSON 文件操作

```rust
// 写入 JSON
fn save_json<T: Serialize>(path: &Path, data: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| format!("序列化失败: {}", e))?;
    std::fs::write(path, json)
        .map_err(|e| format!("写入文件失败: {}", e))
}

// 读取 JSON
fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("读取文件失败: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("解析 JSON 失败: {}", e))
}
```

## 模块可见性

```rust
// BAD - 不必要 pub
pub fn internal_helper() { ... }

// GOOD - 最小可见性
fn internal_helper() { ... }  // 模块内使用
pub(crate) fn crate_helper() { ... }  // crate 内使用
pub fn public_api() { ... }  // 公开 API
```

## 最佳实践

1. **最小可见性** - 默认私有，按需公开
2. **丰富错误信息** - 错误消息包含上下文（文件名、URL、操作）
3. **序列化控制** - 使用 `#[serde(default)]` 处理缺失字段
4. **类型别名** - 复杂类型使用别名提高可读性
5. **避免 unwrap** - 在库代码中不使用 unwrap/expect

## 常见陷阱

- `unwrap()` panic → 使用 `map_err` + `?` 传播错误
- 路径处理错误 → 使用 `PathBuf` 而非字符串
- JSON 序列化失败 → 确保所有字段可序列化
- 文件编码问题 → 明确指定 UTF-8 编码
- 空值未处理 → 使用 `Option` 和 `unwrap_or_default`
