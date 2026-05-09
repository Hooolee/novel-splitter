# Vue 3 前端开发规范

> **技术栈**: Vue 3 + TypeScript + Vite + Tailwind CSS + Tauri

## 文档索引

| 文件 | 说明 | 优先级 |
|------|------|--------|
| [项目结构](./directory-structure.md) | 目录组织和文件命名 | 参考 |
| [组件开发](./components.md) | 组件设计、Props、事件、插槽 | **必读** |
| [状态管理](./state-management.md) | Vue ref/reactive、localStorage 持久化 | **必读** |
| [组合式函数 (Composables)](./composables.md) | 逻辑复用、Tauri invoke 封装 | 参考 |
| [Tauri 集成](./tauri-integration.md) | invoke 调用、事件监听、文件操作 | **必读** |
| [CSS 与布局](./css-layout.md) | Tailwind 模式、响应式、主题 | 参考 |
| [类型安全](./type-safety.md) | TypeScript 规范、接口定义 | 参考 |
| [质量](./quality.md) | 提交前检查清单 | **必读** |

## 核心规则速览

| 规则 | 参考 |
|------|------|
| **默认使用 `<script setup lang="ts">`** | [components.md](./components.md) |
| **使用 `ref()` / `reactive()` 管理状态** | [state-management.md](./state-management.md) |
| **使用 `invoke()` 调用后端命令** | [tauri-integration.md](./tauri-integration.md) |
| **事件监听在 `onMounted`/`onUnmounted` 中管理** | [tauri-integration.md](./tauri-integration.md) |
| **设置通过 localStorage 持久化** | [state-management.md](./state-management.md) |
| **No `any` 类型** | [type-safety.md](./type-safety.md) |
| **逻辑复用提取为 composable** | [composables.md](./composables.md) |
| **File tree 操作在 `refreshTreeFiles()` 中统一管理** | [tauri-integration.md](./tauri-integration.md) |

## 架构概览

```
+------------------------------------------------------+
|                Vue 3 Application                      |
|                                                       |
|  App.vue (主组件)                                      |
|  ├── 模式切换 (单本下载 / 榜单监控)                     |
|  ├── 文件树查看器                                      |
|  ├── 小说元数据显示                                     |
|  ├── AI 设置 / 日志查看器 / 配置模态框                  |
|  └── 下载进度日志                                      |
+---------------------------+--------------------------+
                            |
        Tauri invoke()      |      Tauri events
                            |
+---------------------------+--------------------------+
|                Rust Backend (Tauri 2)                 |
|  +-----------+ +--------+ +----------+ +----------+  |
|  | Commands  | | Spider | | AI       | | File     |  |
|  | (lib.rs)  | | (crate)| | (ai.rs)  | | Ops      |  |
|  +-----------+ +--------+ +----------+ +----------+  |
+------------------------------------------------------+
```

## 快速开始

1. **必读文档** - 组件开发、状态管理、Tauri 集成
2. **理解项目结构** - 参考 [目录结构](./directory-structure.md)
3. **开发** - `npm run tauri dev`
4. **提交前** - 完成 [质量检查清单](./quality.md)

**语言**: 所有文档使用**中文**编写。
