# Rust 项目结构

本文档描述 Rust 后端模块组织和目录布局规范。

## 项目结构

```
src-tauri/src/
├── main.rs                 # Tauri 应用入口
├── lib.rs                  # Tauri 命令定义 (核心 API)
├── analysis_engine.rs      # 定时分析引擎
├── ai.rs                   # AI 集成 (LLM API 调用)
├── browser_spider.rs       # 无头浏览器爬虫 (WAF 调试)
├── db.rs                   # 数据库 (SQLite, 可选)
├── spiders/                # 小说平台爬虫
│   ├── mod.rs              # 爬虫 Trait + 路由
│   ├── fanqie.rs           # 番茄小说爬虫
│   └── qidian.rs           # 起点中文网爬虫
```

## 文件职责

### `main.rs` - 应用入口

Tauri 应用启动入口，初始化插件、窗口配置：

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![...])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### `lib.rs` - Tauri 命令

所有暴露给前端的 `#[tauri::command]` 函数集中在此文件。包括：

| 命令 | 说明 | 参数 |
|------|------|------|
| `start_download` | 下载单本小说 | url, chapter_count, platform, spider_visible |
| `scan_and_download_rank` | 榜单扫书并下载 | url, platform, spider_visible |
| `start_ai_analysis` | 流式 AI 分析 | novel_path |
| `get_file_tree` | 构建文件树 | path |
| `get_file_content` | 读取文件内容 | path |
| `export_chapter` | 导出章节 | chapter_path, content |
| `update_novel_metadata` | 更新元数据 | novel_path, metadata |
| `delete_novel` | 删除小说 | novel_path |
| `delete_chapter` | 删除章节 | chapter_path |
| `clear_log` | 清空日志 | — |

### `spiders/` - 爬虫模块

#### `mod.rs` - 统一接口

定义 `NovelSpider` trait 和平台路由：

```rust
#[async_trait]
pub trait NovelSpider {
    async fn scrape_novel_info(&self, url: &str) -> Result<NovelInfo>;
    async fn scrape_chapters(&self, url: &str, chapter_count: usize) -> Result<Vec<Chapter>>;
}

pub enum Platform {
    Fanqie,
    Qidian,
    Browser { visible: bool },
}
```

#### `fanqie.rs` / `qidian.rs` - 平台实现

- 实现 `NovelSpider` trait
- 小说信息提取（标题、标签、简介、字数）
- 章节列表和内容爬取
- 起点特有 WAF 处理逻辑

### `ai.rs` - AI 集成

- 流式 AI 分析和事件发送
- JSON / 文本响应处理
- 自动分析提示词管理
- 支持 OpenAI / DeepSeek 兼容 API

### `analysis_engine.rs` - 定时引擎

- 定时扫描和分析任务
- 浏览器窗口复用
- 系统托盘集成

## 命名规范

### 文件和模块

| 类型 | 规范 | 示例 |
|------|------|------|
| Rust 源文件 | snake_case | `fanqie.rs`, `analysis_engine.rs` |
| 模块声明 | snake_case | `mod spiders;` |
| 结构体 | PascalCase | `NovelInfo`, `Chapter` |
| 函数/方法 | snake_case | `scrape_novel_info`, `start_download` |
| Trait | PascalCase | `NovelSpider` |
| 枚举 | PascalCase | `Platform`, `AnalysisStatus` |

## 何时创建新模块

- 添加新平台爬虫 → 在 `spiders/` 下新建文件，实现 `NovelSpider` trait
- 添加新后端功能 → 新建 `.rs` 文件，在 `lib.rs` 中公开命令
- 共享逻辑 → 提取到 `mod.rs` 或新建 utility 模块
