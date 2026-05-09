# 组合式函数 (Composables) 开发规范

本文档覆盖 Vue 3 组合式函数模式，用于封装可复用的有状态逻辑。

## 基本模式

### 基础 Composable

```typescript
// src/composables/useDownload.ts
import { ref, readonly } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export function useDownload() {
  const isDownloading = ref(false)
  const error = ref<string | null>(null)

  async function startDownload(url: string, count: number, platform: string) {
    isDownloading.value = true
    error.value = null
    try {
      await invoke('start_download', { url, chapterCount: count, platform })
    } catch (e) {
      error.value = typeof e === 'string' ? e : '下载失败'
    } finally {
      isDownloading.value = false
    }
  }

  return {
    isDownloading: readonly(isDownloading),
    error: readonly(error),
    startDownload,
  }
}
```

### 文件树 Composable

```typescript
// src/composables/useFileTree.ts
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

interface FileNode {
  name: string
  path: string
  is_dir: boolean
  children?: FileNode[]
}

export function useFileTree() {
  const tree = ref<FileNode[]>([])
  const selectedPath = ref<string | null>(null)
  const expandedFolders = ref<Set<string>>(new Set())

  async function refresh(path: string) {
    tree.value = await invoke<FileNode[]>('get_file_tree', { path })
  }

  function toggleFolder(path: string) {
    const next = new Set(expandedFolders.value)
    if (next.has(path)) {
      next.delete(path)
    } else {
      next.add(path)
    }
    expandedFolders.value = next
  }

  function select(path: string) {
    selectedPath.value = path
  }

  return {
    tree: readonly(tree),
    selectedPath: readonly(selectedPath),
    expandedFolders: readonly(expandedFolders),
    refresh,
    toggleFolder,
    select,
  }
}
```

### AI 分析 Composable

```typescript
// src/composables/useAIAnalysis.ts
import { ref, readonly } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export function useAIAnalysis() {
  const isAnalyzing = ref(false)
  const result = ref('')
  const status = ref<'idle' | 'running' | 'complete' | 'error'>('idle')

  async function startAnalysis(novelPath: string) {
    isAnalyzing.value = true
    status.value = 'running'
    result.value = ''

    // 监听流式响应
    const unlisten = await listen<{content: string}>('ai-analysis', (event) => {
      result.value += event.payload.content
    })

    try {
      await invoke('start_ai_analysis', { novelPath })
      status.value = 'complete'
    } catch (e) {
      status.value = 'error'
      result.value = typeof e === 'string' ? e : '分析失败'
    } finally {
      isAnalyzing.value = false
      unlisten()
    }
  }

  return {
    isAnalyzing: readonly(isAnalyzing),
    result: readonly(result),
    status: readonly(status),
    startAnalysis,
  }
}
```

## 现有的内联逻辑

目前项目中的逻辑主要写在 `App.vue` 内。当需要提取为 composable 时的候选：

| 内联逻辑 | 建议 Composable | 功能 |
|----------|----------------|------|
| 下载逻辑 | `useDownload` | invoke start_download, 进度管理 |
| 文件树逻辑 | `useFileTree` | 刷新、展开、选择 |
| AI 分析 | `useAIAnalysis` | invoke + 事件流处理 |
| 设置管理 | `useSettings` | localStorage 读写 |
| 主题切换 | `useTheme` | 深色/浅色模式切换 |

## 最佳实践

1. **单一职责** - 每个 composable 一个功能
2. **返回 readonly** - 暴露状态时使用 `readonly()` 防止外部修改
3. **清理资源** - 事件监听在 composable 内部管理生命周期
4. **错误处理** - 捕获并暴露错误状态
5. **命名规范** - 以 `use` 开头，camelCase

## 常见陷阱

- 忘记返回 cleanup → 事件监听泄漏
- 返回可变引用 → 外部直接修改导致状态不一致（使用 `readonly`）
- 过度抽象 → 只在逻辑重复 2+ 次时提取 composable
- 忽略错误处理 → 未暴露错误状态给 UI
