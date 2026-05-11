# Tauri 集成规范

本文档覆盖前端与 Tauri 后端的通信模式：invoke 调用、事件监听、文件操作。

## invoke 命令调用

### 基本模式

```typescript
import { invoke } from '@tauri-apps/api/core'

// 无参数
const result = await invoke<string>('read_log_file')

// 有参数
const tree = await invoke<FileNode[]>('get_file_tree', {
  path: downloadPath,
})
```

### 错误处理

```typescript
async function safeInvoke<T>(fn: () => Promise<T>): Promise<[T | null, string | null]> {
  try {
    const result = await fn()
    return [result, null]
  } catch (e) {
    const message = typeof e === 'string' ? e : '操作失败'
    return [null, message]
  }
}

// 使用
const [data, error] = await safeInvoke(() => 
  invoke<FileNode[]>('get_file_tree', { path })
)
if (error) {
  // 显示错误
}
```

## 事件监听

### 基本模式

```typescript
import { listen } from '@tauri-apps/api/event'
import { onMounted, onUnmounted } from 'vue'

// 监听单个事件
let unlisten: (() => void) | null = null

onMounted(async () => {
  unlisten = await listen<PayloadType>('event-name', (event) => {
    // 处理事件
    progress.value = event.payload
  })
})

onUnmounted(() => {
  unlisten?.()
})
```

### 批量监听

```typescript
const cleanups: (() => void)[] = []

onMounted(async () => {
  // 批量监听，统一清理
  cleanups.push(
    (await listen('download-progress', onProgress)).toString() as any
  )
  cleanups.push(
    (await listen('ai-analysis', onAnalysis)).toString() as any
  )
  cleanups.push(
    (await listen('ai-analysis-status', onStatus)).toString() as any
  )
})

onUnmounted(() => {
  cleanups.forEach(fn => fn())
})
```

## 可用命令列表

| 命令名 | 参数 | 返回 | 说明 |
|--------|------|------|------|
| `start_download` | `{ url, chapterCount, platform, spiderVisible }` | `void` | 开始下载小说 |
| `scan_and_download_rank` | `{ url, platform, spiderVisible }` | `void` | 榜单下载 |
| `start_ai_analysis` | `{ novelPath }` | `void` | 开始 AI 分析 |
| `get_file_tree` | `{ path }` | `FileNode[]` | 获取文件树 |
| `get_file_content` | `{ path }` | `string` | 读取文件内容 |
| `export_chapter` | `{ chapterPath, content }` | `void` | 导出章节 |
| `update_novel_metadata` | `{ novelPath, metadata }` | `void` | 更新元数据 |
| `delete_novel` | `{ novelPath }` | `void` | 删除小说 |
| `delete_chapter` | `{ chapterPath }` | `void` | 删除章节 |
| `read_log_file` | — | `string` | 读取日志 |
| `clear_log` | — | `void` | 清空日志 |
| `trigger_full_scan` | `{ targetUrl: string \| null, platform: string \| null }` | `void` | V2.0 触发流水线（榜单/单本由 URL 推断 mode） |
| `evaluate_novel` | `{ novelId: number }` | `string` | V2.0 单本重跑多 Agent 评估，返回 ai_reviews JSON |
| `list_novels` | `{ filter?: NovelListFilter }` | `NovelListRow[]` | V2.0 任务四a 书库卡片查询，含 parsed ai_reviews + latest_rank + scan_count |

`NovelListFilter` schema：

```typescript
interface NovelListFilter {
  tags: string[];                 // OR 语义（任一命中即收）
  consensus: ConsensusKey[];      // OR
  platform: string | null;
  sort_by: 'updated_desc' | 'latest_rank_asc' | 'scan_count_desc';
}
```

`NovelListRow` 含 `id/book_id/platform/title/author/tags/word_count/created_at/updated_at` + 派生字段 `ai_reviews/latest_rank/scan_count`（在 `src/components/NovelCard.vue` 内 `export interface`，App.vue 复用）。


## 可用事件列表

| 事件名 | Payload 类型 | 说明 |
|--------|-------------|------|
| `download-progress` | `{ current: number, total: number, chapter: string }` | V1 下载进度（老命令 `start_download` / `scan_and_download_rank` 发，UI 不再绑定） |
| `ai-analysis` | `{ content: string }` | AI 流式响应（单本拆书） |
| `ai-analysis-status` | `{ status: string, message: string }` | AI 状态（单本拆书） |
| `pipeline-progress` | `{ phase: number, status: 'started'\|'completed'\|'failed', message: string, progress: [number, number] \| null }` | V2.0 流水线阶段事件，phase ∈ 1..=4：1=Producer 扫榜/单本元数据，2=Fetch 抓章节，3=AI 提纯，4=Multi-Agent 评估。`progress` 为 `(done, total)` 元组或 null |
| `report-generated` | `()` | V2.0 流水线最终完成信号（前端用作 isDownloading reset） |

### `pipeline-progress` 监听示例

```typescript
interface PipelineProgress {
    phase: number;
    status: 'started' | 'completed' | 'failed';
    message: string;
    progress: [number, number] | null;
}

const currentPhase = ref<PipelineProgress | null>(null);

listen<PipelineProgress>('pipeline-progress', (event) => {
    currentPhase.value = event.payload;
    // 在 phase 2 completed 时刷文件树
    if (event.payload.phase === 2 && event.payload.status === 'completed') {
        refreshTreeFiles();
    }
});
```

事件来源：`src-tauri/src/analysis_engine.rs::run_full_analysis_pipeline` 在每个 Phase 边界 emit。

## 文件操作模式

### 刷新文件树

```typescript
// 在删除或下载后调用
async function refreshTreeFiles() {
  try {
    fileTree.value = await invoke<FileNode[]>('get_file_tree', {
      path: downloadPath,
    })
  } catch (e) {
    console.error('刷新文件树失败:', e)
  }
}
```

### 读取和保存

```typescript
// 读取文件
async function loadContent(path: string) {
  novelContent.value = await invoke<string>('get_file_content', { path })
}

// 保存元数据
async function saveMetadata(novelPath: string, metadata: NovelMetadata) {
  await invoke('update_novel_metadata', { novelPath, metadata })
}
```

## 最佳实践

1. **类型安全** - invoke 命令始终指定返回类型 `<T>`
2. **错误处理** - 每个 invoke 调用包裹 try-catch
3. **清理事件** - 组件卸载时取消所有事件监听
4. **统一刷新** - 文件变更后调用 `refreshTreeFiles()`
5. **超时考虑** - 长时间操作有用户反馈（加载状态）

## 常见陷阱

- 命令名拼写错误 → 前端调用 camelCase，后端 snake_case (Tauri 自动转换)
- 忘记清理监听 → 组件卸载后仍收到事件
- 类型不匹配 → 后端返回需匹配前端类型定义
- 忽略错误 → 下载失败时用户看不到错误信息
