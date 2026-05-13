<script setup lang="ts">
import { ref, onMounted, nextTick, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { marked } from "marked";
import { type NovelListRow, type AiReviewsBreakdown } from "./components/NovelCard.vue";
import NovelGrid from "./components/NovelGrid.vue";

type ConsensusKey = NonNullable<NovelListRow['ai_reviews']>['consensus'];
type SortBy = 'updated_desc' | 'latest_rank_asc' | 'scan_count_desc';

interface NovelListFilter {
    tags: string[];
    consensus: ConsensusKey[];
    platform: string | null;
    sort_by: SortBy;
}

// 配置 marked：为标题生成 id
const renderer = new marked.Renderer();
renderer.heading = ({ text, depth }: { text: string; depth: number }) => {
    const id = text.replace(/<[^>]*>/g, '').replace(/[^\w\u4e00-\u9fff]+/g, '-').replace(/^-|-$/g, '');
    return `<h${depth} id="heading-${id}">${text}</h${depth}>`;
};
marked.setOptions({ renderer });

// --- Workspace State ---
// workspaceRoot is the user-selected base directory
// downloads go to {workspaceRoot}/downloads/
// logs go to {workspaceRoot}/logs/
const workspaceRoot = ref(localStorage.getItem('workspace_root') || '');

// --- State ---
// V2.0 流水线由 trigger_full_scan 驱动；前端只保留「+ 添加书籍」入口的轻量配置。
const newBookPlatform = ref<'qidian' | 'fanqie'>('qidian');

// --- Tab Navigation ---
const activeTab = ref<'library' | 'reports'>('library');

// --- Reports State ---
const reportFiles = ref<string[]>([]);
const selectedReport = ref<string | null>(null);
const reportContent = ref('');
const rankOptions = ref<{ value: string; label: string }[]>([]);
const workflowConfig = ref<WorkflowConfigPayload | null>(null);

// 从报告内容中提取目录
const reportToc = computed(() => {
    if (!reportContent.value) return [];
    const lines = reportContent.value.split('\n');
    const toc: { level: number; text: string; id: string }[] = [];
    for (const line of lines) {
        const match = line.match(/^(#{1,4})\s+(.+)/);
        if (match) {
            const text = match[2].replace(/\*\*/g, '').trim();
            const id = 'heading-' + text.replace(/[^\w\u4e00-\u9fff]+/g, '-').replace(/^-|-$/g, '');
            toc.push({ level: match[1].length, text, id });
        }
    }
    return toc;
});

function scrollToHeading(id: string) {
    const el = document.getElementById(id);
    if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
}

// 「+ 添加书籍」模态框
const showAddBookModal = ref(false);
const newBookUrl = ref("");

// 流水线阶段事件（pipeline-progress payload）
interface PipelineProgress {
    phase: number;                          // 1=Producer 2=Fetch 3=AI Outline 4=Multi-Agent
    status: 'started' | 'completed' | 'failed';
    message: string;
    progress: [number, number] | null;      // (done, total)
}

interface ScanRunStatus {
    status: 'completed' | 'failed';
    message: string;
}

interface WorkflowConfigPayload {
    enabled: boolean;
    schedule_time: string;
    rank_urls: string[];
}
const currentPhase = ref<PipelineProgress | null>(null);

const isDownloading = ref(false);
const isEvaluatingNovel = ref(false);

const downloadLog = ref<string[]>([]);

interface RisingNovelRow {
    id: number;
    title: string;
    platform: string;
    rank: number;
    rank_change: string;
    tags: string[];
}

interface RiskTagRow {
    tag: string;
    avg_weight: number;
    sample_count: number;
}

// File Tree State
// --- Library DB cards (任务四a) ---
const novels = ref<NovelListRow[]>([]);
// Template helper: vue-tsc workaround for v-for type narrowing
// _novels not needed
const novelsLoading = ref(false);
const tagFilter = ref<string[]>([]);
const consensusFilter = ref<ConsensusKey[]>([]);
const sortBy = ref<SortBy>('updated_desc');
const selectedNovel = ref<NovelListRow | null>(null);

const allTags = computed(() => {
    const set = new Set<string>();
    for (const n of novels.value) for (const t of n.tags) if (t) set.add(t);
    return Array.from(set).sort();
});

const CONSENSUS_OPTIONS: { value: ConsensusKey; label: string }[] = [
    { value: 'all_yes',      label: '一致看好' },
    { value: 'majority_yes', label: '多数看好' },
    { value: 'divergent',    label: '分歧' },
    { value: 'majority_no',  label: '多数不看好' },
    { value: 'all_no',       label: '一致不看好' },
];

// --- Insights panel (任务四b) ---
const CONSENSUS_WEIGHT: Record<string, number> = {
    all_yes: 2, majority_yes: 1.5, divergent: 0, majority_no: -1, all_no: -2,
};
const tagWeightMap = computed(() => {
    const map = new Map<string, number>();
    for (const n of novels.value) {
        const w = CONSENSUS_WEIGHT[n.ai_reviews?.consensus ?? ''] ?? 0;
        for (const t of n.tags) {
            if (!t) continue;
            map.set(t, (map.get(t) ?? 0) + w);
        }
    }
    return map;
});

const topTags = computed(() => {
    return Array.from(tagWeightMap.value.entries())
        .sort((a, b) => b[1] - a[1])
        .slice(0, 15);
});


const focusCountMap = computed(() => {
    const map = new Map<string, number>();
    for (const n of novels.value) {
        const agents = n.ai_reviews?.agents;
        if (!agents) continue;
        for (const key of ['reader', 'editor', 'author'] as const) {
            const agent = agents[key];
            if (!agent?.focus) continue;
            for (const f of agent.focus) {
                if (!f) continue;
                map.set(f, (map.get(f) ?? 0) + 1);
            }
        }
    }
    return map;
});

const topFocus = computed(() => {
    return Array.from(focusCountMap.value.entries())
        .sort((a, b) => b[1] - a[1])
        .slice(0, 10);
});

const selectedNovelId = computed((): number | null => selectedNovel.value?.id ?? null);

const consensusDistribution = computed(() => {
    const dist: Record<string, number> = { all_yes: 0, majority_yes: 0, divergent: 0, majority_no: 0, all_no: 0, unrated: 0 };
    for (const n of novels.value) {
        const c = n.ai_reviews?.consensus;
        if (c && c in dist) dist[c]++;
        else dist.unrated++;
    }
    return dist;
});

const risingNovels = ref<RisingNovelRow[]>([]);
const riskTags = ref<RiskTagRow[]>([]);

const consensusLabelMap: Record<string, string> = Object.fromEntries(
    CONSENSUS_OPTIONS.map(opt => [opt.value, opt.label]),
) as Record<string, string>;

function consensusLabel(value: string | null | undefined): string {
    if (!value) return '—';
    return consensusLabelMap[value] ?? value;
}

function normalizeTextList(value: string[] | string | null | undefined): string[] {
    if (!value) return [];
    if (Array.isArray(value)) {
        return value.map(item => item.trim()).filter(Boolean);
    }
    const text = value.trim();
    return text ? [text] : [];
}

function countBreakdownField(field: keyof AiReviewsBreakdown, limit: number): [string, number][] {
    const map = new Map<string, number>();
    for (const novel of novels.value) {
        const raw = novel.ai_reviews?.breakdown?.[field] as string[] | string | null | undefined;
        for (const item of normalizeTextList(raw)) {
            map.set(item, (map.get(item) ?? 0) + 1);
        }
    }
    return Array.from(map.entries())
        .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0], 'zh-Hans-CN'))
        .slice(0, limit);
}

const topGoldfingerTypes = computed(() => countBreakdownField('goldfinger_type', 8));
const topProtagonistArchetypes = computed(() => countBreakdownField('protagonist_archetype', 6));

const selectedBreakdown = computed(() => selectedNovel.value?.ai_reviews?.breakdown ?? null);
const selectedChapterEndHookTypes = computed(() => normalizeTextList(selectedBreakdown.value?.chapter_end_hook_types));
const selectedLearningPoints = computed(() => normalizeTextList(selectedBreakdown.value?.learning_points));
const selectedBreakdownOverviewReady = computed(() => {
    const breakdown = selectedBreakdown.value;
    if (!breakdown) return false;
    return Boolean(
        breakdown.goldfinger_type ||
        breakdown.protagonist_archetype ||
        breakdown.opening_hook ||
        selectedChapterEndHookTypes.value.length > 0
    );
});
const selectedBreakdownPacingReady = computed(() => {
    const breakdown = selectedBreakdown.value;
    if (!breakdown) return false;
    return Boolean(breakdown.hook_density || breakdown.pacing_notes);
});
const selectedBreakdownLearningReady = computed(() => {
    if (!selectedBreakdown.value) return false;
    return selectedLearningPoints.value.length > 0;
});
const selectedHookDensityBadge = computed(() => hookDensityBadge(selectedBreakdown.value?.hook_density ?? null));

function hookDensityBadge(value: string | null | undefined): { label: string; cls: string } | null {
    switch (value) {
        case 'high':
            return { label: '高密度', cls: 'bg-emerald-500/15 text-emerald-300 border-emerald-500/30' };
        case 'medium':
            return { label: '适中', cls: 'bg-sky-500/15 text-sky-300 border-sky-500/30' };
        case 'low':
            return { label: '较稀疏', cls: 'bg-amber-500/15 text-amber-300 border-amber-500/30' };
        default:
            return null;
    }
}

async function loadNovels() {
    novelsLoading.value = true;
    try {
        const filter: NovelListFilter = {
            tags: tagFilter.value,
            consensus: consensusFilter.value,
            platform: null,
            sort_by: sortBy.value,
        };
        novels.value = await invoke<NovelListRow[]>('list_novels', { filter });
    } catch (e) {
        console.error('list_novels 失败:', e);
        novels.value = [];
    } finally {
        novelsLoading.value = false;
    }
}

async function loadReportInsights() {
    try {
        const [rising, risks] = await Promise.all([
            invoke<RisingNovelRow[]>('list_rising_novels', { limit: 5 }),
            invoke<RiskTagRow[]>('list_risk_tags', { minCount: 2, limit: 8 }),
        ]);
        risingNovels.value = rising;
        riskTags.value = risks;
    } catch (e) {
        console.error('加载报表洞察失败:', e);
        risingNovels.value = [];
        riskTags.value = [];
    }
}

function toggleTag(t: string) {
    const i = tagFilter.value.indexOf(t);
    if (i >= 0) tagFilter.value.splice(i, 1);
    else tagFilter.value.push(t);
    loadNovels();
}

function toggleConsensus(c: ConsensusKey) {
    const i = consensusFilter.value.indexOf(c);
    if (i >= 0) consensusFilter.value.splice(i, 1);
    else consensusFilter.value.push(c);
    loadNovels();
}

function selectNovel(novel: NovelListRow) {
    selectedNovel.value = novel;
}

function setActiveTab(tab: 'library' | 'reports') {
    activeTab.value = tab;
    selectedNovel.value = null;
    if (tab === 'reports') {
        loadReportFiles();
    }
}

async function evaluateSelectedNovel() {
    if (!selectedNovel.value) return;
    if (!aiConfig.value.apiKey) {
        showSettings.value = true;
        alert("请先配置 AI API Key");
        return;
    }

    isEvaluatingNovel.value = true;
    try {
        await invoke('update_ai_config', {
            apiBase: aiConfig.value.apiBase,
            apiKey: aiConfig.value.apiKey,
            model: aiConfig.value.model
        });
        downloadLog.value.push(`[${new Date().toLocaleTimeString()}] 正在重新评估《${selectedNovel.value.title}》...`);
        await invoke<string>('evaluate_novel', { novelId: selectedNovel.value.id });
        await loadNovels();

        const refreshed = novels.value.find(n => n.id === selectedNovel.value?.id);
        if (refreshed) {
            selectedNovel.value = refreshed;
        }
        downloadLog.value.push(`[${new Date().toLocaleTimeString()}] 《${selectedNovel.value.title}》评估完成。`);
    } catch (e) {
        console.error("Failed to evaluate novel:", e);
        alert(`评估失败: ${e}`);
        downloadLog.value.push(`[${new Date().toLocaleTimeString()}] 评估失败: ${e}`);
    } finally {
        isEvaluatingNovel.value = false;
    }
}

// --- AI Settings ---
const showSettings = ref(false);
const aiConfig = ref({
    apiBase: localStorage.getItem('ai_api_base') || 'https://api.openai.com/v1',
    apiKey: localStorage.getItem('ai_api_key') || '',
    model: localStorage.getItem('ai_model') || 'gpt-3.5-turbo',
    promptChapter: localStorage.getItem('ai_prompt_chapter') || '', // 拆单章
    promptSummary: localStorage.getItem('ai_prompt_summary') || '', // 总结前几章
    analysisChapters: parseInt(localStorage.getItem('ai_analysis_chapters') || '5'), // AI 分析读取章数
    spiderVisible: localStorage.getItem('spider_visible') === 'true' // 控制蜘蛛窗口可见，用于调试 WAF
});
const availableModels = ref<string[]>([]);
const isFetchingModels = ref(false);

async function saveSettings() {
    localStorage.setItem('ai_api_base', aiConfig.value.apiBase);
    localStorage.setItem('ai_api_key', aiConfig.value.apiKey);
    localStorage.setItem('ai_model', aiConfig.value.model);
    localStorage.setItem('ai_prompt_chapter', aiConfig.value.promptChapter);
    localStorage.setItem('ai_prompt_summary', aiConfig.value.promptSummary);
    localStorage.setItem('ai_analysis_chapters', String(aiConfig.value.analysisChapters));
    localStorage.setItem('spider_visible', String(aiConfig.value.spiderVisible));
    
    // 同步到后端的 workflow_config.json 供全量扫榜和定时任务使用
    try {
        await invoke('update_ai_config', {
            apiBase: aiConfig.value.apiBase,
            apiKey: aiConfig.value.apiKey,
            model: aiConfig.value.model
        });
        console.log("Synced AI config to backend workflow_config.json");
    } catch (e) {
        console.error("Failed to sync AI config to backend:", e);
    }
    
    showSettings.value = false;
}

function formatRankOption(url: string): { value: string; label: string } {
    const platform = url.includes('fanqie') ? 'fanqie' : 'qidian';
    const normalized = url.replace(/^https?:\/\//, '').replace(/\/$/, '');
    const tail = normalized.split('/').slice(-2).join(' / ');
    const prefix = platform === 'fanqie' ? '番茄' : '起点';
    return {
        value: `${platform}:${url}`,
        label: `${prefix} · ${tail || normalized}`
    };
}

async function loadWorkflowConfig() {
    try {
        const config = await invoke<WorkflowConfigPayload>('get_workflow_config');
        workflowConfig.value = config;
        rankOptions.value = config.rank_urls.map(formatRankOption);
    } catch (e) {
        console.error("Failed to load workflow config:", e);
        workflowConfig.value = null;
        rankOptions.value = [];
    }
}

// --- Workspace Directory Selection ---
async function selectWorkspaceDir() {
    try {
        const selected = await open({
            directory: true,
            multiple: false,
            title: "选择工作目录"
        });

        if (selected && typeof selected === 'string') {
            workspaceRoot.value = selected;
            localStorage.setItem('workspace_root', selected);

            // Ensure subdirectories exist
            await invoke("ensure_workspace_dirs", { workspaceRoot: selected });

            // 同步工作目录到后端（系统托盘/调度器使用同一路径）
            await invoke("set_workspace_root", { root: selected });
        }
    } catch (e) {
        alert("选择目录失败: " + e);
    }
}

// --- Log Viewer ---
const showLogs = ref(false);
const logContent = ref("");

async function openLogs() {
    showLogs.value = true;
    await fetchLogs();
}

async function fetchLogs() {
    try {
        logContent.value = await invoke("read_log_file", {
            workspaceRoot: workspaceRoot.value || null
        });
        // Scroll to bottom (optional, but naive impl here)
        nextTick(() => {
            const el = document.getElementById("log-textarea");
            if (el) el.scrollTop = el.scrollHeight;
        });
    } catch (e) {
        logContent.value = "读取日志失败: " + e;
    }
}

// --- Theme ---
const themes = ['dark', 'light'];
const currentThemeIndex = ref(0);

function cycleTheme() {
    currentThemeIndex.value = (currentThemeIndex.value + 1) % themes.length;
    const theme = themes[currentThemeIndex.value];
    if (theme === 'dark') {
        document.documentElement.removeAttribute('data-theme');
    } else {
        document.documentElement.setAttribute('data-theme', theme);
    }
}

// --- Events ---

// --- Logic ---

onMounted(async () => {
    // 同步工作目录到后端（确保托盘/调度器使用同一路径）
    const savedRoot = localStorage.getItem('workspace_root');
    if (savedRoot) {
        try { await invoke("set_workspace_root", { root: savedRoot }); } catch (e) { /* ignore */ }
    }

    listen('download-progress', (event: any) => {
        const payload = event.payload;
        downloadLog.value.push(`[${new Date().toLocaleTimeString()}] ${payload.message}`);

        if (payload.status === 'completed') {
             isDownloading.value = false;
             // 移除自动分析，改为手动触发
         } else if (payload.status === 'error') {
            // Error handling
            // We might want to stop on critical error?
            // isDownloading.value = false;
        }
    });

    listen("report-generated", () => {
        isDownloading.value = false;
        currentPhase.value = null;
        logContent.value += `[${new Date().toLocaleTimeString()}] 扫榜完成，报告已生成。\n`;
        loadReportFiles();
        loadReportInsights();
    });

    listen<ScanRunStatus>("scan-run-status", (event) => {
        const payload = event.payload;
        isDownloading.value = false;
        currentPhase.value = null;
        const line = `[${new Date().toLocaleTimeString()}] ${payload.message}`;
        downloadLog.value.push(line);
        logContent.value += `${line}\n`;
        if (payload.status === 'completed') {
            loadReportFiles();
            loadNovels();
            loadReportInsights();
        }
    });

    listen<PipelineProgress>("pipeline-progress", (event) => {
        const payload = event.payload;
        currentPhase.value = payload;
        const progressTag = payload.progress
            ? ` (${payload.progress[0]}/${payload.progress[1]})`
            : '';
        downloadLog.value.push(
            `[${new Date().toLocaleTimeString()}] [Phase ${payload.phase}/${4} · ${payload.status}]${progressTag} ${payload.message}`
        );
        if (payload.phase === 2 && payload.status === 'completed') {
            loadNovels();
            loadReportInsights();
        }
        if (payload.phase === 4 && payload.status === 'completed') {
            loadNovels();
            loadReportInsights();
        }
    });

    // Strategy 2: Periodic refresh while downloading (every 2s) to catch new folders
    loadReportFiles();
    loadReportInsights();
    loadWorkflowConfig();
    loadNovels();
});

async function submitAddBook() {
    if (isDownloading.value) return;
    if (!newBookUrl.value.trim()) {
        alert("请输入小说主页 URL");
        return;
    }
    if (!workspaceRoot.value) {
        alert("请先选择工作目录");
        return;
    }

    isDownloading.value = true;
    downloadLog.value = [];
    showAddBookModal.value = false;

    try {
        await invoke("trigger_full_scan", {
            targetUrl: newBookUrl.value.trim(),
            platform: newBookPlatform.value,
        });
    } catch (e) {
        alert("Error: " + e);
        isDownloading.value = false;
    }
}

async function fetchModels() {
    if (!aiConfig.value.apiBase || !aiConfig.value.apiKey) {
        alert("请先填写 Base URL 和 API Key");
        return;
    }
    
    isFetchingModels.value = true;
    try {
        const models = await invoke("fetch_ai_models", {
            apiBase: aiConfig.value.apiBase,
            apiKey: aiConfig.value.apiKey,
        });
        availableModels.value = models as string[];
        if (availableModels.value.length > 0 && !aiConfig.value.model) {
            aiConfig.value.model = availableModels.value[0];
        }
    } catch (e) {
        alert("获取模型列表失败: " + e);
        availableModels.value = [];
    } finally {
        isFetchingModels.value = false;
    }
}

async function clearLogs() {
    console.log("clearLogs function called!");

    try {
        console.log("Calling invoke clear_log...");
        const result = await invoke("clear_log", {
            workspaceRoot: workspaceRoot.value || null
        });
        console.log("Clear logs result:", result);
        logContent.value = "";
        alert(result);
    } catch (e) {
        console.error("Clear logs error:", e);
        alert(`清空失败: ${e}`);
    }
}

// --- Reports ---
async function loadReportFiles() {
    if (!workspaceRoot.value) {
        reportFiles.value = [];
        return;
    }
    try {
        reportFiles.value = await invoke("list_reports", {
            workspaceRoot: workspaceRoot.value
        }) as string[];
    } catch (e) {
        console.error("Failed to load reports:", e);
        reportFiles.value = [];
    }
}
const selectedRank = ref('');

async function triggerFullScan() {
    if (!workspaceRoot.value) {
        alert("请先选择工作目录");
        return;
    }
    isDownloading.value = true;
    logContent.value += `[${new Date().toLocaleTimeString()}] 已触发后台全量扫榜任务...\n`;
    setActiveTab('reports');
    
    let targetUrl = null;
    let platform = null;
    if (selectedRank.value) {
        const parts = selectedRank.value.split(":");
        platform = parts[0];
        targetUrl = parts.slice(1).join(":");
    }

    try {
        await invoke('trigger_full_scan', { targetUrl, platform });
    } catch (e) {
        logContent.value += `[${new Date().toLocaleTimeString()}] 触发失败: ${e}\n`;
        isDownloading.value = false;
    }
}

async function selectReport(filename: string) {
    selectedReport.value = filename;
    try {
        reportContent.value = await invoke("read_report", {
            workspaceRoot: workspaceRoot.value,
            filename: filename
        }) as string;
    } catch (e) {
        reportContent.value = "读取报告失败: " + e;
    }
}

function formatReportName(filename: string): string {
    if (filename.startsWith('manual_report_')) {
        const part = filename.replace('manual_report_', '').replace('.md', '');
        const segments = part.split('_');
        if (segments.length >= 2 && segments[0].length === 8) {
            const d = segments[0];
            const t = segments[1];
            return `手动扫榜 ${d.slice(0,4)}-${d.slice(4,6)}-${d.slice(6,8)} ${t.slice(0,2)}:${t.slice(2,4)}`;
        }
        return `手动扫榜 ${part}`;
    }
    if (filename.startsWith('report_')) {
        return `定时扫榜 ${filename.replace('report_', '').replace('.md', '')}`;
    }
    return filename.replace('.md', '');
}

</script>

<template>
  <div class="h-screen flex flex-col text-txt bg-bg font-sans overflow-hidden">
    <header class="h-16 bg-sidebar border-b border-border-dim flex items-center gap-4 px-4 flex-shrink-0">
        <div class="flex items-center gap-3 min-w-0">
            <div class="w-8 h-8 rounded-xl bg-gradient-to-br from-red-500 to-orange-600 flex items-center justify-center shadow-lg shadow-red-900/20 flex-shrink-0">
                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="w-5 h-5 text-white">
                    <path d="M11.7 2.805a.75.75 0 0 1 .6 0A16.036 16.036 0 0 0 17.686 5.5c1.657.492 2.766 1.763 3.394 3.033.453.916.71 2.016.92 3.149.23 1.238.23 2.536.002 3.791-.191 1.054-.537 2.062-1.025 2.923-.526.927-1.398 1.956-2.825 2.668a1.503 1.503 0 0 1-1.304 0c-1.427-.712-2.299-1.741-2.825-2.668-.488-.861-.834-1.869-1.025-2.923-.228-1.255-.228-2.553.002-3.791.21-1.133.467-2.233.92-3.149.628-1.27 1.737-2.54 3.394-3.033a16.033 16.033 0 0 0 5.386 2.695.75.75 0 0 1 .458 1.06l-.99.99-.028.028a3.155 3.155 0 0 0-.67 1.264c-.055.234-.265.378-.49.337a.66.66 0 0 1-.502-.45c-.179-.646-.502-1.246-.938-1.758a.74.74 0 0 1 .09-1.03l.97-.88a14.522 14.522 0 0 1-4.225-2.004Z" />
                </svg>
            </div>
            <div class="min-w-0">
                <h1 class="font-bold text-base text-txt tracking-tight leading-tight">拆书工具 <span class="text-accent text-xs align-top opacity-80 font-normal">Pro</span></h1>
                <p class="text-[10px] text-txt-dim font-medium tracking-wide uppercase">Novel Splitter</p>
            </div>
        </div>

        <div class="flex-1 flex justify-center">
            <div class="bg-subtle p-1 rounded-lg flex relative border border-border-dim min-w-[240px] max-w-md w-full">
                <div class="absolute top-1 bottom-1 rounded-md bg-active border border-border-dim shadow-sm transition-all duration-300 ease-out" :style="{ left: activeTab === 'library' ? '4px' : 'calc(50% + 1px)', width: 'calc(50% - 5px)' }"></div>
                <button @click="setActiveTab('library')" class="flex-1 relative z-10 text-xs font-medium py-1.5 text-center transition-colors duration-200" :class="activeTab === 'library' ? 'text-txt' : 'text-txt-dim hover:text-txt'">📚 书库</button>
                <button @click="setActiveTab('reports')" class="flex-1 relative z-10 text-xs font-medium py-1.5 text-center transition-colors duration-200" :class="activeTab === 'reports' ? 'text-txt' : 'text-txt-dim hover:text-txt'">📊 拆书雷达</button>
            </div>
        </div>

        <div class="flex items-center gap-1 flex-shrink-0">
            <button @click="showSettings = true" class="w-7 h-7 flex items-center justify-center rounded-md hover:bg-subtle text-txt-dim hover:text-txt transition-all" title="API 设置">
                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-4 h-4">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 0 1 0 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 0 1 0-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281Z" />
                    <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z" />
                </svg>
            </button>
            <button @click="cycleTheme" class="w-7 h-7 flex items-center justify-center rounded-md hover:bg-subtle text-txt-dim hover:text-txt transition-all" title="切换皮肤">
                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-4 h-4">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9.53 16.122a3 3 0 0 0-5.78 1.128 2.25 2.25 0 0 1-2.4 2.245 4.5 4.5 0 0 0 8.4-2.245c0-.399-.078-.78-.22-1.128Zm0 0a15.998 15.998 0 0 0 3.388-1.62m-5.043-.025a15.994 15.994 0 0 1 1.622-3.395m3.42 3.418a15.995 15.995 0 0 0 4.764-4.648l3.876-5.814a1.151 1.151 0 0 0-1.597-1.597L14.146 6.32a16.03 16.03 0 0 0-4.649 4.763m0 0a2.18 2.18 0 0 0-1.655.895" />
                </svg>
            </button>
        </div>
    </header>

    <!-- Main Content -->
    <div class="flex-1 flex flex-col p-4 gap-4 overflow-hidden">

        <!-- ===== 报告全屏视图 ===== -->
        <div v-if="selectedReport && activeTab === 'reports'" class="flex-1 bg-card rounded-lg border border-border flex overflow-hidden">
            <!-- 左侧目录 -->
            <div v-if="reportToc.length > 0" class="w-56 flex-shrink-0 border-r border-border flex flex-col">
                <div class="bg-white/5 px-3 py-2 border-b border-border text-xs font-bold text-txt-dim uppercase tracking-wider">📑 目录</div>
                <div class="flex-1 overflow-y-auto p-2 space-y-0.5">
                    <div v-for="item in reportToc" :key="item.id" @click="scrollToHeading(item.id)"
                        class="cursor-pointer text-[11px] py-1 px-2 rounded hover:bg-white/5 text-txt-dim hover:text-txt transition-colors truncate"
                        :style="{ paddingLeft: (item.level - 1) * 12 + 8 + 'px' }"
                    >{{ item.text }}</div>
                </div>
            </div>
            <!-- 右侧报告内容 -->
            <div class="flex-1 flex flex-col overflow-hidden">
                <div class="bg-white/5 px-4 py-2 border-b border-border flex justify-between items-center text-sm font-bold">
                    <span>📊 {{ formatReportName(selectedReport) }}</span>
                    <button @click="selectedReport = null; reportContent = ''" class="text-txt-dim hover:text-txt text-xs">✕ 关闭</button>
                </div>
                <div class="flex-1 p-6 overflow-y-auto prose prose-invert prose-sm max-w-none" v-html="marked(reportContent || '')"></div>
            </div>
        </div>
        <!-- ===== 书库/分析 分栏视图 ===== -->
        <template v-if="!(selectedReport && activeTab === 'reports')">
        <!-- 书库无选中全宽 -->
        <div v-if="activeTab === 'library' && !selectedNovel" class="flex flex-col gap-4 pb-6 overflow-y-auto">
            <div class="flex items-center justify-between gap-4 flex-shrink-0">
                <button @click="selectWorkspaceDir" class="flex items-center gap-2 bg-subtle border border-border-dim rounded-lg px-3 py-2 hover:bg-hover transition-colors group min-w-0">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-4 h-4 text-accent flex-shrink-0"><path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 0 1 4.5 9.75h15A2.25 2.25 0 0 1 21.75 12v.75m-8.69-6.44-2.12-2.12a1.5 1.5 0 0 0-1.061-.44H4.5A2.25 2.25 0 0 0 2.25 6v12a2.25 2.25 0 0 0 2.25 2.25h15A2.25 2.25 0 0 0 21.75 18V9a2.25 2.25 0 0 0-2.25-2.25h-5.379a1.5 1.5 0 0 1-1.06-.44Z" /></svg>
                    <span v-if="workspaceRoot" class="text-xs text-txt truncate max-w-56" :title="workspaceRoot">{{ workspaceRoot.split('/').pop() || workspaceRoot }}</span>
                    <span v-else class="text-xs text-txt-dim italic">选择工作目录...</span>
                </button>
                <div class="flex items-center gap-1">
                    <button @click="showAddBookModal = true" :disabled="!workspaceRoot || isDownloading" class="text-accent hover:text-orange-300 transition-colors p-1.5 rounded hover:bg-subtle disabled:opacity-30" title="添加书籍">➕</button>
                    <button @click="loadNovels" class="text-txt-dim hover:text-txt transition-colors p-1.5 rounded hover:bg-subtle" title="刷新列表">🔄</button>
                </div>
            </div>
            <div class="flex flex-wrap items-center gap-2 flex-shrink-0">
                <div class="flex flex-wrap gap-1">
                    <button v-for="opt in CONSENSUS_OPTIONS" :key="opt.value" @click="toggleConsensus(opt.value!)" class="text-xs px-2 py-0.5 rounded border transition-all" :class="consensusFilter.includes(opt.value!) ? 'bg-accent/20 border-accent text-accent' : 'border-border-dim text-txt-dim hover:text-txt'">{{ opt.label }}</button>
                </div>
                <select v-model="sortBy" @change="loadNovels" class="bg-input border border-border-dim rounded px-2 py-1 text-xs outline-none focus:border-accent">
                    <option value="updated_desc">更新</option>
                    <option value="latest_rank_asc">排名</option>
                    <option value="scan_count_desc">上榜</option>
                </select>
            </div>
            <div v-if="allTags.length > 0" class="flex flex-wrap gap-1 flex-shrink-0">
                <button v-for="t in allTags.slice(0, 30)" :key="t" @click="toggleTag(t)" class="text-xs px-1.5 py-0.5 rounded border transition-all" :class="tagFilter.includes(t) ? 'bg-accent/20 border-accent text-accent' : 'border-border-dim text-txt-dim hover:text-txt'">{{ t }}</button>
            </div>
            <NovelGrid :novels="novels" :novels-loading="novelsLoading" :selected-novel-id="selectedNovelId" @select="selectNovel" />
        </div>
        <!-- 报告无选中 -->
        <div v-else-if="activeTab === 'reports' && !selectedReport" class="flex flex-col gap-4 overflow-y-auto pb-6">
            <div class="bg-subtle/30 border border-border-dim rounded-lg p-4 flex-shrink-0">
                <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
                    <div class="bg-card/70 rounded-lg border border-border-dim p-3">
                        <div class="text-xs text-txt-dim mb-2 font-medium">热门题材</div>
                        <div class="flex flex-wrap gap-1">
                            <span v-for="[tag, w] in topTags.slice(0, 10)" :key="tag" class="px-2 py-0.5 rounded text-xs border transition-all" :class="topTags[0]?.[0] === tag ? 'bg-accent/10 border-accent/30 text-accent' : 'border-border-dim text-txt-dim'">{{ tag }} <span class="font-mono">{{ w > 0 ? '+' : '' }}{{ w }}</span></span>
                        </div>
                    </div>
                    <div class="bg-card/70 rounded-lg border border-border-dim p-3">
                        <div class="text-xs text-txt-dim mb-2 font-medium">关注点</div>
                        <div class="flex flex-wrap gap-1">
                            <span v-for="[kw, cnt] in topFocus.slice(0, 8)" :key="kw" class="px-2 py-0.5 rounded text-xs border border-border-dim text-txt-dim">{{ kw }} <span class="font-mono opacity-50">{{ cnt }}</span></span>
                        </div>
                    </div>
                    <div class="bg-card/70 rounded-lg border border-border-dim p-3">
                        <div class="text-xs text-txt-dim mb-2 font-medium">高频金手指</div>
                        <div v-if="topGoldfingerTypes.length > 0" class="flex flex-wrap gap-1">
                            <span v-for="[item, cnt] in topGoldfingerTypes" :key="item" class="px-2 py-0.5 rounded text-xs border border-border-dim text-txt-dim">{{ item }} <span class="font-mono opacity-50">{{ cnt }}</span></span>
                        </div>
                        <div v-else class="text-xs text-txt-dim">等待评估积累</div>
                    </div>
                    <div class="bg-card/70 rounded-lg border border-border-dim p-3">
                        <div class="text-xs text-txt-dim mb-2 font-medium">主角人设</div>
                        <div v-if="topProtagonistArchetypes.length > 0" class="flex flex-wrap gap-1">
                            <span v-for="[item, cnt] in topProtagonistArchetypes" :key="item" class="px-2 py-0.5 rounded text-xs border border-border-dim text-txt-dim">{{ item }} <span class="font-mono opacity-50">{{ cnt }}</span></span>
                        </div>
                        <div v-else class="text-xs text-txt-dim">等待评估积累</div>
                    </div>
                    <div class="bg-card/70 rounded-lg border border-border-dim p-3">
                        <div class="text-xs text-txt-dim mb-2 font-medium">共识</div>
                        <div class="flex flex-wrap gap-1">
                            <span v-for="[c, label, cls] in [['all_yes','看好','bg-green-500/10 text-green-300 border-green-500/30'],['majority_yes','多看好','bg-emerald-500/10 text-emerald-300 border-emerald-500/30'],['divergent','分歧','bg-amber-500/10 text-amber-300 border-amber-500/30'],['majority_no','多不看好','bg-rose-500/10 text-rose-300 border-rose-500/30'],['all_no','不看好','bg-red-500/10 text-red-300 border-red-500/30']] as const" :key="c" class="px-2 py-0.5 rounded text-xs border" :class="cls">{{ label }} {{ consensusDistribution[c] }}</span>
                        </div>
                    </div>
                    <div class="bg-card/70 rounded-lg border border-border-dim p-3">
                        <div class="text-xs text-txt-dim mb-2 font-medium">黑马</div>
                        <div v-if="risingNovels.length > 0" class="space-y-1.5">
                            <div v-for="item in risingNovels" :key="item.id" class="flex items-start justify-between gap-2 text-xs">
                                <div class="min-w-0">
                                    <div class="text-txt truncate">{{ item.title }}</div>
                                    <div class="text-[11px] text-txt-dim">{{ item.platform }} · #{{ item.rank }}</div>
                                </div>
                                <div class="text-accent font-mono whitespace-nowrap">
                                    {{ item.rank_change.startsWith('-') ? item.rank_change : `+${item.rank_change.replace(/^\+/, '')}` }}
                                </div>
                            </div>
                        </div>
                        <div v-else class="text-xs text-txt-dim">暂无最新黑马</div>
                    </div>
                    <div class="bg-card/70 rounded-lg border border-border-dim p-3">
                        <div class="text-xs text-txt-dim mb-2 font-medium">风险赛道</div>
                        <div v-if="riskTags.length > 0" class="space-y-1.5">
                            <div v-for="item in riskTags" :key="item.tag" class="flex items-start justify-between gap-2 text-xs">
                                <div class="min-w-0">
                                    <div class="text-txt truncate">{{ item.tag }}</div>
                                    <div class="text-[11px] text-txt-dim">样本 {{ item.sample_count }} 个</div>
                                </div>
                                <div class="text-rose-300 font-mono whitespace-nowrap">{{ item.avg_weight.toFixed(1) }}</div>
                            </div>
                        </div>
                        <div v-else class="text-xs text-txt-dim">暂无负向标签</div>
                    </div>
                </div>
            </div>
            <select v-model="selectedRank" class="w-full bg-subtle border border-border-dim text-txt text-xs rounded-lg px-4 py-2.5 outline-none focus:border-accent cursor-pointer">
                <option value="">（按 workflow_config.json 批量扫榜）</option>
                <option v-for="opt in rankOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
            </select>
            <div class="bg-subtle/30 border border-border-dim rounded-lg p-3 text-xs text-txt-dim">
                <div>监控榜单 {{ workflowConfig?.rank_urls.length ?? 0 }} 个</div>
                <div v-if="workflowConfig">定时任务 {{ workflowConfig.enabled ? `已开启 ${workflowConfig.schedule_time}` : '未开启' }}</div>
            </div>
            <button @click="triggerFullScan" :disabled="isDownloading" class="w-full bg-gradient-to-r from-accent to-orange-500 text-[var(--accent-text)] font-bold py-2.5 px-4 rounded-lg hover:opacity-90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 text-xs shadow-sm"><span v-if="isDownloading" class="animate-spin">⏳</span><span>{{ isDownloading ? '后台扫榜中...' : '🚀 立即扫榜' }}</span></button>
            <div class="flex-1 bg-subtle rounded-xl border border-border-dim overflow-y-auto p-2 space-y-1"><div v-if="reportFiles.length === 0" class="flex items-center justify-center h-32 text-txt-dim text-xs">暂无报告</div><template v-for="file in reportFiles" :key="file"><div @click="selectReport(file)" class="px-3 py-2.5 rounded-lg cursor-pointer transition-all border flex items-center gap-3" :class="selectedReport === file ? 'bg-gradient-to-r from-accent/10 to-transparent border-l-2 border-l-accent' : 'hover:bg-hover border-l-2 border-l-transparent text-txt-dim hover:text-txt'"><span>📊</span><div class="flex-1 min-w-0"><div class="truncate font-medium text-xs">{{ formatReportName(file) }}</div><div class="text-[10px] opacity-40 truncate">{{ file }}</div></div></div></template></div>
        </div>
        <!-- 选中书籍时分栏 -->
        <div v-else class="flex-1 grid grid-cols-2 gap-4 min-h-0">
            <!-- Left: DB Detail View -->
            <div class="bg-card rounded-lg border border-border flex flex-col overflow-hidden">
                <div class="bg-white/5 px-4 py-2 border-b border-border flex justify-between items-center text-sm font-bold">
                    <div class="flex items-center gap-2 min-w-0">
                        <button
                            @click="selectedNovel = null"
                            class="text-xs text-accent hover:text-orange-300 transition-colors px-2 py-1 rounded-md border border-accent/20 bg-accent/5"
                        >
                            ← 返回书库
                        </button>
                        <span>📖 单书详情</span>
                    </div>
                    <span class="font-normal text-gray-500 text-xs">数据库详情</span>
                </div>
                
                <!-- V2 Novel Detail Panel (任务四a) -->
                <div v-if="selectedNovel" class="flex-1 p-6 overflow-y-auto">
                    <div class="text-center mb-6">
                        <div class="text-5xl mb-3">📚</div>
                        <h2 class="text-xl font-bold text-accent mb-1">{{ selectedNovel.title }}</h2>
                        <div class="text-xs text-txt-dim">
                            <span v-if="selectedNovel.word_count">{{ (selectedNovel.word_count / 10000).toFixed(1) }}w 字 · </span>
                            <span>{{ selectedNovel.platform }}</span>
                            <span v-if="selectedNovel.latest_rank !== null"> · 最新排名 #{{ selectedNovel.latest_rank }}</span>
                            <span> · 上榜 {{ selectedNovel.scan_count }} 次</span>
                        </div>
                    </div>

                    <div v-if="selectedNovel.tags.length > 0" class="flex flex-wrap gap-1.5 justify-center mb-5">
                        <span v-for="tag in selectedNovel.tags" :key="tag" class="px-2 py-0.5 bg-subtle border border-border-dim rounded text-[11px] text-txt-dim">{{ tag }}</span>
                    </div>

                    <div class="flex gap-2 mb-5">
                        <button @click="evaluateSelectedNovel" :disabled="isEvaluatingNovel || !aiConfig.apiKey" class="flex-1 py-2 text-xs rounded-lg border border-accent/30 text-accent hover:bg-accent/10 transition-all disabled:opacity-30 disabled:cursor-not-allowed flex items-center justify-center gap-1.5">
                            <span>{{ isEvaluatingNovel ? '⏳' : '🤖' }}</span> {{ isEvaluatingNovel ? '评估中…' : '重新评估' }}
                        </button>
                    </div>

                    <div v-if="selectedNovel.ai_reviews" class="space-y-4">
                        <div class="bg-subtle p-4 rounded-lg border border-border-dim">
                            <div class="text-xs text-accent mb-2 uppercase tracking-wider font-bold">🎯 拆书判断</div>
                            <div class="text-sm">{{ consensusLabel(selectedNovel.ai_reviews.consensus) }}</div>
                            <div v-if="selectedNovel.ai_reviews.meta?.input_chapters" class="text-[11px] text-txt-dim mt-2">
                                基于前 {{ selectedNovel.ai_reviews.meta.input_chapters }} 章样本
                            </div>
                        </div>

                        <div v-for="(agentKey, idx) in (['reader','editor','author'] as const)" :key="agentKey">
                            <div class="bg-subtle p-4 rounded-lg border border-border-dim">
                                <div class="flex justify-between items-center mb-2">
                                    <span class="text-xs text-accent uppercase tracking-wider font-bold">
                                        {{ ['📖 读者','✏️ 主编','✍️ 白金作者'][idx] }}
                                    </span>
                                    <span class="text-[11px] px-1.5 py-0.5 rounded bg-bg border border-border-dim">
                                        投票: {{ selectedNovel.ai_reviews.agents?.[agentKey]?.vote ?? '—' }}
                                    </span>
                                </div>
                                <div v-if="selectedNovel.ai_reviews.agents?.[agentKey]?.focus?.length" class="flex flex-wrap gap-1 mb-2">
                                    <span v-for="f in selectedNovel.ai_reviews.agents[agentKey]!.focus" :key="f" class="text-[10px] px-1.5 py-0.5 rounded bg-bg border border-border-dim text-txt-dim">{{ f }}</span>
                                </div>
                                <div v-if="selectedNovel.ai_reviews.agents?.[agentKey]?.comment" class="text-xs text-txt opacity-90 leading-relaxed">
                                    {{ selectedNovel.ai_reviews.agents[agentKey]!.comment }}
                                </div>
                                <div v-else class="text-xs text-txt-dim italic">无评论</div>
                            </div>
                        </div>
                    </div>
                    <div v-else class="bg-subtle p-4 rounded-lg border border-border-dim text-center">
                        <div class="text-xs text-txt-dim">🤖 该书尚未评估</div>
                        <div class="text-[10px] text-txt-dim opacity-60 mt-1">下次扫榜或手动重跑后会自动生成</div>
                    </div>
                    <div class="space-y-4 mt-4">
                        <div class="bg-subtle p-4 rounded-lg border border-border-dim">
                            <div class="text-xs text-accent mb-2 uppercase tracking-wider font-bold">📄 拆书提要</div>
                            <template v-if="selectedBreakdownOverviewReady && selectedBreakdown">
                                <div class="space-y-3">
                                    <div class="grid grid-cols-[88px_1fr] gap-2 text-sm">
                                        <span class="text-txt-dim">金手指</span>
                                        <span class="text-txt">{{ selectedBreakdown.goldfinger_type || '—' }}</span>
                                        <span class="text-txt-dim">主角人设</span>
                                        <span class="text-txt">{{ selectedBreakdown.protagonist_archetype || '—' }}</span>
                                        <span class="text-txt-dim">开篇钩子</span>
                                        <span class="text-txt leading-relaxed">{{ selectedBreakdown.opening_hook || '—' }}</span>
                                    </div>
                                    <div v-if="selectedChapterEndHookTypes.length > 0" class="flex flex-wrap gap-1.5">
                                        <span v-for="item in selectedChapterEndHookTypes" :key="item" class="px-2 py-0.5 rounded-full text-[11px] border border-border-dim bg-bg text-txt-dim">{{ item }}</span>
                                    </div>
                                </div>
                            </template>
                            <p v-else class="text-sm text-txt leading-relaxed">AI 评估扩展中，下次重新评估时会生成。</p>
                        </div>
                        <div class="bg-subtle p-4 rounded-lg border border-border-dim">
                            <div class="text-xs text-accent mb-2 uppercase tracking-wider font-bold">⏱️ 章节节奏</div>
                            <template v-if="selectedBreakdownPacingReady && selectedBreakdown">
                                <div class="space-y-3">
                                    <span
                                        v-if="selectedHookDensityBadge"
                                        class="inline-flex items-center px-2.5 py-1 rounded-full text-[11px] border"
                                        :class="selectedHookDensityBadge.cls"
                                    >
                                        {{ selectedHookDensityBadge.label }}
                                    </span>
                                    <p class="text-sm text-txt leading-relaxed">{{ selectedBreakdown.pacing_notes || '—' }}</p>
                                </div>
                            </template>
                            <p v-else class="text-sm text-txt leading-relaxed">AI 评估扩展中，下次重新评估时会生成。</p>
                        </div>
                        <div class="bg-subtle p-4 rounded-lg border border-border-dim">
                            <div class="text-xs text-accent mb-2 uppercase tracking-wider font-bold">✍️ 写法可借鉴点</div>
                            <template v-if="selectedBreakdownLearningReady">
                                <ul class="space-y-2 text-sm text-txt leading-relaxed list-disc list-inside">
                                    <li v-for="point in selectedLearningPoints" :key="point">{{ point }}</li>
                                </ul>
                            </template>
                            <p v-else class="text-sm text-txt leading-relaxed">AI 评估扩展中，下次重新评估时会生成。</p>
                        </div>
                    </div>
                </div>
            </div>

            <!-- Right: DB Summary -->
            <div class="bg-card rounded-lg border border-border flex flex-col overflow-hidden">
                <div class="bg-white/5 px-4 py-2 border-b border-border flex justify-between items-center text-sm font-bold">
                    <span>🤖 三视角评估摘要</span>
                    <span class="text-[11px] text-txt-dim">作者视角</span>
                </div>
                <div class="flex-1 p-4 overflow-y-auto">
                    <div v-if="selectedNovel" class="bg-subtle p-4 rounded-lg border border-border-dim h-full flex flex-col justify-between">
                        <div>
                            <div class="text-xs text-accent mb-2 uppercase tracking-wider font-bold">📘 本次评估概览</div>
                            <div class="grid grid-cols-[72px_1fr] gap-2 text-xs">
                                <span class="text-txt-dim">书名</span>
                                <span class="text-txt">{{ selectedNovel.title }}</span>
                                <span class="text-txt-dim">平台</span>
                                <span class="text-txt">{{ selectedNovel.platform }}</span>
                                <span class="text-txt-dim">最新排名</span>
                                <span class="text-txt">{{ selectedNovel.latest_rank !== null ? `#${selectedNovel.latest_rank}` : '—' }}</span>
                                <span class="text-txt-dim">上榜次数</span>
                                <span class="text-txt">{{ selectedNovel.scan_count }} 次</span>
                            </div>
                            <div v-if="selectedNovel.ai_reviews" class="mt-4 space-y-2 text-xs">
                                <div class="grid grid-cols-[72px_1fr] gap-2">
                                    <span class="text-txt-dim">评估模型</span>
                                    <span class="text-txt">{{ selectedNovel.ai_reviews.meta?.model ?? '—' }}</span>
                                </div>
                                <div class="grid grid-cols-[72px_1fr] gap-2">
                                    <span class="text-txt-dim">样本章节</span>
                                    <span class="text-txt">{{ selectedNovel.ai_reviews.meta?.input_chapters ?? '—' }} 章</span>
                                </div>
                                <div class="grid grid-cols-[72px_1fr] gap-2">
                                    <span class="text-txt-dim">综合判断</span>
                                    <span class="text-txt">{{ consensusLabel(selectedNovel.ai_reviews.consensus) }}</span>
                                </div>
                            </div>
                        </div>
                        <div class="text-[11px] text-txt-dim leading-relaxed">
                            章节节奏、黄金三章拆解和可借鉴写法会在后续版本补充。
                        </div>
                    </div>
                    <div v-else class="bg-subtle p-4 rounded-lg border border-border-dim text-center text-xs text-txt-dim">
                        选中一本书后，这里会显示作者向摘要
                    </div>
                </div>
            </div>
        </div>
        </template>
        <!-- Log / Progress Footer -->
        <div class="h-8 bg-sidebar rounded flex items-center px-4 gap-4 text-xs text-gray-400 flex-shrink-0">
             <button @click="openLogs" class="bg-black/20 hover:bg-black/40 px-2 py-0.5 rounded text-accent transition-colors flex items-center gap-1">
                <span>📜</span> 查看日志
            </button>
            <span class="font-bold whitespace-nowrap" :class="isDownloading ? 'text-success' : ''">
                <template v-if="currentPhase">
                    Phase {{ currentPhase.phase }}/4 · {{ currentPhase.message }}
                </template>
                <template v-else-if="isDownloading">
                    正在扫榜...
                </template>
                <template v-else>
                    就绪
                </template>
            </span>
            <span v-if="downloadLog.length > 0" class="text-accent truncate flex-1">
                {{ downloadLog[downloadLog.length - 1] }}
            </span>
        </div>

    </div>

  </div>

  <!-- Add Book Modal -->
  <div v-if="showAddBookModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 backdrop-blur-sm" @click.self="showAddBookModal = false">
      <div class="bg-card border border-border p-6 rounded-xl w-96 shadow-2xl">
          <h3 class="text-lg font-bold mb-4 flex items-center gap-2">
              📥 添加书籍
          </h3>
          <p class="text-[11px] text-txt-dim mb-4 leading-relaxed">
              输入小说主页 URL，系统会走完整 V2.0 流水线（抓取 → AI 提纯细纲 → 多 Agent 评估）。
          </p>

          <div class="space-y-4">
              <div class="flex flex-col gap-1">
                  <label class="text-xs text-gray-500">平台</label>
                  <select v-model="newBookPlatform" class="bg-input border border-border rounded px-3 py-2 text-sm outline-none focus:border-accent">
                      <option value="qidian">📖 起点中文网</option>
                      <option value="fanqie">🍅 番茄小说（开发中）</option>
                  </select>
              </div>

              <div class="flex flex-col gap-1">
                  <label class="text-xs text-gray-500">小说主页 URL</label>
                  <input
                      v-model="newBookUrl"
                      type="text"
                      placeholder="https://www.qidian.com/book/..."
                      class="bg-input border border-border rounded px-3 py-2 text-sm outline-none focus:border-accent"
                      @keyup.enter="submitAddBook"
                  >
                  <p class="text-[10px] text-gray-500">需为单本小说主页（含 /book/ 或 /info/ 路径）</p>
              </div>
          </div>

          <div class="flex gap-2 mt-6">
              <button
                  @click="showAddBookModal = false"
                  class="flex-1 py-2 text-xs rounded-lg border border-border-dim text-txt-dim hover:text-txt hover:border-border transition-all"
              >
                  取消
              </button>
              <button
                  @click="submitAddBook"
                  :disabled="isDownloading || !newBookUrl.trim()"
                  class="flex-1 py-2 text-xs rounded-lg bg-accent text-[var(--accent-text)] font-bold hover:opacity-90 transition-all disabled:opacity-40 disabled:cursor-not-allowed"
              >
                  {{ isDownloading ? '运行中…' : '🚀 开始拆解' }}
              </button>
          </div>
      </div>
  </div>

  <!-- Settings Modal -->
  <div v-if="showSettings" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 backdrop-blur-sm">
      <div class="bg-card border border-border p-6 rounded-xl w-96 shadow-2xl">
          <h3 class="text-lg font-bold mb-4 flex items-center gap-2">
              ⚙️ AI 配置
          </h3>
          
          <div class="space-y-4">
              <div class="flex flex-col gap-1">
                  <label class="text-xs text-gray-500">API 接口地址 (Base URL)</label>
                  <input v-model="aiConfig.apiBase" type="text" placeholder="https://api.openai.com/v1" class="bg-input border border-border rounded px-3 py-2 text-sm outline-none focus:border-accent">
                  <p class="text-[10px] text-gray-500">例如: https://api.deepseek.com</p>
              </div>
              
              <div class="flex flex-col gap-1">
                  <label class="text-xs text-gray-500">API 密钥 (Key)</label>
                  <input v-model="aiConfig.apiKey" type="password" placeholder="sk-..." class="bg-input border border-border rounded px-3 py-2 text-sm outline-none focus:border-accent">
              </div>
              
              <div class="flex flex-col gap-1">
                  <label class="text-xs text-gray-500">模型名称 (Model)</label>
                  <div class="flex gap-2">
                       <input 
                            v-model="aiConfig.model" 
                            type="text" 
                            placeholder="gpt-3.5-turbo" 
                            class="flex-1 bg-input border border-border rounded px-3 py-2 text-sm outline-none focus:border-accent"
                            list="model-list"
                        >
                       <datalist id="model-list">
                            <option v-for="m in availableModels" :key="m" :value="m" />
                       </datalist>

                       <button 
                            @click="fetchModels" 
                            class="p-2 bg-input border border-border rounded hover:border-accent hover:text-accent text-gray-400 transition-all disabled:opacity-50 flex items-center justify-center w-10 shrink-0"
                            :disabled="isFetchingModels"
                            title="刷新模型列表"
                        >
                            <span v-if="isFetchingModels" class="animate-spin">⏳</span>
                            <span v-else>🔄</span>
                       </button>
                  </div>
                  <div v-if="availableModels.length > 0" class="text-[10px] text-green-500 mt-1 flex justify-between">
                      <span>已加载 {{ availableModels.length }} 个模型</span>
                      <span class="cursor-pointer underline opacity-70 hover:opacity-100" @click="availableModels = []">清除</span>
                  </div>
              </div>

              <div class="flex flex-col gap-1">
                  <label class="text-xs text-gray-500 flex justify-between">
                      <span>拆单章提示词 (选填)</span>
                      <span class="text-[10px] text-accent cursor-pointer hover:underline" @click="aiConfig.promptChapter = ''">恢复默认</span>
                  </label>
                  <textarea 
                    v-model="aiConfig.promptChapter" 
                    placeholder="留空则使用默认的「网文主编拆解」提示词..." 
                    class="bg-input border border-border rounded px-3 py-2 text-xs outline-none focus:border-accent h-20 resize-none leading-relaxed"
                  ></textarea>
              </div>

              <div class="flex flex-col gap-1">
                  <label class="text-xs text-gray-500 flex justify-between">
                      <span>整本/前几章总结提示词 (选填)</span>
                      <span class="text-[10px] text-accent cursor-pointer hover:underline" @click="aiConfig.promptSummary = ''">恢复默认</span>
                  </label>
                  <textarea
                    v-model="aiConfig.promptSummary"
                    placeholder="留空则使用默认的「网文商业分析」提示词..."
                    class="bg-input border border-border rounded px-3 py-2 text-xs outline-none focus:border-accent h-20 resize-none leading-relaxed"
                  ></textarea>
              </div>

              <div class="flex flex-col gap-1">
                  <label class="text-xs text-gray-500">AI 分析读取章数</label>
                  <div class="flex items-center gap-2">
                      <input
                        v-model.number="aiConfig.analysisChapters"
                        type="number"
                        min="1"
                        max="50"
                        class="w-20 bg-input border border-border rounded px-3 py-2 text-sm outline-none focus:border-accent"
                      >
                      <span class="text-xs text-gray-500">章 (默认 5 章)</span>
                  </div>
                  <p class="text-[10px] text-gray-500">AI 分析时会读取前 N 章内容，章数越多分析越准确，但耗费 Token 越多</p>
              </div>

              <div class="flex items-center gap-2">
                  <input id="spider-visible" type="checkbox" v-model="aiConfig.spiderVisible" class="accent-accent">
                  <label for="spider-visible" class="text-xs text-gray-500">显示蜘蛛窗口（调试起点 WAF/验证码）</label>
              </div>
          </div>
          
          <div class="mt-6 flex justify-end gap-2">
              <button @click="showSettings = false" class="px-4 py-2 text-xs rounded hover:bg-white/5 text-gray-400">取消</button>
              <button @click="saveSettings" class="px-4 py-2 text-xs rounded bg-accent text-[var(--accent-text)] font-bold">保存配置</button>
          </div>
      </div>
  </div>

  <!-- Log Modal -->
  <div v-if="showLogs" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 backdrop-blur-sm">
      <div class="bg-card border border-border p-4 rounded-xl w-[600px] h-[500px] shadow-2xl flex flex-col">
          <div class="flex justify-between items-center mb-2">
              <h3 class="text-lg font-bold flex items-center gap-2">
                  📜 后台日志 (app.log)
              </h3>
              <div class="flex gap-2">
                  <button @click="clearLogs" class="text-red-400 hover:text-red-300 px-2 py-1 text-xs border border-red-400 rounded transition-colors hover:bg-red-500/20">清空日志</button>
                  <button @click="fetchLogs" class="text-accent hover:text-white px-2 py-1 text-xs border border-accent rounded transition-colors">刷新</button>
                  <button @click="showLogs = false" class="text-gray-400 hover:text-white text-xl leading-none">×</button>
              </div>
          </div>
          <textarea 
            id="log-textarea"
            readonly 
            class="flex-1 bg-black/30 border border-border rounded p-2 text-xs font-mono text-gray-300 outline-none resize-none whitespace-pre"
            :value="logContent"
          ></textarea>
      </div>
  </div>
</template>

<style scoped>
/* Tailwind handles most things, custom scrollbars if needed */
.bg-active {
    background-color: var(--active-bg);
}

/* Markdown 报告渲染样式 */
:deep(.prose) h1, :deep(.prose) h2, :deep(.prose) h3 {
    color: var(--accent-color, #f97316);
    margin-top: 1.2em;
    margin-bottom: 0.5em;
    font-weight: 700;
}
:deep(.prose) h1 { font-size: 1.5em; border-bottom: 1px solid rgba(255,255,255,0.1); padding-bottom: 0.3em; }
:deep(.prose) h2 { font-size: 1.25em; }
:deep(.prose) h3 { font-size: 1.1em; }
:deep(.prose) h4 { font-size: 1em; opacity: 0.9; }
:deep(.prose) p { margin: 0.6em 0; line-height: 1.7; color: var(--txt-color, #e5e5e5); font-size: 0.875rem; }
:deep(.prose) ul, :deep(.prose) ol { padding-left: 1.5em; margin: 0.5em 0; }
:deep(.prose) li { margin: 0.3em 0; font-size: 0.875rem; line-height: 1.6; color: var(--txt-color, #d4d4d4); }
:deep(.prose) blockquote { border-left: 3px solid var(--accent-color, #f97316); padding: 0.5em 1em; margin: 0.8em 0; background: rgba(255,255,255,0.03); border-radius: 0 8px 8px 0; }
:deep(.prose) blockquote p { margin: 0.2em 0; }
:deep(.prose) hr { border-color: rgba(255,255,255,0.08); margin: 1.5em 0; }
:deep(.prose) strong { color: #fff; }
:deep(.prose) a { color: var(--accent-color, #f97316); text-decoration: underline; }
:deep(.prose) code { background: rgba(255,255,255,0.08); padding: 0.15em 0.4em; border-radius: 4px; font-size: 0.85em; }
</style>
