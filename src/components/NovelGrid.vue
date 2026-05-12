<script setup lang="ts">
import NovelCard, { type NovelListRow } from "./NovelCard.vue";

defineProps<{
  novels: NovelListRow[];
  novelsLoading: boolean;
  selectedNovelId: number | null;
}>();

const emit = defineEmits<{
  (e: "select", novel: NovelListRow): void;
}>();
</script>

<template>
  <div class="flex-1 overflow-y-auto custom-scrollbar">
    <div v-if="novelsLoading" class="flex items-center justify-center h-40 text-txt-dim text-xs gap-2">
      <span class="animate-spin">⏳</span>加载中…
    </div>
    <div v-else-if="novels.length === 0" class="flex items-center justify-center h-40 text-txt-dim text-xs">
      书库为空，点击 ➕ 添加或到报告 Tab 扫榜
    </div>
    <div v-else class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
      <NovelCard
        v-for="item in novels"
        :key="item.id"
        :novel="item"
        :selected="selectedNovelId === item.id"
        @click="emit('select', item)"
      />
    </div>
  </div>
</template>
