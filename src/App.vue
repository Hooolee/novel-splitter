<script setup lang="ts">
import { ref, onMounted, nextTick, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { marked } from "marked";
import NovelCard, { type NovelListRow } from "./components/NovelCard.vue";

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

// Computed paths
const downloadsDir = computed(() => {
    if (!workspaceRoot.value) return '';
    return `${workspaceRoot.value}/downloads`;
});

// --- State ---
// V2.0 流水线由 trigger_full_scan 驱动；前端只保留「+ 添加书籍」入口的轻量配置。
const newBookPlatform = ref<'qidian' | 'fanqie'>('qidian');

// --- Tab Navigation ---
const activeTab = ref<'library' | 'reports'>('library');

// --- Reports State ---
const reportFiles = ref<string[]>([]);
const selectedReport = ref<string | null>(null);
const reportContent = ref('');

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
const currentPhase = ref<PipelineProgress | null>(null);

const isDownloading = ref(false);

const downloadLog = ref<string[]>([]);

// File Tree State
interface FileNode {
    name: string;
    path: string;
    is_dir: boolean;
    children: FileNode[];
    expanded?: boolean; 
}
const treeFiles = ref<FileNode[]>([]);

const selectedFile = ref<string | null>(null);
const fileContent = ref("");
const splitContent = ref("");
const isSplitting = ref(false);

// Metadata State
interface NovelMetadata {
    title: string;
    url: string;
    tags: string[];
    word_count: string;
    description: string;
    ai_analysis?: {
        genre: string;
        style: string;
        goldfinger: string;
        opening: string;
        highlights: string;
    }
}
const currentMetadata = ref<NovelMetadata | null>(null);

// --- Library DB cards (任务四a) ---
const novels = ref<NovelListRow[]>([]);
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
    currentMetadata.value = null;
    selectedFile.value = null;
    fileContent.value = '';
    splitContent.value = '';
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

            // Refresh file tree
            await refreshTreeFiles();
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

    // Listen for AI Streaming
    listen('ai-analysis', (event: any) => {
        splitContent.value += event.payload.chunk;
        // Auto scroll to bottom?
    });
    
    listen('ai-analysis-status', (event: any) => {
         const payload = event.payload;
         if (payload.status === 'start') {
             // splitContent.value = `[System] ${payload.message}\n\n`; // Don't wipe manual split content for auto-analysis
         } else if (payload.status === 'error') {
             splitContent.value += `\n[Error] ${payload.message}`;
             isSplitting.value = false;
         } else if (payload.status === 'done') {
             isSplitting.value = false;
             
             // Check if this was a JSON analysis result
             try {
                 // Try to parse the last block of content
                 // The AI might return markdown code blocks, so we need to clean it
                 // This is a bit hacky, depending on if we are in auto-analyze mode
                 // Ideally we should have a flag or separate event for auto-analysis
                 // But for now, let's just trigger a store update if it looks like JSON
             } catch (e) {
                 // ignore
             }
         } else {
             // Streaming content
             // Only append if we are viewing the split tab OR if we are capturing for auto-analysis?
             // Actually, for auto-analysis, we need to capture the stream separately.
             // Simplification: We will use the same 'ai-analysis' event but we need to know if it's for auto-analysis.
             // Since the backend doesn't support distinguish, we might see the content appearing in the "拆书结果" box.
             // That is acceptable for now.
         }
    });

    listen('download-progress', (event: any) => {
        const payload = event.payload;
        downloadLog.value.push(`[${new Date().toLocaleTimeString()}] ${payload.message}`);

        // Auto refresh tree on every minor completion or folder creation hint
        if (payload.status === 'completed' || payload.status === 'skipped' || payload.message.includes('下载完成') || payload.message.includes('已保存')) {
            refreshTreeFiles();
        }

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
        refreshTreeFiles();
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
            refreshTreeFiles();
            loadNovels();
        }
        if (payload.phase === 4 && payload.status === 'completed') {
            loadNovels();
        }
    });

    // Strategy 2: Periodic refresh while downloading (every 2s) to catch new folders
    setInterval(() => {
        if (isDownloading.value) {
            refreshTreeFiles();
        }
    }, 2000);
    
    refreshTreeFiles();
    loadReportFiles();
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
    currentMetadata.value = null;
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

async function refreshTreeFiles() {
    if (!downloadsDir.value) {
        treeFiles.value = [];
        return;
    }
    try {
        const res = await invoke("get_file_tree", { dirName: downloadsDir.value });
        const nodes = res as FileNode[];
        // Filter out info.json from top level (unlikely) or ensure children don't show it?
        // UI v-for will filter it easily.
        nodes.forEach(n => {
            if (n.is_dir) n.expanded = false; 
        });
        treeFiles.value = nodes;
    } catch (e) {
        console.error(e);
    }
}

async function loadNovelMetadata(path: string) {
    try {
        // Assume info.json is directly inside the novel folder
        // The path arg is relative to base dir, e.g. "NovelName"
        // So we want "NovelName/info.json"
        
        // Windows/Mac path separator issue? get_file_content handles path joining.
        // We construct the relative path string manually.
        const metaPath = path.endsWith('/') ? `${path}info.json` : `${path}/info.json`;
        
        const content = await invoke("get_file_content", { 
            dir: downloadsDir.value, 
            filename: metaPath
        });
        
        if (content) {
            currentMetadata.value = JSON.parse(content as string);
            fileContent.value = ""; // Clear text content to show metadata view
        }
    } catch (e) {
        // It's okay if metadata doesn't exist
        console.log("No metadata found for", path);
        currentMetadata.value = null;
    }
}

async function startSplit() {
    if (!fileContent.value) return;
    if (!aiConfig.value.apiKey) {
        showSettings.value = true;
        alert("请先配置 AI API Key");
        return;
    }

    isSplitting.value = true;
    splitContent.value = "准备连接 AI...\n";
    
    // Auto-save settings just in case
    saveSettings(); 

    try {
        await invoke("start_ai_analysis", {
            apiBase: aiConfig.value.apiBase,
            apiKey: aiConfig.value.apiKey,
            model: aiConfig.value.model,
            prompt: aiConfig.value.promptChapter,
            content: fileContent.value.substring(0, 3000), // Limit context window for safety
            responseJson: false
        });
    } catch (e) {
        splitContent.value = "启动失败: " + e;
        isSplitting.value = false;
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

function exportResult() {
    if (!splitContent.value) {
        alert("没有可导出的内容");
        return;
    }
    
    if (!selectedFile.value) {
        alert("请先选择一个章节");
        return;
    }
    
    // selectedFile format: "NovelName/01.txt"
    // Extract novel name and chapter index
    const pathParts = selectedFile.value.split('/');
    if (pathParts.length < 2) {
        alert("无法识别文件路径");
        return;
    }
    
    const novelName = pathParts[0];
    const fileName = pathParts[pathParts.length - 1];
    
    // Extract chapter index from filename (e.g., "01.txt" -> 1)
    const match = fileName.match(/(\d+)\.txt$/);
    if (!match) {
        alert("无法识别章节编号");
        return;
    }
    
    const chapterIndex = parseInt(match[1]);

    invoke("export_chapter", {
        novelTitle: novelName,
        chapterIndex: chapterIndex,
        content: splitContent.value,
        workspaceRoot: workspaceRoot.value || null
    }).then((path) => {
        alert(`导出成功！\n文件路径: ${path}`);
    }).catch((e) => {
        alert(`导出失败: ${e}`);
    });
}

async function autoAnalyze(novelName: string) {
    if (!aiConfig.value.apiKey) {
        downloadLog.value.push(`[System] Skipped analysis for ${novelName}: No API Key`);
        return;
    }
    
    downloadLog.value.push(`[System] Starting analysis for ${novelName}...`);

    try {
        // 1. Read first N chapters (configurable)
        const chaptersToRead = aiConfig.value.analysisChapters || 5;
        let fullContent = "";
        for (let i = 1; i <= chaptersToRead; i++) {
            const fileName = `${String(i).padStart(2, '0')}.txt`;
            const filePath = `${novelName}/${fileName}`; // Relative path
            try {
                const content = await invoke("get_file_content", {
                    dir: downloadsDir.value,
                    filename: filePath
                });
                fullContent += `\n\n--- 第 ${i} 章 ---\n\n${content}`;
            } catch (e) {
                // Ignore missing chapters (maybe less than configured)
            }
        }

        if (!fullContent.trim()) {
            downloadLog.value.push(`[System] Analysis failed: No content read`);
            return;
        }
        
        // 2. Prepare Prompt
        // 2. Prompt：用户可自定义总结提示词，空则回退后端默认
        const prompt: string = (aiConfig.value.promptSummary && aiConfig.value.promptSummary.trim())
            ? aiConfig.value.promptSummary
            : await invoke("get_auto_analysis_prompt");

        // 3. Call AI
        // We use a temporary way to capture the output since the backend streams to a global event
        // We will override the splitContent to show the user what is happening
        splitContent.value = `正在自动分析《${novelName}》...\n\n`;
        isSplitting.value = true;
        
        // We need to listen to the specific stream for this analysis
        // But since the global listener appends to splitContent, we can just watch splitContent or wait for 'done'
        // Ideally, we should refactor backend to return a RequestID and filter events.
        // For this prototype, we will wait for the 'ai-analysis-status' done event.
        
        let capturedOutput = "";
        const unlisten = await listen('ai-analysis', (event: any) => {
             capturedOutput += event.payload.chunk;
        });
        
        // Helper to wait for done
        const waitForDone = new Promise<void>((resolve, reject) => {
             const unlistenStatus = listen('ai-analysis-status', (event: any) => {
                 if (event.payload.status === 'done') {
                     unlistenStatus.then(f => f());
                     resolve();
                 } else if (event.payload.status === 'error') {
                     unlistenStatus.then(f => f());
                     reject(event.payload.message);
                 }
             });
        });
        
        await invoke("start_ai_analysis", {
            apiBase: aiConfig.value.apiBase,
            apiKey: aiConfig.value.apiKey,
            model: aiConfig.value.model,
            prompt: prompt,
            content: fullContent.substring(0, 15000), // Limit context
            responseJson: false // 禁用原生 json_object，避免某些代理层因为兼容问题直接返回空流
        });
        
        await waitForDone;
        unlisten(); // Stop listening
        
        // 4. Parse JSON
        let cleanedOutput = capturedOutput;
        // 移除 <think> 标签及其内容 (适配 deepseek-r1 等思考模型)
        cleanedOutput = cleanedOutput.replace(/<think>[\s\S]*?<\/think>/gi, '').trim();

        let jsonStr = cleanedOutput;
        // 尝试从 markdown 代码块中提取
        const jsonMatch = cleanedOutput.match(/```(?:json)?\s*([\s\S]*?)\s*```/i);
        if (jsonMatch) {
            jsonStr = jsonMatch[1];
        }
        
        // 更稳健的提取，避免 AI 返回额外文案导致解析失败
        const extractJson = (raw: string) => {
            try {
                return JSON.parse(raw);
            } catch (e1) {
                try {
                    // 尝试寻找最外层的 {}
                    const firstBrace = raw.indexOf('{');
                    const lastBrace = raw.lastIndexOf('}');
                    if (firstBrace !== -1 && lastBrace !== -1 && lastBrace > firstBrace) {
                        const inner = raw.substring(firstBrace, lastBrace + 1);
                        return JSON.parse(inner);
                    }
                } catch (e2) {
                    console.error("Secondary JSON parse failed:", e2);
                }
                console.error("Raw string that failed to parse:", raw);
                throw new Error("No valid JSON found in response.\nRaw Output:\n" + raw.substring(0, 2000));
            }
        };

        try {
            const analysis = extractJson(jsonStr);
            console.log("Analysis result:", analysis);
            
            // 5. Save to info.json via backend
            try {
                await invoke("update_novel_metadata", {
                    dirName: downloadsDir.value,
                    novelName: novelName,
                    metadata: { ai_analysis: analysis }
                });
                downloadLog.value.push(`[System] Analysis saved for ${novelName}`);
            } catch (saveErr: any) {
                console.error("Failed to save analysis:", saveErr);
                downloadLog.value.push(`[System] Analysis save failed: ${saveErr}`);
                return;
            }
            
            // 6. Refresh UI if we are looking at this novel
            if (currentMetadata.value && currentMetadata.value.title === novelName) {
                loadNovelMetadata(selectedFile.value || novelName); // Refresh
            }
            
        } catch (e: any) {
            console.error("Failed to parse AI response:", e);
            splitContent.value = "❌ 解析失败！AI 返回了不符合预期的格式。\n\n【原始返回内容截取】\n" + (e.message || e);
            downloadLog.value.push(`[System] Analysis failed: JSON parse error`);
        }
        
    } catch (e) {
        console.error("Auto analyze error:", e);
        downloadLog.value.push(`[System] Analysis error: ${e}`);
    } finally {
        isSplitting.value = false;
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
    isDownloading.value = true;
    logContent.value += `[${new Date().toLocaleTimeString()}] 已触发后台全量扫榜任务...\n`;
    activeTab.value = 'reports'; // 切换到报告页
    
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
    currentMetadata.value = null;
    fileContent.value = '';
    splitContent.value = '';
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
  <div class="h-screen flex text-txt bg-bg font-sans overflow-hidden">
    
    <!-- Sidebar -->
    <!-- Sidebar -->
    <!-- Sidebar -->
    <div class="w-72 bg-sidebar border-r border-border-dim flex flex-col p-5 flex-shrink-0 transition-all duration-300">
        <!-- Header -->
        <div class="flex items-center gap-3 mb-8 px-1">
            <div class="w-8 h-8 rounded-xl bg-gradient-to-br from-red-500 to-orange-600 flex items-center justify-center shadow-lg shadow-red-900/20">
                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="w-5 h-5 text-white">
                    <path d="M11.7 2.805a.75.75 0 0 1 .6 0A16.036 16.036 0 0 0 17.686 5.5c1.657.492 2.766 1.763 3.394 3.033.453.916.71 2.016.92 3.149.23 1.238.23 2.536.002 3.791-.191 1.054-.537 2.062-1.025 2.923-.526.927-1.398 1.956-2.825 2.668a1.503 1.503 0 0 1-1.304 0c-1.427-.712-2.299-1.741-2.825-2.668-.488-.861-.834-1.869-1.025-2.923-.228-1.255-.228-2.553.002-3.791.21-1.133.467-2.233.92-3.149.628-1.27 1.737-2.54 3.394-3.033a16.033 16.033 0 0 0 5.386 2.695.75.75 0 0 1 .458 1.06l-.99.99-.028.028a3.155 3.155 0 0 0-.67 1.264c-.055.234-.265.378-.49.337a.66.66 0 0 1-.502-.45c-.179-.646-.502-1.246-.938-1.758a.74.74 0 0 1 .09-1.03l.97-.88a14.522 14.522 0 0 1-4.225-2.004Z" />
                </svg>
            </div>
            <div>
                <h1 class="font-bold text-base text-txt tracking-tight leading-tight">拆书工具 <span class="text-accent text-xs align-top opacity-80 font-normal">Pro</span></h1>
                <p class="text-[10px] text-txt-dim font-medium tracking-wide uppercase">Novel Splitter</p>
            </div>
        </div>
        
        <!-- Tab Switcher -->
        <div class="bg-subtle p-1 rounded-lg flex mb-6 relative border border-border-dim">
            <div class="absolute top-1 bottom-1 rounded-md bg-active border border-border-dim shadow-sm transition-all duration-300 ease-out" :style="{ left: activeTab === 'library' ? '4px' : 'calc(50% + 1px)', width: 'calc(50% - 5px)' }"></div>
            <button @click="activeTab = 'library'" class="flex-1 relative z-10 text-xs font-medium py-1.5 text-center transition-colors duration-200" :class="activeTab === 'library' ? 'text-txt' : 'text-txt-dim hover:text-txt'">📚 书库</button>
            <button @click="activeTab = 'reports'; loadReportFiles()" class="flex-1 relative z-10 text-xs font-medium py-1.5 text-center transition-colors duration-200" :class="activeTab === 'reports' ? 'text-txt' : 'text-txt-dim hover:text-txt'">📊 报告</button>
        </div>

        <!-- ==================== 书库 Tab ==================== -->
        <template v-if="activeTab === 'library'">
        <!-- Workspace Directory Selector -->
        <div class="mb-4 px-1">
            <div class="flex items-center justify-between mb-2">
                <span class="text-[11px] font-bold text-txt-dim uppercase tracking-wider">工作目录</span>
            </div>
            <button
                @click="selectWorkspaceDir"
                class="w-full bg-subtle border border-border-dim rounded-lg px-3 py-2.5 text-left hover:bg-hover transition-colors group"
            >
                <div class="flex items-center gap-2">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-4 h-4 text-accent flex-shrink-0">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 0 1 4.5 9.75h15A2.25 2.25 0 0 1 21.75 12v.75m-8.69-6.44-2.12-2.12a1.5 1.5 0 0 0-1.061-.44H4.5A2.25 2.25 0 0 0 2.25 6v12a2.25 2.25 0 0 0 2.25 2.25h15A2.25 2.25 0 0 0 21.75 18V9a2.25 2.25 0 0 0-2.25-2.25h-5.379a1.5 1.5 0 0 1-1.06-.44Z" />
                    </svg>
                    <span v-if="workspaceRoot" class="text-xs text-txt truncate flex-1" :title="workspaceRoot">
                        {{ workspaceRoot.split('/').pop() || workspaceRoot }}
                    </span>
                    <span v-else class="text-xs text-txt-dim italic">点击选择目录...</span>
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-3.5 h-3.5 text-txt-dim group-hover:text-txt transition-colors flex-shrink-0">
                        <path stroke-linecap="round" stroke-linejoin="round" d="m8.25 4.5 7.5 7.5-7.5 7.5" />
                    </svg>
                </div>
                <div v-if="workspaceRoot" class="text-[10px] text-txt-dim mt-1 truncate opacity-60" :title="workspaceRoot">
                    {{ workspaceRoot }}
                </div>
            </button>
        </div>

        <!-- Resources Title -->
        <div class="flex items-center justify-between mb-3 px-1">
            <span class="text-[11px] font-bold text-txt-dim uppercase tracking-wider">Resources</span>
            <div class="flex items-center gap-1">
                <button @click="showAddBookModal = true" :disabled="!workspaceRoot || isDownloading" class="text-accent hover:text-orange-300 transition-colors p-1 rounded hover:bg-subtle disabled:opacity-30 disabled:cursor-not-allowed" title="添加单本书籍（走 V2.0 完整管线）">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="w-4 h-4">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                    </svg>
                </button>
                <button @click="loadNovels" class="text-txt-dim hover:text-txt transition-colors p-1 rounded hover:bg-subtle" title="刷新列表">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="w-3.5 h-3.5">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0l3.181 3.183a8.25 8.25 0 0013.803-3.7M4.031 9.865a8.25 8.25 0 0113.803-3.7l3.181 3.182m0-4.991v4.99" />
                    </svg>
                </button>
            </div>
        </div>
        
        <!-- Filter Bar -->
        <div class="flex flex-col gap-2 mb-3 px-1">
            <!-- Consensus filter -->
            <div class="flex flex-wrap gap-1">
                <button
                    v-for="opt in CONSENSUS_OPTIONS"
                    :key="opt.value"
                    @click="toggleConsensus(opt.value!)"
                    class="text-[10px] px-2 py-0.5 rounded border transition-all"
                    :class="consensusFilter.includes(opt.value!) ? 'bg-accent/20 border-accent text-accent' : 'border-border-dim text-txt-dim hover:text-txt'"
                >{{ opt.label }}</button>
            </div>

            <!-- Tag filter (limit display to keep sidebar tidy) -->
            <div v-if="allTags.length > 0" class="flex flex-wrap gap-1 max-h-20 overflow-y-auto">
                <button
                    v-for="t in allTags.slice(0, 30)"
                    :key="t"
                    @click="toggleTag(t)"
                    class="text-[10px] px-1.5 py-0.5 rounded border transition-all"
                    :class="tagFilter.includes(t) ? 'bg-accent/20 border-accent text-accent' : 'border-border-dim text-txt-dim hover:text-txt'"
                >{{ t }}</button>
            </div>

            <!-- Sort -->
            <select
                v-model="sortBy"
                @change="loadNovels"
                class="bg-input border border-border-dim rounded px-2 py-1 text-[11px] outline-none focus:border-accent"
            >
                <option value="updated_desc">按最近更新</option>
                <option value="latest_rank_asc">按最近排名</option>
                <option value="scan_count_desc">按上榜次数</option>
            </select>
        </div>

        <!-- Novel Grid -->
        <div class="flex-1 overflow-y-auto custom-scrollbar pb-4">
            <div v-if="novelsLoading" class="flex flex-col items-center justify-center h-full text-txt-dim text-xs gap-2">
                <span class="animate-spin text-2xl">⏳</span>
                <span>加载中…</span>
            </div>
            <div v-else-if="novels.length === 0" class="flex flex-col items-center justify-center h-full text-txt-dim text-xs gap-3 px-4">
                <div class="w-12 h-12 rounded-full bg-subtle flex items-center justify-center">
                    <span class="text-2xl opacity-30">📚</span>
                </div>
                <div class="text-center opacity-50">书库为空<br>点击右上角 + 添加单本，或到「报告」Tab 扫榜</div>
            </div>
            <div v-else class="grid grid-cols-1 gap-2">
                <NovelCard
                    v-for="n in novels"
                    :key="n.id"
                    :novel="n"
                    :selected="selectedNovel?.id === n.id"
                    @click="selectNovel"
                />
            </div>
        </div>
        </template>

        <!-- ==================== 报告 Tab ==================== -->
        <template v-if="activeTab === 'reports'">
        <div class="flex items-center justify-between mb-3 px-1">
            <span class="text-[11px] font-bold text-txt-dim uppercase tracking-wider">扫榜报告</span>
            <button @click="loadReportFiles" class="text-txt-dim hover:text-txt transition-colors p-1 rounded hover:bg-subtle" title="刷新">
                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="w-3.5 h-3.5"><path stroke-linecap="round" stroke-linejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0l3.181 3.183a8.25 8.25 0 0013.803-3.7M4.031 9.865a8.25 8.25 0 0113.803-3.7l3.181 3.182m0-4.991v4.99" /></svg>
            </button>
        </div>
        <div class="flex flex-col gap-2 mb-3">
            <!-- 美化后的下拉框 -->
            <div class="relative w-full">
                <select v-model="selectedRank" class="w-full appearance-none bg-subtle border border-border-dim text-txt text-[12px] rounded-lg pl-4 pr-10 py-2.5 outline-none focus:border-accent focus:ring-1 focus:ring-accent/30 transition-all cursor-pointer shadow-sm">
                    <option value="">(按配置 workflow_config.json 批量扫榜)</option>
                    <optgroup label="起点·热门排行" class="bg-subtle text-txt">
                        <option value="qidian:https://www.qidian.com/rank/hotsales/">畅销榜</option>
                        <option value="qidian:https://www.qidian.com/rank/yuepiao/">月票榜</option>
                        <option value="qidian:https://www.qidian.com/rank/readindex/">阅读指数榜</option>
                        <option value="qidian:https://www.qidian.com/rank/recom/">推荐榜</option>
                        <option value="qidian:https://www.qidian.com/rank/newfans/">书友榜</option>
                        <option value="qidian:https://www.qidian.com/rank/collect/">收藏榜</option>
                        <option value="qidian:https://www.qidian.com/rank/vipup/">更新榜</option>
                        <option value="qidian:https://www.qidian.com/rank/vipcollect/">VIP收藏榜</option>
                    </optgroup>
                    <optgroup label="起点·新书排行" class="bg-subtle text-txt">
                        <option value="qidian:https://www.qidian.com/rank/signnewbook/">签约作者新书榜</option>
                        <option value="qidian:https://www.qidian.com/rank/pubnewbook/">公众作者新书榜</option>
                        <option value="qidian:https://www.qidian.com/rank/newsign/">新人签约新书榜</option>
                        <option value="qidian:https://www.qidian.com/rank/newauthor/">新人作者新书榜</option>
                    </optgroup>
                    <optgroup label="起点·其他排行" class="bg-subtle text-txt">
                        <option value="qidian:https://www.qidian.com/rank/mm/">女生精选榜</option>
                        <option value="qidian:https://www.qidian.com/rank/mm/yuepiao/">女生月票榜</option>
                    </optgroup>
                    <optgroup label="番茄榜单 (开发中)" class="bg-subtle text-txt">
                        <option value="fanqie:https://fanqienovel.com/rank/1_2_1141">番茄男频阅读榜</option>
                        <option value="fanqie:https://fanqienovel.com/rank/0_2_1139">番茄女频阅读榜</option>
                    </optgroup>
                </select>
                <!-- 自定义右侧箭头 -->
                <div class="absolute inset-y-0 right-0 flex items-center pr-3 pointer-events-none text-txt-dim">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-4 h-4 opacity-70">
                        <path stroke-linecap="round" stroke-linejoin="round" d="m19.5 8.25-7.5 7.5-7.5-7.5" />
                    </svg>
                </div>
            </div>
            <button 
                @click="triggerFullScan"
                :disabled="isDownloading"
                class="w-full bg-gradient-to-r from-accent to-orange-500 text-[var(--accent-text)] font-bold py-2.5 px-4 rounded-lg hover:opacity-90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 text-xs shadow-sm"
            >
                <span v-if="isDownloading" class="animate-spin text-lg">⏳</span>
                <span>{{ isDownloading ? '后台扫榜中...' : '🚀 立即扫榜此榜单' }}</span>
            </button>
        </div>
        <div class="flex-1 bg-subtle rounded-xl border border-border-dim overflow-y-auto mb-4 select-none p-2 space-y-1 custom-scrollbar">
            <div v-if="reportFiles.length === 0" class="flex flex-col items-center justify-center h-full text-txt-dim text-xs gap-3">
                <div class="w-12 h-12 rounded-full bg-subtle flex items-center justify-center"><span class="text-2xl opacity-30">📊</span></div>
                <div class="text-center opacity-50">暂无报告<br>可通过系统托盘触发扫榜</div>
            </div>
            <div v-for="file in reportFiles" :key="file" @click="selectReport(file)"
                class="px-3 py-2.5 rounded-lg cursor-pointer transition-all duration-200 border flex items-center gap-3"
                :class="selectedReport === file ? 'bg-gradient-to-r from-accent/10 to-transparent border-l-2 border-l-accent border-transparent' : 'hover:bg-hover border-l-2 border-l-transparent border-transparent text-txt-dim hover:text-txt'"
            >
                <span class="text-lg">📊</span>
                <div class="flex-1 min-w-0">
                    <div class="truncate font-medium text-[12px]">{{ formatReportName(file) }}</div>
                    <div class="text-[10px] opacity-40 truncate">{{ file }}</div>
                </div>
            </div>
        </div>
        </template>

        <div class="mt-auto pt-4 border-t border-border-dim flex justify-between items-center group/settings">
            <span class="text-[10px] text-txt-dim group-hover/settings:text-txt transition-colors">SETTINGS</span>
            <div class="flex gap-1">
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
        </div>
    </div>

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
        <div class="flex-1 grid grid-cols-2 gap-4 min-h-0">
            <!-- Left: Original Content / Metadata View -->
            <div class="bg-card rounded-lg border border-border flex flex-col overflow-hidden">
                <div class="bg-white/5 px-4 py-2 border-b border-border flex justify-between items-center text-sm font-bold">
                    <span>📖 原文预览</span>
                    <span class="font-normal text-gray-500 text-xs">{{ selectedFile || '请在左侧选择' }}</span>
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

                    <div v-if="selectedNovel.ai_reviews" class="space-y-4">
                        <div class="bg-subtle p-4 rounded-lg border border-border-dim">
                            <div class="text-xs text-accent mb-2 uppercase tracking-wider font-bold">🎯 共识</div>
                            <div class="text-sm">{{ selectedNovel.ai_reviews.consensus ?? '—' }}</div>
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
                </div>

                <!-- Metadata Card View (Enhanced) -->
                 <div v-else-if="currentMetadata" class="flex-1 p-6 overflow-y-auto">
                     <div class="text-center mb-6">
                         <div class="text-5xl mb-3">📚</div>
                         <h2 class="text-xl font-bold text-accent mb-1">{{ currentMetadata.title }}</h2>
                         <div class="text-xs text-txt-dim">{{ currentMetadata.word_count }}</div>
                     </div>
                     
                     <div class="flex flex-wrap gap-1.5 justify-center mb-5">
                         <span v-for="tag in currentMetadata.tags" :key="tag" class="px-2 py-0.5 bg-subtle border border-border-dim rounded text-[11px] text-txt-dim">{{ tag }}</span>
                     </div>

                     <!-- AI 操作按钮组 -->
                     <div class="flex gap-2 mb-5">
                         <button @click="autoAnalyze(currentMetadata.title)" :disabled="isSplitting || !aiConfig.apiKey" class="flex-1 py-2 text-xs rounded-lg border border-accent/30 text-accent hover:bg-accent/10 transition-all disabled:opacity-30 disabled:cursor-not-allowed flex items-center justify-center gap-1.5">
                             <span>🔍</span> 商业分析
                         </button>
                         <button @click="() => { if(selectedFile) startSplit(); }" :disabled="!selectedFile || isSplitting || !aiConfig.apiKey" class="flex-1 py-2 text-xs rounded-lg border border-blue-400/30 text-blue-400 hover:bg-blue-400/10 transition-all disabled:opacity-30 disabled:cursor-not-allowed flex items-center justify-center gap-1.5">
                             <span>📖</span> 深度拆解
                         </button>
                     </div>
                     
                     <div class="bg-subtle p-4 rounded-lg text-left w-full border border-border-dim">
                         <div class="text-xs text-txt-dim mb-2 uppercase tracking-wider">简介</div>
                         <p class="text-sm leading-relaxed text-txt whitespace-pre-wrap">{{ currentMetadata.description }}</p>

                         <!-- AI Analysis Section -->
                         <div v-if="currentMetadata.ai_analysis" class="mt-5 pt-5 border-t border-border-dim">
                            <div class="text-xs text-accent mb-3 uppercase tracking-wider font-bold flex items-center gap-1">
                                <span>🤖 AI 深度分析</span>
                            </div>
                            <div class="space-y-2.5 text-xs">
                                <div class="grid grid-cols-[60px_1fr] gap-2"><span class="text-txt-dim">题材</span><span class="text-txt font-medium">{{ currentMetadata.ai_analysis.genre }}</span></div>
                                <div class="grid grid-cols-[60px_1fr] gap-2"><span class="text-txt-dim">风格</span><span class="text-txt font-medium">{{ currentMetadata.ai_analysis.style }}</span></div>
                                <div class="grid grid-cols-[60px_1fr] gap-2"><span class="text-txt-dim">金手指</span><span class="text-txt font-medium">{{ currentMetadata.ai_analysis.goldfinger }}</span></div>
                                <div class="space-y-1"><span class="text-txt-dim block">故事开头</span><p class="text-txt leading-relaxed opacity-90">{{ currentMetadata.ai_analysis.opening }}</p></div>
                                <div class="space-y-1"><span class="text-txt-dim block">核心看点</span><p class="text-txt leading-relaxed opacity-90">{{ currentMetadata.ai_analysis.highlights }}</p></div>
                            </div>
                         </div>
                     </div>
                 </div>

                <!-- Normal File Content -->
                <div v-else class="flex-1 p-4 overflow-y-auto whitespace-pre-wrap leading-relaxed text-sm font-serif">
                    {{ fileContent || '暂无内容...' }}
                </div>
            </div>

            <!-- Right: Analysis Result -->
            <div class="bg-card rounded-lg border border-border flex flex-col overflow-hidden">
                <div class="bg-white/5 px-4 py-2 border-b border-border flex justify-between items-center text-sm font-bold">
                    <span>🤖 拆书/分析结果</span>
                    <button @click="exportResult" class="text-accent text-xs border border-accent rounded px-2 py-0.5 hover:bg-accent hover:text-bg transition-colors">
                        📤 导出结果
                    </button>
                </div>
                <div class="flex-1 p-4 overflow-y-auto whitespace-pre-wrap font-mono text-sm text-blue-300">
                    {{ splitContent || '等待分析...' }}
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
