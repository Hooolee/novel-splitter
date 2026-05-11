<script setup lang="ts">
import { computed } from 'vue';

export interface AiReviewsAgent {
  vote: 'yes' | 'no' | 'maybe' | null;
  focus?: string[];
  comment?: string;
}

export interface AiReviews {
  agents?: {
    reader?: AiReviewsAgent | null;
    editor?: AiReviewsAgent | null;
    author?: AiReviewsAgent | null;
  };
  consensus?: 'all_yes' | 'all_no' | 'majority_yes' | 'majority_no' | 'divergent';
  meta?: { model?: string; generated_at?: string; input_chapters?: number };
}

export interface NovelListRow {
  id: number;
  book_id: string;
  platform: string;
  title: string;
  author: string | null;
  tags: string[];
  word_count: number | null;
  created_at: string;
  updated_at: string;
  ai_reviews: AiReviews | null;
  latest_rank: number | null;
  scan_count: number;
}

const props = defineProps<{
  novel: NovelListRow;
  selected?: boolean;
}>();

const emit = defineEmits<{
  (e: 'click', novel: NovelListRow): void;
}>();

const consensus = computed(() => props.novel.ai_reviews?.consensus);

const consensusBadge = computed(() => {
  switch (consensus.value) {
    case 'all_yes': return { label: '一致看好', cls: 'bg-green-500/20 text-green-300 border-green-500/40' };
    case 'majority_yes': return { label: '多数看好', cls: 'bg-emerald-500/20 text-emerald-300 border-emerald-500/40' };
    case 'divergent': return { label: '分歧', cls: 'bg-amber-500/20 text-amber-300 border-amber-500/40' };
    case 'majority_no': return { label: '多数不看好', cls: 'bg-rose-500/20 text-rose-300 border-rose-500/40' };
    case 'all_no': return { label: '一致不看好', cls: 'bg-red-500/20 text-red-300 border-red-500/40' };
    default: return null;
  }
});

const votes = computed(() => {
  const a = props.novel.ai_reviews?.agents;
  if (!a) return null;
  return {
    reader: a.reader?.vote ?? null,
    editor: a.editor?.vote ?? null,
    author: a.author?.vote ?? null,
  };
});

function voteIcon(v: 'yes' | 'no' | 'maybe' | null): string {
  if (v === 'yes') return '👍';
  if (v === 'no') return '👎';
  if (v === 'maybe') return '🤔';
  return '·';
}
</script>

<template>
  <div
    @click="emit('click', novel)"
    class="bg-card border border-border rounded-lg p-3 cursor-pointer transition-all hover:border-accent/50 hover:shadow-md flex flex-col gap-2"
    :class="selected ? 'border-accent ring-1 ring-accent/30' : ''"
  >
    <div class="flex items-start gap-2">
      <h4 class="flex-1 font-bold text-sm text-txt truncate" :title="novel.title">
        {{ novel.title }}
      </h4>
      <span v-if="novel.latest_rank !== null" class="text-[10px] px-1.5 py-0.5 rounded bg-accent/10 text-accent font-mono whitespace-nowrap">
        #{{ novel.latest_rank }}
      </span>
    </div>

    <div v-if="novel.tags.length > 0" class="flex flex-wrap gap-1">
      <span
        v-for="t in novel.tags.slice(0, 3)"
        :key="t"
        class="text-[10px] px-1.5 py-0.5 rounded bg-subtle border border-border-dim text-txt-dim"
      >{{ t }}</span>
      <span v-if="novel.tags.length > 3" class="text-[10px] text-txt-dim self-center">+{{ novel.tags.length - 3 }}</span>
    </div>

    <div class="flex items-center justify-between gap-2">
      <span v-if="consensusBadge" class="text-[10px] px-1.5 py-0.5 rounded border whitespace-nowrap" :class="consensusBadge.cls">
        {{ consensusBadge.label }}
      </span>
      <span v-else class="text-[10px] text-txt-dim italic">未评估</span>

      <div v-if="votes" class="flex items-center gap-1 text-sm">
        <span :title="`读者: ${votes.reader ?? '-'}`">{{ voteIcon(votes.reader) }}</span>
        <span :title="`主编: ${votes.editor ?? '-'}`">{{ voteIcon(votes.editor) }}</span>
        <span :title="`白金: ${votes.author ?? '-'}`">{{ voteIcon(votes.author) }}</span>
      </div>
    </div>

    <div class="flex justify-between text-[10px] text-txt-dim border-t border-border-dim pt-1.5 mt-auto">
      <span v-if="novel.word_count">{{ (novel.word_count / 10000).toFixed(1) }}w 字</span>
      <span v-else class="opacity-50">—</span>
      <span>上榜 {{ novel.scan_count }} 次</span>
    </div>
  </div>
</template>
