# 状态管理规范

本文档覆盖 Vue 3 状态管理模式：ref/reactive、localStorage 持久化、事件状态。

## 状态分类

| 类别 | 工具 | 使用场景 |
|------|------|----------|
| 本地 UI 状态 | `ref` | 模态框开关、输入框值 |
| 应用共享状态 | `ref` + 顶层变量 | 文件树、下载日志 |
| 持久化状态 | `ref` + localStorage | API 配置、主题偏好 |
| 后端事件状态 | `listen()` | 下载进度、AI 流式响应 |

## 响应式状态

### 基础类型

```typescript
import { ref, computed } from 'vue'

// 基础类型
const count = ref(0)
const mode = ref<'single' | 'rank'>('single')
const isDownloading = ref(false)

// 计算属性
const canDownload = computed(() => 
  url.value.length > 0 && !isDownloading.value
)
```

### 对象和数组

```typescript
// 对象
const novelMetadata = ref<NovelMetadata | null>(null)

// 数组（下载日志）
interface LogEntry {
  timestamp: string
  message: string
  level: 'info' | 'warn' | 'error'
}
const downloadLog = ref<LogEntry[]>([])
```

## localStorage 持久化

### 设置存储模式

```typescript
import { ref, watch } from 'vue'

// 加载
function loadSetting<T>(key: string, defaultValue: T): T {
  try {
    const stored = localStorage.getItem(key)
    return stored ? JSON.parse(stored) : defaultValue
  } catch {
    return defaultValue
  }
}

// 保存
function saveSetting<T>(key: string, value: T): void {
  localStorage.setItem(key, JSON.stringify(value))
}

// 使用示例
const apiKey = ref(loadSetting('apiKey', ''))
const selectedModel = ref(loadSetting('selectedModel', 'deepseek-chat'))
const customPrompt = ref(loadSetting('customPrompt', ''))

// 自动保存
watch(apiKey, (val) => saveSetting('apiKey', val))
watch(selectedModel, (val) => saveSetting('selectedModel', val))
```

### 存储键名规范

| 键名 | 类型 | 默认值 |
|------|------|--------|
| `apiKey` | string | `''` |
| `apiBaseUrl` | string | `'https://api.deepseek.com'` |
| `selectedModel` | string | `'deepseek-chat'` |
| `customPrompt` | string | `''` |
| `spiderVisible` | boolean | `false` |

## 状态通信

### 父子组件

```vue
<!-- 父传递 prop -->
<FileTree 
  :tree="fileTree" 
  :expanded-folders="expandedFolders"
  @select="handleSelect"
  @refresh="refreshTreeFiles"
/>

<!-- 子接收 prop 和 emit -->
<script setup lang="ts">
const props = defineProps<{
  tree: FileNode[]
  expandedFolders: Set<string>
}>()
const emit = defineEmits<{
  (e: 'select', path: string): void
  (e: 'refresh'): void
}>()
</script>
```

### 兄弟组件

通过共同的父组件传递状态：

```
App.vue (共享状态: selectedNovel, fileTree)
  ├── FileTree (通过 props 接收 tree)
  ├── NovelMetadata (通过 props 接收 selectedNovel)
  └── DownloadProgress (通过 props 接收 log)
```

## 状态更新模式

### 下载进度

```typescript
// Tauri 事件驱动
import { listen } from '@tauri-apps/api/event'

onMounted(() => {
  const unlisten = listen<{current: number, total: number, chapter: string}>(
    'download-progress',
    (event) => {
      downloadProgress.value = event.payload
      downloadLog.value.push({
        timestamp: new Date().toLocaleString(),
        message: `下载章节 ${event.payload.current}/${event.payload.total}: ${event.payload.chapter}`,
        level: 'info',
      })
    }
  )
  // 清理
  onUnmounted(() => { unlisten.then(fn => fn()) })
})
```

### AI 分析流

```typescript
const aiResult = ref('')

onMounted(() => {
  const unlisten = listen<{content: string}>('ai-analysis', (event) => {
    aiResult.value += event.payload.content
  })
})
```

### 文件树刷新

```typescript
async function refreshTreeFiles() {
  fileTree.value = await invoke<FileNode[]>('get_file_tree', {
    path: downloadPath
  })
}
```

## 最佳实践

1. **最小化共享状态** - 尽量保持状态在局部
2. **统一刷新入口** - 文件树刷新通过 `refreshTreeFiles()`
3. **自动保存** - 使用 `watch` 自动持久化设置
4. **清理监听** - 组件卸载时取消 Tauri 事件监听
5. **类型安全** - 所有状态有 TypeScript 类型

## 常见陷阱

- 状态不同步 → 操作后手动调用 `refreshTreeFiles()`
- 事件泄漏 → 组件卸载时未取消 `listen()`
- 重复设置 → 多个 `watch` 互相触发
- 过度持久化 → 只持久化用户配置，不持久化临时状态
- JSON 解析失败 → `loadSetting` 有 try-catch 兜底
