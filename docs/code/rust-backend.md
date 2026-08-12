# Rust 后端详情

> `src-tauri/src/` 下 25 个 Rust 源文件的命令清单、数据模型、架构决策。
> 顶层索引见 [CODE_INDEX.md](../../CODE_INDEX.md#8-rust-后端25)。

---

## 主模块

### main.rs（6 行）

应用入口，调 `pgb1_lib::run()`。

### lib.rs（375 行）

**Tauri 初始化**：
- 命令注册（约 90 个）
- 插件注册：`single-instance` / `log` / `autostart` / `opener` / `drag` / `dialog` / `clipboard` / `notification` / `updater` / `process`
- Windows Acrylic 毛玻璃
- 调度器初始化 + 补打检测
- `hotkey` 全局快捷键初始化
- 启动时同步 `autolaunch` 状态

**关闭行为**：`CloseRequested` 拦截 + `hide()` 最小化到系统托盘，不退出。

### models.rs（681 行）

**29 个 struct + 3 个 enum**。见 [数据模型](#数据模型) 小节。

### conversion.rs（153 行）

**转换管理**：
- `ConversionSession` 状态管理（含 `tp_scale` / `tp_webp_quality` TP 预设参数）
- `bring_window_to_front`（Win32 API）
- `handle_file_event`（监控 `01_scale/` 递归）

**双路径支持**：
- 普通任务：`[XX]/file.webp` → `[img-XX]/file.webp`
- Prototype：`[XX]/{subcat}/file.webp` → `[img-XX]/{subcat}/file.webp`

**v2.5.2 事件载荷修复**：`conversion-organized` 事件 payload 对 Prototype 携带 `subcat/stem` 格式（与前端 images map key 对齐），普通任务仍为 `stem`。

### hotkey.rs（144 行）

**全局快捷键**（独立线程 Win32 消息循环）：
- `start_hotkey_listener`
- `do_toggle_window`
- `parse_shortcut` 支持计算器键（`0xB7`）

### scheduler.rs（262 行）

**考勤调度器**：
- `AttendanceScheduler`：管理 3 个常驻定时任务（出勤/退勤/日报）。
  旧「临时加班定时任务」（schedule_overtime）随 OvertimePage 死链一并删除（2026-06-10）
- `Arc<Mutex<AttendanceScheduler>>` 作为 Tauri State
- `create_reminder_window`：400×200 毛玻璃置顶弹窗
  - **visible(false) 创建 + Rust 侧 500ms 延迟 show() 双保险**（由 ReminderPage onMounted 调 show()）
- `calc_duration_until`

**日报预热**：
- `DAILY_REPORT_PRE_WARM_SECS = 90`
- `pre_warm_daily_report`：提前 90 秒创建隐藏 WebView 加载 Google Docs
- 在 `daily_timer_loop` 中日报提醒前 90 秒触发

### migrations.rs（95 行）

**安装目录迁移**（v2.8.10 起 productName 由「PG素材管理系统」→「PGB1」的善后）：
- `migrate_legacy_install`：新版首次启动时静默清理旧中文 productName 安装残留（`%LOCALAPPDATA%\Programs\PG素材管理系统\` 及老快捷方式），失败只记日志不中断启动
- **爆炸半径自律**：仅固定字面量路径，不 glob、不递归、不碰 `AppData\Roaming`；目录需含 `.exe` 才认定为 NSIS 安装目录后清理

---

## commands/ 子模块

### commands/mod.rs（27 行）

`pub use` 重导出所有子模块的命令，统一对外接口（含 `psd` / `workflow_paths` 子模块）。

### commands/workflow_paths.rs（72 行，2026-06-10 新增）

**工作流目录命名 SSOT**（与前端 `src/config/projectPaths.ts` 对齐）：
- 目录名常量：`DIR_ORIGINAL` / `DIR_SCALE` / `DIR_DONE` / `DIR_PREVIEW` / `DIR_NEXTCLOUD` / `DIR_NC_PREVIEW` / `DIR_NC_BREAKDOWN` / `DIR_NC_ORIGINAL` / `DIR_EXPORT` / `DIR_AE` / `DIR_PSD` 等
- 阶段前缀常量：`STAGE_PREFIX_ANIM`（an）/ `STAGE_PREFIX_IMG`（img）
- 路径构造：`vfx_dir` / `export_dir` / `nextcloud_dir` / `nextcloud_task_dir` / `img_dir_name` / `an_dir_name` / `stage_dir_prefix`

### commands/scanning.rs（1324 行）—— 最大文件

**扫描命令**：
- `scan_projects` / `scan_tasks` / `scan_materials` / `scan_directory`
- `list_sequence_frames`
- `scan_material_versions`
- `scan_preview_videos`
- `scan_app_shortcuts`

**DirSnapshot 缓存**：目录快照减少重复 IO。
- `from_dir`：普通任务（key = 阶段子目录名，value = 子目录内条目）
- `from_dir_subcat`：Prototype 专用（value = `[XX]/{subcat}/` 子层条目），使所有查询方法对 Prototype 同样适用——普通/Prototype 共用 `determine_progress_image_cached` / `determine_progress_sequence_cached`（2026-06-10 双轨合并，旧 `determine_progress_prototype_*` / `find_file_in_*` 直读函数已删除）
- `from_nextcloud_dir`：根层（"." 键）+ original/ 子目录（原件直传，方案 B）

**Prototype 处理**：`split_prototype_name` + 按子分类构建快照（每子分类 3 次批量读取）。

### commands/attendance.rs（1138 行）

**考勤命令**：
- `load_attendance_config` / `save_attendance_config`
- `save_attendance_password` / `load_attendance_password`（Windows Credential Manager）
- `execute_clock_action`（WebView 登录 → 导航到打刻 → 点击出勤/退勤 → 更新记录）
- `show_clock_webview` / `close_clock_webview`
- `open_daily_report`（含预热 + 滚动）
- `test_reminder`
- `load_attendance_record` / `save_attendance_record`
- `reschedule_attendance`
- `open_reminder_window`（前端主动打开提醒窗口，用于「结束加班」按钮触发退勤打卡）

**日报自动化关键 JS**：
- `DAILY_REPORT_INIT_JS` 常量（`initialization_script` 注入）检测 `readyState=complete` + `.kix-cursor-caret` 两条件，满足后写 `#pgb-ready` hash
- `spawn_daily_report_scroll` 轮询就绪后 JS focus + HWND 置顶 + `send_ctrl_end()` 跳到文档末尾
- `send_ctrl_end()` / `scroll_to_bottom_via_wheel`（Win32 API 物理滚动）

### commands/conversion.rs（1336 行）

**转换/缩放命令**：
- `scan_normalize_items` / `execute_normalize_v2` / `restore_normalize_backup`（Phase 5b+ 规范化独立页面：全量盘点 + 命名/自适应画布/加黑底三操作 + `.normalize_backup` 备份/恢复 + `normalize-progress` 事件；备份按规范后名做 key 保留最早纯净原件；图像辅助 `trim_transparent` / `composite_on_black`）
- `execute_scaling`（Phase 5c，Lanczos3 + `scaling-progress` 事件）
- `start_conversion` / `stop_conversion`（Phase 5d 静帧 webp 监控）
- `execute_sequence_conversion`（序列帧 TexturePacker）
- `collect_drag_files`（Phase 5a OS 级拖拽收集）
- `copy_to_nextcloud` / `import_files` / `copy_preview_to_nextcloud`

**Prototype 处理**：`collect_best_files` / `collect_matching_files_in_subdirs` 统一接受 `sub_name: Option<&str>` 参数（None = 普通任务，Some = 深入子分类一层），不再有 `_prototype` 孪生函数（2026-06-10 合并）。

### commands/projects.rs（707 行）

**项目管理命令**：
- `create_project` / `rename_project` / `delete_project`（移入回收站）
- `update_project_deadline`
- `set_project_priority` / `set_task_priority`
- `toggle_subtask_completion`
- `mark_upload_prompted`
- `load_global_tasks` / `save_global_tasks`
- `apply_task_changes`（核心：对比新旧启用列表 → 创建/归档任务文件夹）
- `list_archived_tasks` / `restore_archived_task` / `delete_archived_version`（时光机）

**Sprint 3·Y-19 关键重构**：4 个"读→改→写回"命令改用 `mutate_project_config` helper（见 [commands/helpers.rs](#commandshelpersrs521-行)）。

### commands/shortcuts.rs（591 行）

**快捷方式命令**：
- `load_shortcuts` / `save_shortcuts`
- `launch_shortcut`（app/folder/web 三类启动方式）
- `extract_exe_icon`（256px JUMBO SHGetImageList + BGRA→RGBA）
- `fetch_favicon`（HTML 解析 + `favicon.ico` 降级 + 验证 ≥32px）
- `copy_icon_to_cache`
- `find_game_exe`（Unity `UnityCrashHandler64.exe` 指纹 + Godot `.pck` 同名配对）

### commands/helpers.rs（661 行）

**公共辅助函数**：
- **扩展名常量 SSOT**（与前端 `fileTypes.ts` 对齐）：`IMAGE_EXTS` / `VIDEO_EXTS` / `FRAME_EXTS` + `material_type_from_ext`
- `matches_base_name`
- `calc_dir_size`
- `regex_strip_version` / `extract_version_number`
- `count_upload_progress` / `count_preview_progress`
- **`mutate_project_config`**（Sprint 3·Y-19 新增）：`.pgb1_project.json` 的"读→闭包改→写回"原子 helper，消除 4 个命令的模板重复
- `move_dir`：同卷优先 `fs::rename`（原子瞬时），跨卷回退复制+删除（2026-06-10）
- `copy_dir_recursive`（symlink/junction 防环）

> 注：`DirSnapshot` 在 scanning.rs；PSD 缩略图已拆到 `commands/psd.rs`。

### commands/psd.rs（203 行，2026-06-10 从 scanning.rs 拆出）

**PSD/PSB 缩略图**：
- `extract_psd_thumbnail`（命令）：PSD 图层合并 + PSB 内嵌 JPEG fallback，`PSD_SEMAPHORE(2)` 限并发，磁盘缓存
- `psd_cache_file`：缓存路径 hash 计算（路径 + mtime + max_size），与 `scan_directory` 的命中检查共用，杜绝两处 hash 逻辑漂移

### commands/translation.rs（346 行）

**翻译命令入口**：
- `translate_text_stream`（SSE 流式 Gemini API）
- `toggle_translator_window`
- `translate_text_once`（单次翻译 + 6 次重试）
- `extract_pdf_pages_text`

PDF 构建底层（字体/排版/命令整合）拆到 `translation/` 子模块。

### commands/translation/pdf_font.rs（216 行）

**CJK 字体处理**（Sprint 3·Y-7 剥离）：
- `load_cjk_font_bytes`：`msyh` / `simhei` 候选
- `extract_single_ttf_from_data`：TTC → TTF 表重定位
- `add_yahe_font`：`Type0` + `Identity-H` + `FontFile2` + `ToUnicode CMap`

### commands/translation/pdf_reflow.rs（464 行）

**PDF 内容流提取 + 流式排版**（Sprint 3·Y-7 剥离）：
- `obj_to_f32`
- `compute_avg_font_size`
- `extract_image_placements`（CTM 栈追踪 XObject 显示尺寸）
- `get_page_xobject_dict`
- `render_flow_pages`（字符宽度换行 + 图片穿插 + 自适应分页）

### commands/translation/pdf_cmds.rs（199 行）

**PDF 命令整合**（Sprint 3·Y-7 剥离）：
- `build_translated_pdf`（reflow 架构整合者）
- `check_translated_pdf_exists`

内部调用 `pdf_font` / `pdf_reflow`。通配符 re-export 到 `translation.rs`，`generate_handler!` 能同时找到原函数和 `__cmd__` wrapper。

### commands/holiday.rs（148 行）

**外部 API 代理**（Sprint 3·Y-13 新增）：
- `fetch_ip_country`（ipapi.co，返回 2 字母国家代码）
- `fetch_cn_holiday_type`（timor.tech，返回 `Option<i32>`，0 工作日/1 假日/2 调休）
- `fetch_nager_holidays`（date.nager.at）

**决策理由**：把前端 fetch 迁到 Rust `reqwest`，避免 IP 泄漏 + 收敛 CSP `connect-src`。

### commands/files.rs（768 行）

**文件操作命令**：
- `open_file`（`ShellExecuteW "open"`）
- `rename_file`（保留扩展名 + 校验非法字符）
- `delete_file`（`SHFileOperationW + FOF_ALLOWUNDO` 回收站）
- `read_text_file`
- `rename_material` / `delete_material`
- `rename_sequence_fps`
- `set_default_ae_file`（Sprint 3·Y-19 改用 `mutate_project_config`）

**素材归档命令（v2.8.13 新增，对齐任务归档三段式）**：
- `delete_material` 行为重写：`00_original` / `01_scale/<sub>/` / `02_done/<sub>/` 命中项 move 到 `<project>/.archived_materials/<Task>/<BaseName>/timestamp_YYYY-MM-DD_HH-MM/<stage>/`；`nextcloud` 命中副本直接 `fs::remove_*`（nextcloud 仅作本地上传标记，非云端本体，不进归档）
- `list_archived_materials(project_path)` — 返回 `Vec<ArchivedMaterialVersion>`，含 60 天懒 GC（调用时顺带 `remove_dir_all` 过期归档 + 清理空 base/task 目录）
- `restore_archived_material(project_path, task_name, base_name, timestamp)` — **拒绝式冲突**：预检目标位置已存在的同名文件，列出冲突清单并报错；无冲突则 move 归档内容回原位并清理归档目录
- `delete_archived_material_version(project_path, task_name, base_name, timestamp)` — 手动物理删除单个归档版本

**内部 helpers**（非 `#[tauri::command]`）：
- `infer_archived_material_type(ts_path)` — 看 `00_original` 首个条目推断 image/sequence/video/other
- `scan_archive_content(ts_path)` — 返回 (总字节数, stages 列表)
- `compute_path_size(path)` — 递归计算文件/目录字节数
- `collect_restore_conflicts(archive_dir, target_dir, stage_label, &mut conflicts)` — 冲突预检
- `restore_stage_dir(archive_dir, target_dir)` — move 归档阶段目录内容回原位

**rename_material 帧文件下划线兼容**：序列帧帧文件命名为 `{base_name}_{帧号}.png`（下划线分隔），`matches_base_name` 只认连字符，故 sequence 分支内对帧文件单独放宽判定为 `stem == base_name || stem.starts_with("{base_name}_") || stem.starts_with("{base_name}-")`，外层目录判定保持严格。

### commands/pinboard.rs（184 行）

**贴图板命令**：
- `get_pinboard` / `save_pinboard`
- `save_pin_image`（RGBA → PNG）
- `delete_pin_image`
- `open_pinboard_window`

### commands/settings.rs（69 行）

**设置命令**：
- `load_settings`（首次运行创建空默认值）
- `save_settings`
- `set_default_ae_file`

### commands/notes.rs（39 行）

**笔记命令**：
- `get_notes`（读 `.pgb1_notes.json`）
- `set_note`（读-改-写，空时删文件）

---

## 已注册命令速查表

| 命令 | 参数 | 返回 | 职责 |
|------|------|------|------|
| `scan_projects` | `root_dir` | `Vec<ProjectInfo>` | 扫描项目根目录，含 `completed_tasks` + `app_icon` 查找 |
| `scan_tasks` | `project_path` | `Vec<TaskInfo>` | 扫描 Export 目录任务，大小取 nextcloud + `video_total/uploaded` |
| `scan_materials` | `task_path` | `Vec<MaterialInfo>` | 核心扫描：00_original → 各阶段判定进度 + scales + fps，支持 Prototype |
| `scan_directory` | `app_handle, dir_path` | `Vec<FileEntry>` | 通用一层扫描 + PSD/PSB 缓存命中时填 `thumbnail_path` |
| `scan_material_versions` | `task_path, base_name, material_type` | `Vec<MaterialVersion>` | 各工作流阶段版本列表，支持 Prototype `subcat/basename` |
| `list_sequence_frames` | `dir_path` | `Vec<String>` | 序列帧目录的帧文件路径列表 |
| `scan_preview_videos` | `task_path, nextcloud_preview_path` | `Vec<PreviewVideoEntry>` | 03_preview 扫描 + 上传状态对比（uploaded/outdated/none） |
| `scan_app_shortcuts` | — | `Vec<AppShortcut>` | 扫描开始菜单 + 桌面 `.lnk`，COM 解析目标 exe |
| `open_in_explorer` | `path` | `()` | Windows explorer 打开路径 |
| `collect_drag_files` | `task_path, materials` | `Vec<String>` | Phase 5a 拖拽收集（02_done > 01_scale > 00_original） |
| `copy_to_nextcloud` | `task_path, material_names` | `CopyResult` | Phase 5a 复制 02_done 到 nextcloud（排除 `.tps`） |
| `import_files` | `source_paths, target_dir` | `ImportResult` | 通用文件导入（同名跳过） |
| `load_global_tasks` / `save_global_tasks` | `root_dir[, config]` | `GlobalTaskConfig` / `()` | `.pgb1_global_tasks.json` CRUD |
| `apply_task_changes` | `project_path, enabled_tasks` | `ApplyTaskResult` | 核心：对比启用列表 → 创建/归档任务文件夹 |
| `list_archived_tasks` / `restore_archived_task` / `delete_archived_version` | ... | ... | 时光机：60 天过期清理 |
| `scan_normalize_items` / `execute_normalize_v2` / `restore_normalize_backup` | `task_path` ／ `app_handle, requests, backup` ／ `current_path, backup_name` | `Vec<NormalizeItem>` ／ `()` ／ `()` | Phase 5b+ 规范化页面：全量盘点 + 命名/裁透明/加黑底 + 备份/恢复 |
| `execute_scaling` | `app_handle, requests` | `()` | Phase 5c Lanczos3 + `scaling-progress` 事件 |
| `start_conversion` / `stop_conversion` | ... | `()` | Phase 5d 转换会话启停 |
| `execute_sequence_conversion` | `app_handle, sequences` | `()` | 序列帧 TexturePacker 转换流程 |
| `create_project` | `root_dir, project_name, deadline?` | `ProjectInfo` | 新建：目录骨架 + `.pgb1_project.json` + PSD 8 个固定子目录 |
| `rename_project` | `project_path, new_name` | `ProjectInfo` | 重命名目录 + 更新 `project_name` |
| `delete_project` | `project_path` | `()` | 移入回收站 + `.pgb1_project.json` 存在性安全检查 |
| `update_project_deadline` | `project_path, deadline?` | `()` | 更新截止日期 |
| `set_project_priority` | `project_path, priority?` | `()` | 项目优先度写入 |
| `set_task_priority` | `project_path, task_name, priority?` | `()` | 任务优先度写入 `task_priorities` Map |
| `toggle_subtask_completion` | `project_path, subtask_key` | `Vec<String>` | 切换子任务完成状态 |
| `mark_upload_prompted` | `project_path, task_name, prompted` | `()` | 标记/取消上传提醒状态 |
| `load/save_attendance_config` / `load/save_attendance_password` | ... | ... | 打卡配置 + Credential Manager |
| `execute_clock_action` | `app_handle, action` | `String` | WebView 打卡自动化 |
| `show_clock_webview` / `close_clock_webview` | `app_handle` | `()` | 前台显示 / 关闭打卡 WebView |
| `open_daily_report` | `app_handle` | `()` | 日报 WebView + 预热命中检测 + 自动滚动 |
| `test_reminder` | `app_handle, reminder_type` | `()` | 设置页测试：spawn 异步触发提醒弹窗 |
| `load/save_attendance_record` | ... | ... | 本地打卡记录 |
| `open_reminder_window` | ... | ... | spawn 异步创建弹窗避免 sync 命令死锁 |
| `reschedule_attendance` | `app_handle, scheduler` | `()` | 重置所有定时任务 |
| `translate_text_stream` | `app_handle, api_key, model, lang_a, lang_b, text` | `()` | 流式 Gemini SSE，逐块 emit `translate-chunk` |
| `translate_text_once` | `app_handle, api_key, model, text, page_index?` | `String` | 单次翻译 + 6 次重试（10s×2^n，上限 120s） |
| `toggle_translator_window` | `app_handle` | `()` | 切换翻译窗口显隐 |
| `open_pinboard_window` | `app_handle, dir_path, canvas_key, title` | `()` | 贴图板窗口（已存在 emit `pinboard-open-tab`，否则创建） |
| `load_shortcuts` / `save_shortcuts` / `launch_shortcut` | ... | ... | 快捷方式 CRUD + 启动 |
| `extract_exe_icon` | `app_handle, exe_path, icon_id` | `String` | exe 图标 → PNG（JUMBO 256px） |
| `fetch_favicon` | `app_handle, url, icon_id` | `Option<String>` | 网页 favicon |
| `rename_material` / `delete_material` | ... | ... | 素材全版本重命名 / 删除（v2.8.13 起 delete 改走归档） |
| `list_archived_materials` / `restore_archived_material` / `delete_archived_material_version` | ... | ... | 素材归档时光机：60 天 GC + 拒绝式冲突恢复（v2.8.13 新增） |
| `rename_file` / `delete_file` | ... | ... | 单文件重命名/回收站 |
| `read_text_file` | `path` | `String` | TXT 预览 |
| `find_game_exe` | `root_dir` | `Option<String>` | Unity/Godot 原型检测 |
| `open_file` | `path` | `()` | `ShellExecuteW "open"` 系统关联 |
| `rename_sequence_fps` | `task_path, base_name, old_fps, new_fps` | `()` | 序列帧帧率重命名目录 |
| `edit_sequence_tps` | `tps_path, gui_path` | `()` | 序列帧「修改」：阻塞打开 TP GUI，关闭后重解析 scale，变了就把 `[an-旧-fps]` 重命名为 `[an-新-fps]`（gui_path 空则退回系统关联打开、不重整理）。定义在 `conversion.rs` |
| `set_default_ae_file` | `project_path, file_name` | `()` | 默认 AE 工程名 |
| `copy_preview_to_nextcloud` | `file_path, nextcloud_preview_path` | `()` | 预览视频复制（breakdown 自动路由） |
| `extract_psd_thumbnail` | `app_handle, path, max_size` | `Option<String>` | PSD 图层合并 + PSB 内嵌 JPEG，磁盘缓存 |
| `get_file_mtime` | `path` | `u64` | 文件修改时间（PSD 缓存失效判断） |
| `get_notes` / `set_note` | ... | ... | `.pgb1_notes.json` CRUD |
| `get_pinboard` / `save_pinboard` / `save_pin_image` / `delete_pin_image` | ... | ... | 贴图板数据 CRUD |
| `extract_pdf_pages_text` | `path` | `Vec<String>` | `pdf-extract` 逐页文字 + 空白页检测 |
| `build_translated_pdf` | `path, translations` | `String` | reflow 架构：图文混排生成 `{stem}_zh.pdf` |
| `check_translated_pdf_exists` | `path` | `Option<String>` | 检测译文存在 |
| `fetch_ip_country` / `fetch_cn_holiday_type` / `fetch_nager_holidays` | ... | ... | Sprint 3·Y-13 外部 API 代理 |

---

## 数据模型

### 项目与任务

| 模型 | 用途 |
|------|------|
| `ProjectConfig` | `.pgb1_project.json` 文件结构（`enabled_tasks` / `archived_tasks` / `completed_subtasks` / `upload_prompted_tasks` / `default_ae_file` / `priority` / `task_priorities`） |
| `ProjectInfo` | 项目 DTO（含 `completed_tasks` / `default_ae_file` / `app_icon` / `priority` / `note`），`scan_projects` 顺带从 `.pgb1_notes.json` 读 `card:{name}` key |
| `TaskInfo` | 任务 DTO（含 `material_total` / `material_uploaded` / `video_total` / `video_uploaded` / `priority` / `note`），大小取 nextcloud 目录 |

### 素材与文件

| 模型 | 用途 |
|------|------|
| `FileEntry` | 文件/目录条目 DTO，含 `thumbnail_path: Option<String>`（PSD/PSB 缓存路径，`skip_serializing_if = None`） |
| `PreviewVideoEntry` | 预览视频条目（`name` / `path` / `extension` / `size_bytes` / `upload_status`） |
| `MaterialInfo` | 素材 DTO（含 `scales: Vec<u32>` / `fps: Option<u32>`） |
| `MaterialVersion` | 版本 DTO（含 `stage` / `scale` / `folder_path` / `size_bytes`），序列帧原始版本 `folder_path` = 目录本身 |
| `DragMaterialRequest` / `CopyMaterialRequest` | 拖拽/复制请求 DTO |
| `CopyResult` / `ImportResult` | 操作结果 DTO（count + errors） |

### 任务管理三层架构

| 模型 | 用途 |
|------|------|
| `GlobalTaskConfig` | `.pgb1_global_tasks.json`，`tasks: Vec<GlobalTask>` |
| `GlobalTask` | 全局任务（`name` + `children: Vec<GlobalTaskChild>`） |
| `GlobalTaskChild` | 子任务（`name`） |
| `ApplyTaskResult` | 任务变更结果（`created` / `archived` / `errors`） |
| `ArchivedVersion` | 归档版本（`task_name` / `timestamp` / `display_time` / `path`） |

### 考勤

| 模型 | 用途 |
|------|------|
| `AttendanceConfig` | 日报打卡配置（`mode: "off"/"auto"/"record_only"` + attendance + daily_report + username） |
| `AttendanceSettings` | 考勤设置（`clock_in_time` / `clock_out_time` / `url` / `lunch_start_time?` / `lunch_end_time?`） |
| `DailyReportSettings` | 日报设置（`enabled` + `time` + `url`） |
| `AttendanceRecord` | 本地打卡记录（`last_clock_in` / `last_clock_out` 日期字符串） |

### 枚举

| 枚举 | 值 |
|------|------|
| `MaterialType` | `Image` / `Sequence` / `Video` / `Other` |
| `MaterialProgress` | `None` / `Original` / `Scaled` / `Done` / `Uploaded` |
| `ShortcutType` | `App` / `Folder` / `Web` |

### 快捷方式

| 模型 | 用途 |
|------|------|
| `Shortcut` | 快捷方式 DTO（`id` / `shortcut_type` / `name` / `path` / `icon_cache` / `order`） |
| `ShortcutsConfig` | `shortcuts.json` 文件结构（`shortcuts: Vec<Shortcut>`） |
| `AppShortcut` | Windows 应用快捷方式（`name` / `target_path`） |

### 贴图板

| 模型 | 用途 |
|------|------|
| `PinAnnotation` | 标注 DTO（`type: pen/arrow/rect/ellipse/text/eraser` + 几何参数），`#[serde(rename_all = "camelCase")]` |
| `PinInfo` | 单张贴图 DTO（`id` / `image` / `x` / `y` / `width` / `height` / `annotations` / `z_index` / `created_at`） |
| `PinboardViewport` | 画布视口（`pan_x` / `pan_y` / `zoom`） |
| `PinboardCanvas` | 单个画布数据（`pins` / `viewport?` / `annotations`） |
| `PinboardData` | `.pgb1_pinboard.json` 结构：`HashMap<String, PinboardCanvas>` |

### 应用设置

| 模型 | 用途 |
|------|------|
| `AppSettings` | 根设置（含 `workflow` / `translation` / `general` / `preview` 四个子对象） |
| `GeneralSettings` | `ui_scale` 默认 1.0，`auto_start: bool` 默认 false |
| `WorkflowSettings` | `tp_scale: f64`（默认 0.5） / `tp_webp_quality: u32`（默认 80） |
| `PreviewSettings` | `default_fps` / `background_transparent` |
| `StartConversionRequest` | 含 `tp_scale` / `tp_webp_quality` 字段（与 WorkflowSettings 对齐） |

---

## 关键架构决策

### 任务管理三层架构

全局任务清单（`.pgb1_global_tasks.json`） → 项目启用列表（`.pgb1_project.json` 的 `enabled_tasks`） → 文件系统（`Export/` + `nextcloud/` 目录）。

归档到 `.archived_tasks/{TaskName}/timestamp_{YYYY-MM-DD_HH-MM}/`。

### Prototype 特例处理

后端按 `split_prototype_name` 拆分 `"subcat/basename"` 格式，各阶段目录深入 `subcat` 子目录。前端按 `name` 中的 `/` 分组。

新建项目时：
- `00_original` / `02_done` 下创建 7 个固定子分类目录
- `01_scale` 只建空目录（子分类目录由缩放操作按需创建）
- PSD 8 个固定子目录在 `create_project` 时一次性创建

### 转换流程

**静帧**：
1. 监控 `01_scale/`（递归）
2. 检测新 `.webp`
3. 按所在 `[XX]` 目录名解析比例
4. 移到 `02_done/[img-XX]/`

**序列帧**：
1. 从 `00_original/` 读取
2. TexturePacker CLI（`--scale=tp_scale` / `--webp-quality=tp_webp_quality` / `--opt` 按尾缀 "normal" 判定 RGBA8888/RGB888）
3. patch `.tps globalSpriteSettings.scale`（1 → tp_scale）
4. GUI 用户调整
5. 检测 `.webp` 存在（否则删 `.tps` + emit `sequence-conversion-failed`）
6. `parse_tps_scale` 锚定 `globalSpriteSettings` 读实际 scale
7. 整理三件套到 `02_done/[an-XX-YY]/`

### 考勤调度系统

`scheduler.rs` 管理 3 个常驻定时任务（出勤/退勤/日报）。`Arc<Mutex<AttendanceScheduler>>` 作为 Tauri State。

**提醒弹窗** = 独立 `WebviewWindow`（400×200 毛玻璃置顶），指向 Vue 路由 `/reminder/:type`。

**日报预热**：`daily_timer_loop` 在日报提醒前 90 秒触发 `pre_warm_daily_report` 创建隐藏 WebView 加载 Google Docs。用户打开时发现已存在 → show + focus + `spawn_daily_report_scroll`（预热命中检测 `#pgb-ready` hash 后直接滚动，跳过轮询）。

### 翻译系统

`hotkey.rs` 在独立线程运行 Win32 消息循环，监听全局热键。首次按键时动态创建 400×500 可调大小 `always_on_top` WebviewWindow，加载 `/translator` 路由，延迟 50ms 应用 Acrylic 毛玻璃。

**SSE 流式**：`translate_text_stream` 调用 Gemini `streamGenerateContent?alt=sse` 端点，spawn 异步任务 + `response.chunk()` 循环读取 SSE 流 → buffer 累积 + `\n\n` 分割 → emit `translate-chunk` 增量文本。

**模型自由输入**：设置页 `<input list>` + `<datalist>` 替代固定 `<select>`，用户可选预设也可手动输入任意模型 ID。

### 进度计算规则

**分母** = 无子任务的父任务数 + 所有子任务数（有子任务的父任务本身不计入）

**完成判定**：
- 无子任务父任务：nextcloud 目录全素材已上传（`completed_tasks`）
- 有子任务父任务：所有子任务在 `completed_subtasks` 中

### Tauri sync 命令死锁规避

`#[tauri::command]` 内创建窗口会死锁。所有创建窗口的操作（`test_reminder` / `open_pinboard_window` / `open_reminder_window`）都用 `tauri::async_runtime::spawn` 异步化。
