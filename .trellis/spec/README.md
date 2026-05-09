# Tauri 桌面应用开发规范

本项目 **fanqie-app** 的通用开发指南，基于 Tauri 2 + Vue 3 + Rust 技术栈。

## 结构

### [前端](./frontend/index.md)

Vue 3 + TypeScript + Vite + Tailwind CSS 前端开发模式：

- [项目结构](./frontend/directory-structure.md)
- [组件开发](./frontend/components.md)
- [状态管理](./frontend/state-management.md)
- [组合式函数 (Composables)](./frontend/composables.md)
- [Tauri 集成](./frontend/tauri-integration.md)
- [CSS 与布局](./frontend/css-layout.md)
- [类型安全](./frontend/type-safety.md)
- [质量检查清单](./frontend/quality.md)

### [后端](./backend/index.md)

Rust + Tauri 2 后端开发模式：

- [项目结构](./backend/directory-structure.md)
- [Tauri 命令](./backend/tauri-commands.md)
- [爬虫开发](./backend/spider-development.md)
- [AI 集成](./backend/ai-integration.md)
- [日志](./backend/logging.md)
- [性能](./backend/performance.md)
- [类型安全](./backend/type-safety.md)
- [质量检查清单](./backend/quality.md)

### [指南](./guides/index.md)

开发思维指南：

- [实现前检查清单](./guides/pre-implementation-checklist.md)
- [跨层思维指南](./guides/cross-layer-thinking-guide.md)

## 技术栈

- **前端**：Vue 3, TypeScript, Vite, Tailwind CSS
- **后端**：Rust, Tauri 2 (桌面框架)
- **爬虫**：scraper, reqwest, tokio
- **AI**：OpenAI 兼容 API (reqwest)
- **存储**：本地文件系统
- **构建**：npm + Cargo (单仓库)

## 语言

所有规范文档使用**中文**编写。
