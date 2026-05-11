# 清理：移除 V1 废弃 Rust 命令

## Goal

V2.0 任务三 UI 瘦身已将前端完全迁移到 `trigger_full_scan` + `list_novels` 等新命令，但 `start_download` / `scan_and_download_rank` / `delete_novel` / `delete_chapter` 四个老命令仍留在 `lib.rs` 中（D6 决议保留、后续清理）。本任务彻底移除它们，同时清理 `ProgressPayload` 等专用结构体。

## 要删的命令

| 命令 | 理由 |
|---|---|
| `start_download` | 已由 `trigger_full_scan`（单本模式）替代 |
| `scan_and_download_rank` | 已由 `trigger_full_scan`（榜单模式）替代 |
| `delete_novel` | file-tree 已从 library Tab 移除，不再调用 |
| `delete_chapter` | 同上 |

## 范围

- 仅 Rust 端，不碰前端
- 不删还在前端使用的命令（get_file_content / get_file_tree / start_ai_analysis / read_log_file 等）
- `ProgressPayload` struct 若仅被删命令使用时一并移除

## Acceptance Criteria

- [ ] `cargo build` 通过
- [ ] `npm run build` 通过（前端无调用残留）
