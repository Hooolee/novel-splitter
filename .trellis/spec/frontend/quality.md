# 提交流程检查清单

提交前端代码前执行以下检查。

## 类型安全

- [ ] 无 `any` 类型
- [ ] `invoke()` 调用指定了返回类型
- [ ] 事件 `listen<T>()` 指定了 payload 类型
- [ ] ref/reactive 都有类型声明
- [ ] 接口与后端 Rust 结构体字段一致

## 状态管理

- [ ] 不需要持久化的未使用 localStorage
- [ ] 组件卸载时取消事件监听
- [ ] 设置类状态使用 `watch` 自动保存
- [ ] 文件操作后调用 `refreshTreeFiles()`

## 组件开发

- [ ] 使用 `<script setup lang="ts">`
- [ ] Props/Events 有 TypeScript 类型
- [ ] 大型组件（>300行）考虑拆分
- [ ] `v-for` 有 `:key`

## Tauri 集成

- [ ] 所有 `invoke` 调用有 try-catch 或错误处理
- [ ] `onUnmounted` 中清理事件监听
- [ ] 命令名与 Rust 端匹配
- [ ] 长时间操作有加载状态显示

## CSS 与主题

- [ ] 使用 CSS 变量而非硬编码颜色
- [ ] 深色模式正常显示
- [ ] 布局在不同窗口大小下正常

## 代码质量

- [ ] 无 `console.log` 留在代码中
- [ ] 未使用的 import 已移除
- [ ] 一致的命名风格
- [ ] 遵循现有代码模式
- [ ] 复杂任务有测试用例，执行前明确测试条件和预期结果

## 快速命令

```bash
# 类型检查
npx vue-tsc --noEmit

# 构建检查
npm run build

# 开发测试
npm run tauri dev
```

## 常见问题

### invoke 调用
```typescript
// BAD - 无错误处理
const tree = await invoke('get_file_tree', { path })

// GOOD - 处理错误
try {
  const tree = await invoke<FileNode[]>('get_file_tree', { path })
} catch (e) {
  errorMsg.value = typeof e === 'string' ? e : '加载失败'
}
```

### 事件监听
```typescript
// BAD - 不清理
onMounted(async () => {
  await listen('download-progress', handler)
})

// GOOD - 清理监听
onMounted(async () => {
  const unlisten = await listen('download-progress', handler)
  onUnmounted(() => { unlisten() })
})
```
