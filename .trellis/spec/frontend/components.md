# 组件开发规范

本文档覆盖 Vue 3 组件开发模式、Props、事件和代码组织。

## 组件语法

### 使用 `<script setup lang="ts">`

所有新组件默认使用 Composition API + `<script setup>`：

```vue
<script setup lang="ts">
import { ref, computed } from 'vue'

// Props 定义
const props = defineProps<{
  title: string
  count?: number
}>()

// 事件定义
const emit = defineEmits<{
  (e: 'update', value: string): void
  (e: 'close'): void
}>()

// 响应式状态
const localCount = ref(0)
const doubleCount = computed(() => localCount.value * 2)
</script>

<template>
  <div class="p-4">
    <h2>{{ title }}</h2>
    <button @click="emit('update', 'new')">更新</button>
  </div>
</template>
```

## 单文件组件 (SFC)

每个功能一个文件，模板 + 脚本 + 样式在一起：

```vue
<script setup lang="ts">
// 逻辑
</script>

<template>
<!-- HTML 模板 -->
</template>

<style scoped>
/* 组件作用域样式 */
</style>
```

## Props 定义

### 使用 TypeScript 类型

```typescript
// 推荐 - 类型安全
const props = defineProps<{
  novelPath: string
  metadata: NovelMetadata | null
  expanded?: boolean  // 可选 prop
}>()
```

### 使用运行时声明

```typescript
// 当需要更多验证时
const props = defineProps({
  title: { type: String, required: true },
  count: { type: Number, default: 0 },
  tags: { type: Array as PropType<string[]>, default: () => [] },
})
```

## 事件

### 自定义事件

```vue
<script setup lang="ts">
const emit = defineEmits<{
  (e: 'select', path: string): void
  (e: 'delete', path: string): void
  (e: 'refresh'): void
}>()

function handleSelect(path: string) {
  emit('select', path)
}
</script>
```

## 双向绑定 (v-model)

```vue
<script setup lang="ts">
const model = defineModel<string>()

// 带验证
const count = defineModel<number>('count', { required: true })
</script>

<template>
  <input v-model="model" />
  <input v-model="count" />
</template>
```

## 插槽

```vue
<script setup lang="ts">
// 默认插槽 + 具名插槽
</script>

<template>
  <div class="card">
    <header>
      <slot name="header">默认标题</slot>
    </header>
    <main>
      <slot />  <!-- 默认插槽 -->
    </main>
    <footer>
      <slot name="footer" />
    </footer>
  </div>
</template>
```

## 条件渲染

```vue
<!-- 使用 v-if 而非 v-show 控制渲染 -->
<AISettingsModal v-if="showSettings" @close="showSettings = false" />

<!-- 使用 v-show 用于频繁切换显示状态 -->
<div v-show="isLoading">加载中...</div>
```

## 循环渲染

```vue
<!-- 列表渲染加 key -->
<div v-for="item in items" :key="item.path">
  {{ item.name }}
</div>
```

## 组件 vs 内联

何时提取为独立组件：

| 信号 | 建议 |
|------|------|
| 模板超过 50 行 | 提取部分到子组件 |
| 逻辑块可复用 | 提取为 composable |
| 模态框内容复杂 | 提取为独立组件 |
| 需要多次渲染 | 提取为组件 |

当前项目中可提取的候选组件：

- `DownloadForm.vue` - 下载表单区域
- `FileTree.vue` - 文件浏览器
- `NovelMetadata.vue` - 小说信息展示
- `AISettingsModal.vue` - AI 设置对话框
- `LogViewer.vue` - 日志查看器
- `DownloadProgress.vue` - 下载进度条

## 最佳实践

1. **默认 `<script setup>`** - 所有新组件使用
2. **类型化 Props/Events** - 使用 TypeScript 类型
3. **单文件组件** - 一个组件一个文件
4. **scoped 样式** - 默认使用 scoped styles
5. **组件命名** - 使用 PascalCase

## 常见陷阱

- 忘记给 `v-for` 加 `key` → 渲染性能问题和状态错误
- 不使用 `scoped` 导致样式污染 → 默认加 `scoped`
- 大型组件不分拆 → 超过 300 行就要考虑拆分
-  Props 无类型 → 使用 TypeScript defineProps
