# 前端项目结构

本文档描述 Vue 3 前端目录组织和文件命名规范。

## 项目结构

```
novel-splitter/
├── src/                          # 源代码
│   ├── App.vue                   # 主组件（单页面应用）
│   ├── main.ts                   # 应用入口
│   ├── style.css                 # 全局样式 + Tailwind
│   ├── components/               # 组件目录 (预留)
│   └── assets/                   # 静态资源
├── index.html                    # HTML 入口
├── package.json                  # 依赖管理
├── vite.config.ts                # Vite 配置
├── tsconfig.json                 # TypeScript 配置
├── tailwind.config.js            # Tailwind CSS 配置
└── postcss.config.js             # PostCSS 配置
```

## App.vue 结构

应用是一个单页面组件，包含以下功能区域：

```
App.vue
├── 状态定义 (ref/reactive)
│   ├── 模式切换 (mode)
│   ├── 配置 (apiKey, model, prompts)
│   ├── 下载状态 (downloading, progress)
│   ├── 文件树 (fileTree, expandedFolders)
│   ├── 小说元数据 (novelMetadata, aiAnalysis)
│   └── UI 状态 (showSettings, showLogViewer)
├── 下载区域
│   ├── URL 输入 + 平台选择
│   ├── 章节数设置
│   └── 开始下载按钮
├── 文件树区域
│   ├── 文件夹展开/折叠
│   └── 章节列表
├── 内容/元数据显示区域
│   ├── 小说信息卡片
│   └── AI 分析结果
├── 模态框
│   ├── 设置 (API, 模型, 提示词)
│   └── 日志查看器
└── 日志/进度区域
    └── 下载进度日志
```

## 文件命名

| 类型 | 规范 | 示例 |
|------|------|------|
| Vue 组件 | PascalCase | `App.vue` |
| TypeScript | camelCase | `main.ts` |
| 样式文件 | kebab-case | `style.css` |
| 配置文件 | kebab-case | `vite.config.ts` |

## 组件提取原则

当 `App.vue` 中某部分逻辑足够复杂时，提取到 `src/components/`：

```
src/components/
├── DownloadForm.vue        # 下载表单
├── FileTree.vue            # 文件树
├── NovelMetadata.vue       # 小说元数据展示
├── AISettingsModal.vue     # AI 设置模态框
├── LogViewer.vue           # 日志查看器
└── DownloadProgress.vue    # 下载进度
```

## 导入路径别名

```typescript
// vite.config.ts
resolve: {
    alias: {
        '@': path.resolve(__dirname, './src'),
    },
}
```

```typescript
// 使用
import App from '@/App.vue';
import { invoke } from '@tauri-apps/api/core';
```

## 最佳实践

1. **单文件组件** - 使用 Vue 3 `<script setup lang="ts">` 语法
2. **按功能组织** - 相关逻辑放在一起（下载、文件、AI）
3. **组件提取** - 模板/逻辑足够复杂时提取为独立组件
4. **遵循现有模式** - 创建新功能前先参考现有代码
5. **最小依赖** - 只引入必要的 npm 包

## 相关路径

| 路径 | 说明 |
|------|------|
| `src/App.vue` | 主应用组件 |
| `src/style.css` | 全局样式 + Tailwind 指令 |
| `src-tauri/src/lib.rs` | 后端命令定义 |
| `index.html` | 入口 HTML |
