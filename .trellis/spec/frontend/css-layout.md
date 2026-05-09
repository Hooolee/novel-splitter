# CSS 与布局最佳实践

本文档覆盖 Tailwind CSS 模式、布局策略和主题系统。

## Tailwind CSS

### 全局样式入口

```css
/* src/style.css */
@tailwind base;
@tailwind components;
@tailwind utilities;
```

## 主题系统

项目使用自定义 CSS 变量实现深色/浅色主题切换：

```css
/* 浅色主题 (默认) */
:root {
  --bg: #ffffff;
  --sidebar: #f8f9fa;
  --card: #ffffff;
  --txt: #1a1a1a;
  --txt-dim: #6b7280;
  --accent: #3b82f6;
  --border: #e5e7eb;
  --border-dim: #f3f4f6;
  --hover: #f3f4f6;
  --subtle: #f9fafb;
  --active-bg: #eff6ff;
  --input: #ffffff;
}

/* 深色主题 */
.dark {
  --bg: #1a1a2e;
  --sidebar: #16213e;
  --card: #0f3460;
  --txt: #e8e8e8;
  --txt-dim: #9ca3af;
  --accent: #60a5fa;
  --border: #374151;
  --border-dim: #1f2937;
  --hover: #1f2937;
  --subtle: #111827;
  --active-bg: #1e3a5f;
  --input: #1f2937;
}
```

### 使用主题变量

```vue
<div class="bg-[var(--bg)] text-[var(--txt)]">
  <aside class="bg-[var(--sidebar)] border-r border-[var(--border)]">
    <!-- 侧边栏 -->
  </aside>
  <main class="bg-[var(--bg)]">
    <!-- 主内容 -->
  </main>
</div>
```

### 主题切换

```typescript
const isDark = ref(false)

function toggleTheme() {
  isDark.value = !isDark.value
  document.documentElement.classList.toggle('dark')
}
```

## 布局模式

### 响应式两栏布局

```html
<div class="flex h-screen">
  <!-- 侧边栏：固定宽度 -->
  <aside class="w-72 shrink-0 overflow-y-auto border-r border-[var(--border)]">
    <!-- 文件树 -->
  </aside>
  <!-- 主内容：弹性填充 -->
  <main class="flex-1 overflow-y-auto">
    <!-- 内容 -->
  </main>
</div>
```

### 卡片组件

```vue
<div class="rounded-lg border border-[var(--border)] bg-[var(--card)] p-4 shadow-sm">
  <slot />
</div>
```

### 内联表单

```html
<div class="flex items-center gap-2">
  <input class="flex-1 rounded border border-[var(--border)] bg-[var(--input)] px-3 py-2 text-sm" />
  <button class="rounded bg-[var(--accent)] px-4 py-2 text-sm text-white hover:opacity-90">
    下载
  </button>
</div>
```

## 布局原则

### 父组件提供外部样式

父控制：定位、间距、尺寸约束

```html
<div class="grid grid-cols-1 gap-4 p-6 lg:grid-cols-2">
  <FileTree class="col-span-1" />
  <NovelMetadata class="col-span-1" />
</div>
```

### 子组件提供内部布局

子控制：内边距、内部 flex、背景色

```vue
<!-- NovelMetadata.vue -->
<div class="rounded-lg border bg-[var(--card)] p-6">
  <h2 class="text-lg font-semibold">小说信息</h2>
  <!-- 内部布局 -->
</div>
```

## 响应式设计

```html
<!-- 移动端优先 -->
<div class="
  p-4           <!-- 移动端 -->
  md:p-6        <!-- 平板 -->
  lg:p-8        <!-- 桌面 -->
">
  <h1 class="
    text-xl
    md:text-2xl
    lg:text-3xl
  ">
    标题
  </h1>
</div>
```

## 模态框样式

```vue
<!-- 设置模态框 -->
<Teleport to="body">
  <div v-if="show" class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
    <div class="w-full max-w-lg rounded-xl bg-[var(--card)] p-6 shadow-xl">
      <div class="mb-4 flex items-center justify-between">
        <h2 class="text-lg font-semibold">设置</h2>
        <button @click="$emit('close')" class="text-[var(--txt-dim)] hover:text-[var(--txt)]">✕</button>
      </div>
      <slot />
    </div>
  </div>
</Teleport>
```

## 最佳实践

1. **主题一致性** - 始终使用 CSS 变量而非硬编码颜色
2. **语义化** - Tailwind 类优先级：布局 > 间距 > 颜色 > 字体
3. **深色模式** - 默认支持深色模式，所有颜色通过变量控制
4. **响应式** - 基本布局适配不同窗口大小
5. **最小化自定义 CSS** - 优先使用 Tailwind 工具类

## 常见陷阱

- 硬编码颜色 → 深色模式无法适配
- 固定高度 → 内容溢出（使用 `min-h` 而非 `h`）
- 忽略 `overflow` → 内容超出容器不可见
- 过度嵌套 flex → 布局复杂度增加
