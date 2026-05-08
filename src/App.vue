<script setup lang="ts">
import { ref, onMounted, nextTick, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { marked } from "marked";

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
const mode = ref<'single' | 'rank'>('single');
const platform = ref('fanqie');

// --- Tab Navigation ---
const activeTab = ref<'library' | 'reports' | 'download'>('library');

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
        const match = line.match(/^(#{1,3})\s+(.+)/);
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

// Single Mode Config
const url = ref("");
const count = ref(10);

// Rank Mode Config
const rankUrl = ref("");
const rankCount = ref(5);
const rankChapterCount = ref(5);

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

function saveSettings() {
    localStorage.setItem('ai_api_base', aiConfig.value.apiBase);
    localStorage.setItem('ai_api_key', aiConfig.value.apiKey);
    localStorage.setItem('ai_model', aiConfig.value.model);
    localStorage.setItem('ai_prompt_chapter', aiConfig.value.promptChapter);
    localStorage.setItem('ai_prompt_summary', aiConfig.value.promptSummary);
    localStorage.setItem('ai_analysis_chapters', String(aiConfig.value.analysisChapters));
    localStorage.setItem('spider_visible', String(aiConfig.value.spiderVisible));
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

onMounted(() => {
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

    // Strategy 2: Periodic refresh while downloading (every 2s) to catch new folders
    setInterval(() => {
        if (isDownloading.value) {
            refreshTreeFiles();
        }
    }, 2000);
    
    refreshTreeFiles();
    loadReportFiles();
});

async function startTask() {
    if (isDownloading.value) return;

    // Check if workspace is configured
    if (!workspaceRoot.value) {
        alert("请先选择工作目录");
        return;
    }

    isDownloading.value = true;
    downloadLog.value = [];
    currentMetadata.value = null; // Clear old metadata

    try {
        if (mode.value === 'single') {
            await invoke("start_download", {
                url: url.value,
                count: Number(count.value),
                dirName: downloadsDir.value,
                platform: platform.value,
                debugSpiderVisible: aiConfig.value.spiderVisible,
                workspaceRoot: workspaceRoot.value || null
            });
        } else {
             await invoke("scan_and_download_rank", {
                rankUrl: rankUrl.value,
                maxNovels: Number(rankCount.value),
                countPerNovel: Number(rankChapterCount.value),
                dirName: downloadsDir.value,
                platform: platform.value,
                debugSpiderVisible: aiConfig.value.spiderVisible,
                workspaceRoot: workspaceRoot.value || null
            });
        }
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

async function selectFile(node: FileNode) {
    // Hidden file check
    if (node.name === 'info.json') return;

    if (node.is_dir) {
        node.expanded = !node.expanded;
        selectedFile.value = node.path;
        // Try to load metadata when clicking folder
        loadNovelMetadata(node.path);
        return;
    }
    
    selectedFile.value = node.path;
    currentMetadata.value = null; // Switch to file view
    
    try {
        const content = await invoke("get_file_content", { 
            dir: downloadsDir.value, 
            filename: node.path 
        });
        fileContent.value = content as string;
        splitContent.value = ""; 
    } catch (e) {
        fileContent.value = "Error reading file: " + e;
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

async function deleteNovel(novelName: string) {
    console.log("deleteNovel called with:", novelName);

    try {
        console.log("Calling invoke delete_novel...");
        const result = await invoke("delete_novel", {
            dirName: downloadsDir.value,
            novelName: novelName,
            workspaceRoot: workspaceRoot.value || null
        });
        console.log("Delete result:", result);
        alert(result);

        // 如果删除的是当前选中的小说，清空相关状态
        if (selectedFile.value && selectedFile.value.startsWith(novelName)) {
            selectedFile.value = null;
            fileContent.value = "";
            splitContent.value = "";
            currentMetadata.value = null;
        }

        await refreshTreeFiles();
    } catch (e) {
        console.error("Delete error:", e);
        alert(`删除失败: ${e}`);
    }
}

async function deleteChapter(novelName: string, chapterFile: string) {
    console.log("deleteChapter called with:", novelName, chapterFile);

    try {
        console.log("Calling invoke delete_chapter...");
        const result = await invoke("delete_chapter", {
            dirName: downloadsDir.value,
            novelName: novelName,
            chapterFile: chapterFile,
            workspaceRoot: workspaceRoot.value || null
        });
        console.log("Delete chapter result:", result);
        alert(result);

        // 如果删除的是当前选中的章节，清空相关状态
        const deletedPath = `${novelName}/${chapterFile}`;
        if (selectedFile.value === deletedPath) {
            selectedFile.value = null;
            fileContent.value = "";
            splitContent.value = "";
        }

        await refreshTreeFiles();
    } catch (e) {
        console.error("Delete chapter error:", e);
        alert(`删除失败: ${e}`);
    }
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
            <div class="absolute top-1 bottom-1 rounded-md bg-active border border-border-dim shadow-sm transition-all duration-300 ease-out" :style="{ left: activeTab === 'library' ? '4px' : activeTab === 'reports' ? 'calc(33.33% + 1px)' : 'calc(66.66% + 2px)', width: 'calc(33.33% - 5px)' }"></div>
            <button @click="activeTab = 'library'" class="flex-1 relative z-10 text-xs font-medium py-1.5 text-center transition-colors duration-200" :class="activeTab === 'library' ? 'text-txt' : 'text-txt-dim hover:text-txt'">📚 书库</button>
            <button @click="activeTab = 'reports'; loadReportFiles()" class="flex-1 relative z-10 text-xs font-medium py-1.5 text-center transition-colors duration-200" :class="activeTab === 'reports' ? 'text-txt' : 'text-txt-dim hover:text-txt'">📊 报告</button>
            <button @click="activeTab = 'download'" class="flex-1 relative z-10 text-xs font-medium py-1.5 text-center transition-colors duration-200" :class="activeTab === 'download' ? 'text-txt' : 'text-txt-dim hover:text-txt'">⬇ 下载</button>
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
            <button @click="refreshTreeFiles" class="text-txt-dim hover:text-txt transition-colors p-1 rounded hover:bg-subtle" title="刷新列表">
                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="w-3.5 h-3.5">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0l3.181 3.183a8.25 8.25 0 0013.803-3.7M4.031 9.865a8.25 8.25 0 0113.803-3.7l3.181 3.182m0-4.991v4.99" />
                </svg>
            </button>
        </div>
        
        <!-- Tree View -->
        <div class="flex-1 bg-subtle rounded-xl border border-border-dim overflow-y-auto mb-4 select-none p-2 space-y-1 custom-scrollbar">
            <!-- No workspace selected -->
            <div v-if="!workspaceRoot" class="flex flex-col items-center justify-center h-full text-txt-dim text-xs gap-3">
                <div class="w-12 h-12 rounded-full bg-subtle flex items-center justify-center">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-6 h-6 opacity-30">
                         <path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 0 1 4.5 9.75h15A2.25 2.25 0 0 1 21.75 12v.75m-8.69-6.44-2.12-2.12a1.5 1.5 0 0 0-1.061-.44H4.5A2.25 2.25 0 0 0 2.25 6v12a2.25 2.25 0 0 0 2.25 2.25h15A2.25 2.25 0 0 0 21.75 18V9a2.25 2.25 0 0 0-2.25-2.25h-5.379a1.5 1.5 0 0 1-1.06-.44Z" />
                    </svg>
                </div>
                <div class="text-center opacity-50">请先选择<br>工作目录</div>
            </div>
            <!-- Workspace selected but empty -->
            <div v-else-if="treeFiles.length === 0" class="flex flex-col items-center justify-center h-full text-txt-dim text-xs gap-3">
                <div class="w-12 h-12 rounded-full bg-subtle flex items-center justify-center">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-6 h-6 opacity-30">
                         <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 9.776c.112-.017.227-.026.344-.026h15.812c.117 0 .232.009.344.026m-16.5 0a2.25 2.25 0 00-1.883 2.542l.857 6a2.25 2.25 0 002.227 1.932H19.05a2.25 2.25 0 002.227-1.932l.857-6a2.25 2.25 0 00-1.883-2.542m-16.5 0V6A2.25 2.25 0 016 3.75h3.879a1.5 1.5 0 011.06.44l2.122 2.12a1.5 1.5 0 001.06.44H18A2.25 2.25 0 0120.25 9v.776" />
                    </svg>
                </div>
                <div class="text-center opacity-50">暂无书籍<br>请在上方下载</div>
            </div>
            
            <template v-for="node in treeFiles" :key="node.path">
                <!-- Level 1: Novel (Folder) Card Style -->
                <div v-if="node.name !== 'info.json'" class="group relative">
                    <div 
                        @click="selectFile(node)"
                        class="px-3 py-2.5 rounded-lg cursor-pointer transition-all duration-200 border border-transparent flex items-center gap-3 relative overflow-hidden"
                        :class="[
                            selectedFile === node.path && node.is_dir 
                            ? 'bg-gradient-to-r from-accent/10 to-transparent border-l-2 border-l-accent' 
                            : 'hover:bg-hover border-l-2 border-l-transparent text-txt-dim hover:text-txt'
                        ]"
                    >
                        <!-- Icon -->
                        <span v-if="node.is_dir" class="relative z-10 transition-transform duration-300" :class="{'rotate-90': node.expanded}">
                             <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="w-5 h-5 text-accent opacity-80">
                                <path d="M19.5 21a3 3 0 0 0 3-3v-4.5a3 3 0 0 0-3-3h-15a3 3 0 0 0-3 3V18a3 3 0 0 0 3 3h15ZM1.5 10.146V6a3 3 0 0 1 3-3h5.379a2.25 2.25 0 0 1 1.59.659l2.122 2.121c.14.141.331.22.53.22H19.5a3 3 0 0 1 3 3v1.146A4.483 4.483 0 0 0 19.5 9h-15a4.483 4.483 0 0 0-3 1.146Z" />
                             </svg>
                        </span>
                        <span v-else class="text-lg">📄</span>
                        
                        <div class="flex-1 min-w-0 flex flex-col justify-center relative z-10">
                            <div class="truncate font-medium text-[13px] leading-none mb-1" :class="selectedFile === node.path ? 'text-accent font-bold' : ''">{{ node.name }}</div>
                            <div v-if="node.is_dir" class="text-[10px] opacity-40 leading-none">{{ node.children.filter(c => c.name !== 'info.json').length }} 章节</div>
                        </div>

                        <!-- AI 分析按钮 -->
                        <button
                            v-if="node.is_dir"
                            @click.stop="autoAnalyze(node.name)"
                            :disabled="isSplitting || !aiConfig.apiKey"
                            class="opacity-0 group-hover:opacity-100 transition-all text-txt-dim hover:text-accent p-1.5 rounded-md hover:bg-subtle disabled:opacity-30 disabled:cursor-not-allowed"
                            :title="!aiConfig.apiKey ? '请先配置 API Key' : 'AI 分析'"
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-3.5 h-3.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M9.813 15.904 9 18.75l-.813-2.846a4.5 4.5 0 0 0-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 0 0 3.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 0 0 3.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 0 0-3.09 3.09ZM18.259 8.715 18 9.75l-.259-1.035a3.375 3.375 0 0 0-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 0 0 2.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 0 0 2.456 2.456L21.75 6l-1.035.259a3.375 3.375 0 0 0-2.456 2.456ZM16.894 20.567 16.5 21.75l-.394-1.183a2.25 2.25 0 0 0-1.423-1.423L13.5 18.75l1.183-.394a2.25 2.25 0 0 0 1.423-1.423l.394-1.183.394 1.183a2.25 2.25 0 0 0 1.423 1.423l1.183.394-1.183.394a2.25 2.25 0 0 0-1.423 1.423Z" />
                            </svg>
                        </button>

                        <!-- Delete button for novel -->
                        <button 
                            v-if="node.is_dir"
                            @click.stop="deleteNovel(node.name)"
                            class="opacity-0 group-hover:opacity-100 transition-all text-txt-dim hover:text-red-400 p-1.5 rounded-md hover:bg-subtle"
                            title="删除小说"
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-3.5 h-3.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0" />
                            </svg>
                        </button>
                    </div>

                    <!-- Level 2: Chapters -->
                    <div v-if="node.is_dir && node.expanded" class="relative mt-1 ml-3 pl-3 border-l border-border-dim space-y-0.5">
                        <template v-for="child in node.children" :key="child.path">
                            <div 
                                v-if="child.name !== 'info.json'"
                                class="px-3 py-2 rounded-md cursor-pointer text-xs flex items-center gap-2 transition-all relative group/chapter"
                                :class="{'bg-accent/10 text-accent font-medium': selectedFile === child.path, 'hover:bg-subtle text-txt-dim hover:text-txt': selectedFile !== child.path}"
                                @click="selectFile(child)"
                            >
                                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-3.5 h-3.5 opacity-70 flex-shrink-0">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 0 0-3.375-3.375h-1.5A1.125 1.125 0 0 1 13.5 7.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 0 0-9-9Z" />
                                </svg>
                                
                                <span class="truncate flex-1 relative top-[0.5px]">{{ child.name.replace('.txt', '') }}</span>
                                
                                <!-- Delete button for chapter -->
                                <button 
                                    @click.stop="deleteChapter(node.name, child.name)"
                                    class="opacity-0 group-hover/chapter:opacity-100 transition-opacity text-txt-dim hover:text-red-400 p-1 rounded hover:bg-subtle"
                                    title="删除章节"
                                >
                                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-3 h-3">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                                    </svg>
                                </button>
                                
                                <div v-if="selectedFile === child.path" class="absolute w-1 h-1 bg-accent rounded-full -left-[17px] top-1/2 -translate-y-1/2 shadow-[0_0_8px_rgba(var(--accent-color),0.5)]"></div>
                            </div>
                        </template>
                         <div v-if="node.children.length === 0" class="pl-3 py-2 text-[10px] text-txt-dim italic">
                            (暂无章节)
                        </div>
                    </div>
                </div>
            </template>
        </div>

        <button 
            @click="startSplit"
            :disabled="!selectedFile || isSplitting || currentMetadata !== null"
            class="w-full bg-gradient-to-r from-subtle to-bg-active border border-border-dim text-txt-dim font-bold py-2.5 px-4 rounded-lg hover:from-accent hover:to-accent hover:text-accent-text hover:border-transparent transition-all disabled:opacity-30 disabled:cursor-not-allowed mb-4 flex items-center justify-center gap-2 shadow-sm"
        >
            <span v-if="isSplitting" class="animate-spin text-lg">⏳</span>
            <span v-else class="text-sm">⚡️ 开始拆书</span>
        </button>
        </template>

        <!-- ==================== 报告 Tab ==================== -->
        <template v-if="activeTab === 'reports'">
        <div class="flex items-center justify-between mb-3 px-1">
            <span class="text-[11px] font-bold text-txt-dim uppercase tracking-wider">扫榜报告</span>
            <button @click="loadReportFiles" class="text-txt-dim hover:text-txt transition-colors p-1 rounded hover:bg-subtle" title="刷新">
                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="w-3.5 h-3.5"><path stroke-linecap="round" stroke-linejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0l3.181 3.183a8.25 8.25 0 0013.803-3.7M4.031 9.865a8.25 8.25 0 0113.803-3.7l3.181 3.182m0-4.991v4.99" /></svg>
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

        <!-- ==================== 下载 Tab ==================== -->
        <template v-if="activeTab === 'download'">
        <div class="space-y-4 px-1 flex-1 overflow-y-auto">
            <div class="flex flex-col gap-1">
                <label class="text-xs text-gray-400">平台</label>
                <select v-model="platform" class="bg-input border border-border rounded px-2 py-2 text-sm focus:border-accent outline-none appearance-none">
                    <option value="fanqie">🍅 番茄小说</option>
                    <option value="qidian">📖 起点中文网</option>
                </select>
            </div>
            <div class="flex flex-col gap-1">
                <label class="text-xs text-gray-400">下载模式</label>
                <div class="flex gap-2">
                    <button @click="mode = 'single'" class="flex-1 py-1.5 text-xs rounded-md border transition-all" :class="mode === 'single' ? 'bg-accent/20 border-accent text-accent' : 'border-border-dim text-txt-dim hover:text-txt'">单本</button>
                    <button @click="mode = 'rank'" class="flex-1 py-1.5 text-xs rounded-md border transition-all" :class="mode === 'rank' ? 'bg-accent/20 border-accent text-accent' : 'border-border-dim text-txt-dim hover:text-txt'">榜单</button>
                </div>
            </div>
            <template v-if="mode === 'single'">
                <div class="flex flex-col gap-1"><label class="text-xs text-gray-400">小说主页链接</label><input v-model="url" type="text" class="bg-input border border-border rounded px-3 py-2 text-sm focus:border-accent outline-none"></div>
                <div class="flex flex-col gap-1"><label class="text-xs text-gray-400">抓取章数</label><input v-model="count" type="number" class="bg-input border border-border rounded px-3 py-2 text-sm focus:border-accent outline-none"></div>
            </template>
            <template v-else>
                <div class="flex flex-col gap-1"><label class="text-xs text-gray-400">榜单链接</label><input v-model="rankUrl" type="text" class="bg-input border border-border rounded px-3 py-2 text-sm focus:border-accent outline-none"></div>
                <div class="flex flex-col gap-1"><label class="text-xs text-gray-400">抓取本数</label><input v-model="rankCount" type="number" min="1" class="bg-input border border-border rounded px-3 py-2 text-sm focus:border-accent outline-none"></div>
                <div class="flex flex-col gap-1"><label class="text-xs text-gray-400">每本章数</label><input v-model="rankChapterCount" type="number" class="bg-input border border-border rounded px-3 py-2 text-sm focus:border-accent outline-none"></div>
            </template>
            <button @click="startTask" :disabled="isDownloading || !workspaceRoot" class="w-full bg-accent text-[var(--accent-text)] font-bold px-4 py-2.5 rounded-lg hover:opacity-90 text-xs disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2">
                {{ isDownloading ? '⬇ 运行中...' : (mode === 'single' ? '⬇ 开始下载' : '🚀 扫榜下载') }}
            </button>
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
                
                <!-- Metadata Card View (Enhanced) -->
                 <div v-if="currentMetadata" class="flex-1 p-6 overflow-y-auto">
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
                {{ isDownloading ? (mode === 'single' ? '正在下载...' : '正在扫榜...') : '就绪' }}
            </span>
            <span v-if="downloadLog.length > 0" class="text-accent truncate flex-1">
                {{ downloadLog[downloadLog.length - 1] }}
            </span>
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
