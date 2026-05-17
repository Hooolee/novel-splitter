<script setup lang="ts">
import { computed } from 'vue';

export interface AiReviewsAgent {
  vote: 'yes' | 'no' | 'maybe' | null;
  focus?: string[];
  comment?: string;
}

export interface AiReviewsBreakdown {
  goldfinger_type?: string;
  protagonist_archetype?: string;
  opening_hook?: string;
  hook_density?: string;
  pacing_notes?: string;
  chapter_end_hook_types?: string[] | string;
  learning_points?: string[] | string;
}

export interface AiReviews {
  agents?: {
    reader?: AiReviewsAgent | null;
    editor?: AiReviewsAgent | null;
    author?: AiReviewsAgent | null;
  };
  consensus?: 'all_yes' | 'all_no' | 'majority_yes' | 'majority_no' | 'divergent';
  breakdown?: AiReviewsBreakdown | null;
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
    case 'all_yes': return { label: '一致看好', cls: 'border-[#9bb47d] bg-[#f1f6e8] text-[#68834d]' };
    case 'majority_yes': return { label: '多数看好', cls: 'border-[#b3c68e] bg-[#f5f8ed] text-[#779255]' };
    case 'divergent': return { label: '分歧', cls: 'border-[#efc48f] bg-[#fff6ea] text-[#d27f33]' };
    case 'majority_no': return { label: '多数不看好', cls: 'border-[#efb0a1] bg-[#fff1ee] text-[#d8654f]' };
    case 'all_no': return { label: '一致不看好', cls: 'border-[#e79d92] bg-[#fff0ed] text-[#c95144]' };
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
  <article
    @click="emit('click', novel)"
    class="paper-panel rounded-[18px] p-4 cursor-pointer transition-all duration-200 hover:-translate-y-0.5 hover:shadow-[0_10px_22px_rgba(170,119,82,0.14)] flex flex-col gap-3 min-h-[238px]"
    :class="selected ? 'border-[#c96c3a] ring-1 ring-[#d98051]/35 shadow-[0_14px_30px_rgba(201,108,58,0.18)]' : ''"
  >
    <div class="flex items-start justify-between gap-3">
      <div class="min-w-0 space-y-1.5">
        <div class="flex items-center gap-2 text-[11px] text-txt-dim">
          <span class="chip-warm px-2 py-0.5 text-[11px]">{{ novel.platform }}</span>
          <span>{{ novel.scan_count }} 次上榜</span>
        </div>
        <h4 class="font-semibold text-[18px] leading-[1.35] text-txt line-clamp-2 tracking-[0.01em]" :title="novel.title">
          {{ novel.title }}
        </h4>
        <div v-if="novel.author" class="text-[13px] text-txt-dim truncate">{{ novel.author }}</div>
      </div>
      <span v-if="novel.latest_rank !== null" class="rounded-[12px] border border-[#edd7bd] bg-[#fff8ef] px-3 py-1 text-[13px] font-semibold text-[#7e5c44] shadow-[inset_0_1px_0_rgba(255,255,255,0.7)]">
        # {{ novel.latest_rank }}
      </span>
    </div>

    <div v-if="novel.tags.length > 0" class="flex flex-wrap gap-1.5">
      <span
        v-for="t in novel.tags.slice(0, 3)"
        :key="t"
        class="chip-warm text-[11px] px-2 py-1 rounded-[10px]"
      >{{ t }}</span>
      <span v-if="novel.tags.length > 3" class="text-[12px] text-txt-dim self-center">+{{ novel.tags.length - 3 }}</span>
    </div>

    <div class="mt-auto space-y-3">
      <div class="flex items-center justify-between gap-2">
        <span v-if="consensusBadge" class="text-[12px] px-3 py-1 rounded-[10px] border whitespace-nowrap font-medium" :class="consensusBadge.cls">
          {{ consensusBadge.label }}
        </span>
        <span v-else class="text-[12px] text-txt-dim italic">尚未评估</span>

        <div v-if="votes" class="flex items-center gap-1.5 text-[14px] rounded-[10px] bg-[#fff8ef] border border-[#ecd6bf] px-2.5 py-1">
          <span :title="`读者: ${votes.reader ?? '-'}`">{{ voteIcon(votes.reader) }}</span>
          <span :title="`主编: ${votes.editor ?? '-'}`">{{ voteIcon(votes.editor) }}</span>
          <span :title="`白金: ${votes.author ?? '-'}`">{{ voteIcon(votes.author) }}</span>
        </div>
      </div>

      <div class="flex items-center justify-between gap-3 border-t border-border-dim/80 pt-3">
        <div class="flex flex-col text-[11px] text-txt-dim leading-tight gap-1">
          <span v-if="novel.word_count">{{ (novel.word_count / 10000).toFixed(1) }} 万字</span>
          <span v-else class="opacity-50">字数待同步</span>
          <span>最新排名与评估同步</span>
        </div>
        <button
          @click.stop="emit('click', novel)"
          class="paper-btn px-3.5 py-2 text-[12px] font-medium text-accent rounded-[10px]"
        >
          🔍 对标拆书
        </button>
      </div>
    </div>
  </article>
</template>
