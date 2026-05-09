# 类型安全规范

本文档覆盖 TypeScript 在前端的最佳实践。

## 核心原则

1. **接口定义在需要的地方** - 与后端共享的类型定义在项目内统一
2. **避免 any** - 不允许使用 `any` 类型
3. **利用类型推导** - TypeScript 能推导的类型不手动定义

## 接口定义

### 小说元数据

```typescript
interface NovelMetadata {
  title: string
  url: string
  tags: string[]
  word_count: string
  description: string
  ai_analysis?: {
    genre: string
    style: string
    goldfinger: string
    opening: string
    highlights: string
  }
}
```

### 文件树节点

```typescript
interface FileNode {
  name: string
  path: string
  is_dir: boolean
  children?: FileNode[]
}
```

### 事件 Payload

```typescript
interface DownloadProgress {
  current: number
  total: number
  chapter: string
}

interface AIAnalysisChunk {
  content: string
}

interface AIAnalysisStatus {
  status: 'start' | 'complete' | 'error'
  message: string
}
```

### 应用设置

```typescript
interface AppSettings {
  apiKey: string
  apiBaseUrl: string
  selectedModel: string
  customPrompt: string
  spiderVisible: boolean
}
```

## 通用模式

### 函数签名

```typescript
// BAD - 缺少类型
function process(data) { ... }

// GOOD - 完整类型
function process(data: NovelMetadata): void { ... }

// GOOD - 类型参数
async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> { ... }
```

### 联合类型

```typescript
// 状态枚举
type DownloadStatus = 'idle' | 'downloading' | 'complete' | 'error'
type AppMode = 'single' | 'rank'
type ThemeMode = 'light' | 'dark'

// 使用
const status = ref<DownloadStatus>('idle')
```

### 类型推导

```typescript
// GOOD - 利用推导
const downloadLog = ref([
  { timestamp: '...', message: '...', level: 'info' as const }
])

// 类型自动推导为 Ref<{timestamp: string, message: string, level: 'info'}[]>
```

## 与 invoke 的类型安全

```typescript
// BAD - 无类型
const result = await invoke('get_file_tree', { path })

// GOOD - 指定返回类型
const tree = await invoke<FileNode[]>('get_file_tree', { path })

// BETTER - 封装为类型安全的函数
async function getFileTree(path: string): Promise<FileNode[]> {
  return invoke<FileNode[]>('get_file_tree', { path })
}
```

## 响应式类型

```typescript
// ref 类型
const count = ref(0)              // Ref<number>
const name = ref('')              // Ref<string>
const data = ref<NovelMetadata | null>(null)

// reactive 类型
const settings = reactive<AppSettings>({
  apiKey: '',
  apiBaseUrl: 'https://api.deepseek.com',
  selectedModel: 'deepseek-chat',
  customPrompt: '',
  spiderVisible: false,
})
```

## 错误处理类型

```typescript
// Tauri invoke 错误通常是 string
async function safeInvoke<T>(
  command: string,
  args?: Record<string, unknown>
): Promise<{ data?: T; error?: string }> {
  try {
    const data = await invoke<T>(command, args)
    return { data }
  } catch (e) {
    return { error: typeof e === 'string' ? e : '未知错误' }
  }
}
```

## 最佳实践

1. **所有接口有类型** - 函数参数、变量、ref 都有类型
2. **不用 any** - 除非绝对必要（几乎不会）
3. **类型定义位置** - 接口集中在组件顶部或独立 types 文件
4. **利用 const 断言** - `as const` 用于字面量类型推导
5. **类型兼容** - 前端类型与后端 Rust 结构体保持一致

## 常见陷阱

- `any` 类型泛滥 → 使用具体类型或 `unknown` + 类型守卫
- 类型与后端不匹配 → 确保 `invoke` 返回类型与 Rust 定义一致
- 过度定义 → TypeScript 能推导的类型不重复定义
- 事件 payload 类型错误 → 确保 `listen<T>` 中的 T 匹配事件数据结构
