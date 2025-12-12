# CLAUDE.md

本文件为 Claude Code (claude.ai/code) 在此仓库中工作时提供指导。

## 项目概览

**fanqie-app** 是一个基于 Tauri 的桌面应用，用于从多个平台（番茄、起点）抓取网络小说并进行 AI 分析。它结合了 Vue 3 TypeScript 前端和 Rust 后端的网页爬虫及 AI 集成。

### 核心功能
- **小说下载**：支持单本小说和榜单排名爬取（番茄小说、起点中文网）
- **AI 分析**：集成 LLM API（OpenAI、DeepSeek 等）进行小说分析
- **章节处理**：拆分章节、自动总结、元数据提取
- **文件管理**：本地文件树查看、章节/小说删除、结果导出

## 技术栈

- **前端**：Vue 3 + TypeScript + Vite + Tailwind CSS
- **后端**：Rust + Tauri 2（桌面框架）
- **爬虫**：`scraper`、`reqwest`、`tokio` 异步操作
- **AI**：OpenAI 兼容 API（通过 `reqwest`）
- **存储**：本地文件系统（无数据库）

## 开发命令

```bash
# 前端开发服务器（运行在 localhost:1420）
npm run dev

# 构建生产版本前端
npm run build

# 预览生产版本
npm run preview

# Tauri 开发模式（完整应用 + 热重载）
npm run tauri -- dev

# Tauri 生产构建
npm run tauri -- build
```

## 项目结构

### 前端 (`src/`)
- **App.vue**：主应用组件（600+ 行代码）
  - 模式切换：单本下载 vs 榜单监控
  - 文件树查看器（文件夹/章节导航）
  - 小说元数据展示（含 AI 分析结果）
  - AI 设置模态框（API 配置、模型选择、自定义提示词）
  - 下载进度日志和日志查看器
  - 主题切换（深色/浅色）

### 后端 (`src-tauri/src/`)
- **lib.rs**：暴露给前端的核心 Tauri 命令
  - `start_download`：下载单本小说
  - `scan_and_download_rank`：从榜单扫取并下载多部小说
  - `start_ai_analysis`：流式 AI 分析（通过事件）
  - `get_file_tree`：构建文件树
  - `get_file_content`：读取章节/元数据文件
  - `export_chapter`：保存分析结果
  - `update_novel_metadata`：保存元数据到 info.json
  - 文件操作：delete_novel、delete_chapter、read_log_file、clear_log

- **spiders/fanqie.rs**：番茄小说爬虫
  - 小说信息提取（标题、标签、简介、字数）
  - 章节列表和内容爬取

- **spiders/qidian.rs**：起点中文网爬虫
  - 起点特定 WAF 处理（可配置蜘蛛窗口用于调试）
  - 类似番茄的章节爬取逻辑

- **spiders/mod.rs**：爬虫 trait 和路由
  - 统一多平台接口

- **browser_spider.rs**：无头浏览器爬虫
  - 用于 WAF/JS 密集型网站

- **ai.rs**：AI 集成模块
  - 流式 AI 分析和事件发送
  - JSON/文本响应处理
  - 自动分析提示词管理

- **main.rs**：Tauri 应用初始化

## 架构模式

### 前后端通信
- **Tauri Invoke**：前端通过 `invoke()` 命令调用后端（异步）
- **事件流**：后端通过 `tauri::event::emit()` 向前端发送事件
  - `download-progress`：爬取过程中的状态更新
  - `ai-analysis`：流式 AI 响应片段
  - `ai-analysis-status`：AI 操作状态（开始/完成/错误）

### 状态管理
- **前端**：Vue ref 响应式管理 UI 状态（模式、平台、配置、文件树等）
- **设置**：存储在 localStorage（API 密钥、模型选择、自定义提示词）
- **日志**：基于文件存储（项目根目录的 app.log）

### 文件组织
- **下载目录**：`../downloads/`（相对于应用根目录）
- **结构**：`downloads/NovelName/01.txt`、`02.txt`、`info.json`
- **元数据**：每部小说一个 JSON 文件，包含标题、标签、简介、AI 分析结果

## 常见开发任务

### 添加新平台爬虫
1. 创建 `src-tauri/src/spiders/newplatform.rs`
2. 实现爬虫结构体，包含 `scrape_novel_info()` 和 `scrape_chapters()` 方法
3. 在 `spiders/mod.rs` 路由中添加平台变体
4. 在前端 App.vue（第 744-747 行）更新平台选择器

### 修改 AI 分析
- **提示词模板**：在设置模态框中编辑，或修改后端 `ai.rs`
- **流式响应**：在 App.vue 第 122-155 行监听 `ai-analysis` 和 `ai-analysis-status` 事件
- **自动分析触发**：`autoAnalyze()` 函数（第 425 行）在下载完成时调用

### 添加下载进度功能
- 在 Rust 后端发送新事件
- 在 App.vue 中通过 `listen()` 监听
- 更新 `downloadLog` ref 以刷新 UI

### 调试爬虫问题
- 启用蜘蛛窗口：在设置中勾选"显示蜘蛛窗口（调试起点 WAF/验证码）"
- 查看日志：点击页脚"查看日志"按钮（打开日志查看器模态框）
- 起点调试：启用 spiderVisible 时可能会打开浏览器窗口用于 WAF 验证

## 关键实现细节

### 小说元数据结构
```typescript
interface NovelMetadata {
    title: string;
    url: string;
    tags: string[];
    word_count: string;
    description: string;
    ai_analysis?: {
        genre: string;          // 题材
        style: string;          // 风格
        goldfinger: string;     // 金手指（主要卖点）
        opening: string;        // 故事开头
        highlights: string;     // 核心看点
    }
}
```

### 下载流程
1. 用户提供 URL 和章节数
2. 调用 `start_download()` 命令，传入平台选择
3. 后端爬虫提取小说信息和章节内容
4. 文件保存为 `downloads/NovelName/nn.txt` 格式
5. 整个过程中持续发送 `download-progress` 事件
6. 完成时自动触发 `autoAnalyze()`（若已配置 API）

### AI 分析流程
1. 读取前 N 章（默认 5 章）
2. 合并章节内容
3. 使用自定义或默认提示词调用 LLM
4. 通过 `ai-analysis` 事件流式传输响应
5. 完成时解析 JSON 响应
6. 通过 `update_novel_metadata()` 保存到 `info.json`
7. 更新 UI 元数据显示

## 样式和 UI 框架

- **Tailwind CSS**：所有样式通过工具类
- **自定义 CSS 变量**：支持主题（深色/浅色）
  - `--bg`、`--sidebar`、`--card`、`--txt`、`--txt-dim`、`--accent`、`--border`、`--border-dim`、`--hover`、`--subtle`、`--active-bg`、`--input`
- **响应式布局**：侧边栏（固定宽 288px）+ 主内容（弹性伸缩）
- **模态框**：设置和日志查看器使用固定定位和背景模糊

## 重要注意事项

- **无数据库**：所有数据存储为文件。更新需通过 `refreshTreeFiles()` 手动刷新
- **错误处理**：主要使用浏览器 alert 和 console 日志。根据需要添加结构化错误 UI
- **异步操作**：前端和 Tauri 大量使用 Promise/async。注意错误处理
- **上下文窗口**：AI 分析限制输入为 3000-15000 字，以控制成本和性能
- **日志**：写入 `app.log` 用于调试。可通过 UI 按钮清空或删除文件
- **localStorage**：设置跨会话持久化。修改存储结构时需验证迁移

## 构建和部署

- **开发**：`npm run tauri -- dev` 获得完整热重载体验
- **生产**：`npm run tauri -- build` 生成平台特定包（macOS .dmg、Windows .msi、Linux .deb）
- **包配置**：定义在 `src-tauri/tauri.conf.json`（图标、窗口大小、权限等）
