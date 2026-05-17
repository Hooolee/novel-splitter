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
    <div v-if="novelsLoading" class="paper-panel rounded-[20px] flex items-center justify-center h-48 text-txt-dim text-sm gap-3">
      <span class="animate-spin text-lg">⏳</span><span>正在整理书库卡片…</span>
    </div>
    <div v-else-if="novels.length === 0" class="paper-panel rounded-[20px] h-[520px] flex flex-col items-center justify-center text-center px-6 text-txt-dim bg-[linear-gradient(180deg,rgba(255,251,245,0.98),rgba(249,241,230,0.94))]">
      <div class="text-[84px] mb-4 opacity-80">📚</div>
      <div class="text-[28px] font-semibold text-txt mb-3 font-serif">书库尚未点亮</div>
      <div class="max-w-[520px] text-[16px] leading-8">
        点击上方 <span class="text-accent font-medium">“添加书籍”</span> 导入单本，
        或切换到 <span class="text-accent font-medium">“拆书雷达”</span> 先扫榜，再回到这里沉淀高潜书目。
      </div>
    </div>
    <div v-else class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4 gap-5">
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
