# PGB1 全量代码优化方案

> **日期**: 2026-04-02
> **状态**: 已定稿，待执行
> **扫描范围**: 66 个源文件，~32,450 行代码
> **发现问题**: 83 项（High 8 / Medium 18 / Low 12）

---

## 执行原则

- **先备份**：每个 Tier 开始前 git commit 当前状态
- **外科手术**：只改必要部分，不顺手格式化
- **逐项验证**：每完成一项跑 `npm run tauri dev` 确认不 break
- **文档同步**：重构完成后更新 CODE_INDEX.md

---

## Tier 1 — 高优先级（架构级，收益最大）

### 1.1 commands.rs 拆分为子模块

**现状**: `src-tauri/src/commands.rs` 7695 行，70 个命令 + 所有辅助函数混在一个文件

**方案**: 按功能域拆分为 `src-tauri/src/commands/` 目录下的子模块

```
src-tauri/src/commands/
├── mod.rs              # pub use 重导出所有命令
├── scanning.rs         # scan_projects, scan_tasks, scan_materials, scan_directory,
│                       # scan_material_versions, scan_preview_videos,
│                       # count_upload_progress, count_preview_progress,
│                       # determine_progress_*, collect_scales_*, find_file_*
│                       # (~1400 行)
├── conversion.rs       # preview_normalize, execute_normalize, execute_scaling,
│                       # start_conversion, stop_conversion, execute_sequence_conversion
│                       # (~600 行)
├── attendance.rs       # load/save_attendance_config, save/load_attendance_password,
│                       # execute_clock_action, show_clock_webview, close_clock_webview,
│                       # open_daily_report, test_reminder, load/save_attendance_record,
│                       # schedule_overtime_reminder, show_overtime_dialog,
│                       # reschedule_attendance, spawn_daily_report_scroll,
│                       # send_ctrl_end, scroll_to_bottom_via_wheel,
│                       # DAILY_REPORT_INIT_JS 常量
│                       # (~1200 行)
├── translation.rs      # translate_text_stream, toggle_translator_window,
│                       # extract_pdf_pages_text, translate_text_once,
│                       # build_translated_pdf, check_translated_pdf_exists
│                       # (~1500 行)
├── shortcuts.rs        # load/save_shortcuts, launch_shortcut, scan_app_shortcuts,
│                       # extract_exe_icon, fetch_favicon, copy_icon_to_cache
│                       # (~500 行)
├── files.rs            # open_in_explorer, open_file, import_files,
│                       # collect_drag_files, copy_to_nextcloud, copy_preview_to_nextcloud,
│                       # rename_material, delete_material, rename_file, delete_file,
│                       # read_text_file, get_file_mtime, find_game_exe
│                       # (~500 行)
├── pinboard.rs         # get_pinboard, save_pinboard, save_pin_image, delete_pin_image,
│                       # open_pinboard_window
│                       # (~200 行)
├── projects.rs         # create_project, delete_project, rename_project,
│                       # update_project_deadline, set_project_priority,
│                       # set_task_priority, set_default_ae_file,
│                       # load/save_global_tasks, apply_task_changes,
│                       # list_archived_tasks, restore_archived_task,
│                       # delete_archived_version, toggle_subtask_completion,
│                       # mark_upload_prompted
│                       # (~800 行)
├── notes.rs            # get_notes, set_note
│                       # (~60 行)
├── settings.rs         # load_settings, save_settings
│                       # (~100 行)
└── helpers.rs          # calc_dir_size, regex_strip_version, extract_version_number,
                        # split_prototype_name, collect_workflow_dirs(新)
                        # (~300 行)
```

**关键**: `lib.rs` 的 `generate_handler!` 宏不受影响，只要函数 `pub` 可见即可。`mod.rs` 用 `pub use` 重导出。

**风险**: 低。纯文件组织重构，不改任何逻辑。

---

### 1.2 提取弹窗公共样式 `dialog.css`

**现状**: 6 个 Dialog 组件各自定义了 ~80 行相同的 CSS（overlay/content/title/body/actions/btn/btn-primary/btn-secondary + 进出场动画）

**涉及文件**:
- `src/components/NormalizationDialog.vue` (L457-483 动画, L190-455 样式)
- `src/components/ConversionDialog.vue` (L415-439 动画, L198-413 样式)
- `src/components/CreateProjectDialog.vue` (L222-248 动画, L111-220 样式)
- `src/components/EditProjectDialog.vue` (L268-294 动画, L136-266 样式)
- `src/components/UploadConfirmDialog.vue` (L125-151 动画, L41-123 样式)
- `src/components/NoteDialog.vue` (L92-166 样式)

**方案**:
1. 新建 `src/styles/dialog.css`，定义公共类：
   - `.dialog-overlay` — 遮罩层（fixed, z-index, backdrop）
   - `.dialog-content` — 内容容器（glass, max-width, padding）
   - `.dialog-title` / `.dialog-body` / `.dialog-actions` — 标题/内容/底部按钮区
   - `.dialog-btn` / `.dialog-btn-primary` / `.dialog-btn-secondary` — 按钮
   - `.dialog-enter-active` / `.dialog-leave-active` / `.dialog-enter-from` / `.dialog-leave-to` — 动画
2. 在 `main.ts` 中 import
3. 各 Dialog 组件删除重复样式，仅保留差异（如 width/max-width 覆盖）

**预计减少**: ~500 行 CSS

---

### 1.3 提取 `useDragIntent` composable

**现状**: TaskPage (L187-227, L268-299, L838-872), MaterialsPage (L328-367), GameIntroPage (L184-221) 各自实现了相同的拖拽意图检测模式（mousedown → 记录起点 → mousemove 距离判断 → 超 5px 阈值触发 startDrag）

**方案**:
```typescript
// src/composables/useDragIntent.ts
export function useDragIntent(options: {
  threshold?: number        // 默认 5
  onDragStart: (paths: string[], event: MouseEvent) => void
  isMultiSelect?: Ref<boolean>
  selectedPaths?: Ref<Set<string>>
}) {
  function onCardMouseDown(path: string, event: MouseEvent) {
    // 统一的 mousedown → mousemove → threshold → startDrag 逻辑
  }
  return { onCardMouseDown }
}
```

**涉及文件**: TaskPage, MaterialsPage, GameIntroPage
**预计减少**: ~200 行

---

### 1.4 提取 `useMultiSelect` composable

**现状**: TaskPage (L87-121), MaterialsPage (L105-131), GameIntroPage (L74-101), ConvertPage (L63-71) 各自实现了 toggleMultiSelect/toggleSelectAll/toggleSelection/isAllSelected

**方案**:
```typescript
// src/composables/useMultiSelect.ts
export function useMultiSelect<T>(
  items: Ref<T[]> | ComputedRef<T[]>,
  pathExtractor: (item: T) => string
) {
  const isMultiSelect = ref(false)
  const selectedPaths = ref<Set<string>>(new Set())
  const isAllSelected = computed(...)
  function toggleMultiSelect() { ... }
  function toggleSelectAll() { ... }
  function toggleSelection(path: string) { ... }
  return { isMultiSelect, selectedPaths, isAllSelected, toggleMultiSelect, toggleSelectAll, toggleSelection }
}
```

**涉及文件**: TaskPage, MaterialsPage, GameIntroPage, ConvertPage
**预计减少**: ~150 行

---

### 1.5 提取绘图逻辑 `useAnnotationDrawing` + `canvasRenderer.ts`

**现状**: PinItem (L159-397) 和 PinboardCanvas (L194-410) 各自实现了完整的绘图事件处理 + Canvas 渲染逻辑，区别仅在坐标系（归一化 0-1 vs 世界坐标）

**方案**:
1. `src/utils/canvasRenderer.ts` — 纯函数：`drawAnnotation(ctx, annotation, coordTransform)`
2. `src/composables/useAnnotationDrawing.ts` — 绘图事件状态管理（mousedown/move/up、文字输入、笔刷光标），通过 `coordTransform` 参数适配不同坐标系

**涉及文件**: PinItem.vue, PinboardCanvas.vue
**预计减少**: ~200 行

---

### 1.6 TaskPage 拆分 (2268行)

**现状**: 单文件包含素材列表、双视图、多选、拖拽、预览视频、侧边栏、子任务弹窗、上传确认、笔记弹窗等全部逻辑

**方案**: 提取 3 个子组件
1. **SubtaskDialog.vue** — 子任务管理弹窗（当前 L571-666 逻辑 + L1562-1600 模板）
2. **PreviewVideoSection.vue** — 预览视频区块（当前 L349-441 + L838-916 逻辑 + L1302-1336 模板）
3. 侧边栏相关逻辑通过 1.3/1.4 composable 提取后自然精简

**预计**: TaskPage 从 2268 行降至 ~1500 行

---

### 1.7 FileDetailSidebar 拆分 (1358行)

**现状**: 包含视频播放控制器、PDF 翻译 UI、文件类型判断、重命名/删除弹窗、全屏 FLIP 动画、拖拽宽度调整、700+ 行非 scoped CSS

**方案**: 提取 2 个子组件
1. **VideoPlayer.vue** — 视频播放控制 + 自定义控制条（当前 L14-104 逻辑 + 模板视频区）
2. **PdfPreviewSection.vue** — PDF 预览 + 翻译 UI（当前 L137-151 + 模板 L556-600）

**预计**: FileDetailSidebar 从 1358 行降至 ~700 行

---

### 1.8 ScalePage/ConvertPage 控制面板统一

**现状**: ScalePage (L307-528) 和 ConvertPage (L547-858) 因 Teleport 到 `#content-row` 使用非 scoped `<style>`，~200 行面板 CSS 几乎完全相同且全局污染

**方案**: 两个选项
- **A**: 提取 `ControlPanel.vue` 组件，内部用 scoped style，通过 slot 接收面板内容
- **B**: 将共享面板样式提取到 `design-system.css` 的 `.control-panel-*` 公共类

**推荐**: 方案 B（改动更小，不需要新组件）

---

## Tier 2 — 中优先级（性能 + Bug + 去重）

### 2.1 scan_materials 预读目录结构 [Rust]

**现状**: `commands.rs` L556-777，每个素材调用 2-4 次目录扫描判定进度 + 1 次收集 scales + 1 次收集 fps。20 素材的任务约 100+ 次 `read_dir`。

**方案**: 在 `scan_materials` 顶部一次性读取 `01_scale/`、`02_done/`、`nextcloud/` 的目录结构到 `HashMap<String, Vec<String>>`，后续 O(1) 查找。

**预计影响**: 大量素材场景扫描时间降低一个数量级

---

### 2.2 base_name 前缀匹配 Bug 修复 [Rust]

**现状**: `commands.rs` L1507 `name.starts_with(base_name)` — 如 base_name="fire" 会误匹配 "fireball.webp"。同样问题出现在 L1356 (done_size_image)、L5410 (rename_material)、L5507 (delete_material)。

**方案**: 改为精确 stem 比较：
```rust
// 旧: name.starts_with(base_name)
// 新: file_stem == base_name || file_stem.starts_with(&format!("{base_name}-"))
```
后者兼容 TexturePacker multipack 输出的 `name-0.webp`。

**影响**: 修复潜在误操作（极端情况下重命名/删除错误文件）

---

### 2.3 打卡 WebView 登录流程去重 [Rust]

**现状**: `commands.rs` execute_clock_action_inner (L3520-3878) 和 test_clock_action_inner (L3905-4181) 约 200 行登录流程几乎完全一致。

**方案**: 提取 `async fn webview_login_flow(app, config, password, visible) -> Result<WebviewWindow>`，两函数只保留最后差异部分（点击打卡 vs 高亮按钮）。

---

### 2.4 Prototype 辅助函数合并 [Rust]

**现状**: 4 对函数只差一层子目录查找：
- `collect_scales_for_sequence` / `collect_scales_for_proto_sequence` (L1054-1106)
- `collect_scales_for_image` / `collect_scales_for_proto_image` (L1108-1154)
- `find_file_in_subdirs` / `find_file_in_proto_subdirs` (L971-996, L1236-1258)
- `find_webp_in_subdirs` / `find_webp_in_proto_subdirs` (L998-1026, L1261-1285)

**方案**: 每对合并为一个函数，增加 `sub_name: Option<&str>` 参数。

**预计减少**: ~320 行

---

### 2.5 提取 `src/utils/format.ts` [前端]

**现状**: `formatSize()` 在 FileDetailSidebar (L238-243)、MaterialCard (L30-34)、TaskCard (L83-88) 重复 3 次。`normalizeDeadline()` 在 CreateProjectDialog (L26-40)、EditProjectDialog (L39-50) 重复 2 次。

**方案**: 新建 `src/utils/format.ts` 导出两个函数，各处改为 import。

---

### 2.6 提取 `src/config/fileTypes.ts` [前端]

**现状**: FileDetailSidebar (L168-172) 用 Array 定义 IMAGE_EXTS/VIDEO_EXTS/TEXT_EXTS/PSD_EXTS/PDF_EXTS，NormalCard (L22-25) 用 Set 定义同名常量但 IMAGE_EXTS 多了 svg/ico。

**方案**: 统一为 Set，定义在 `src/config/fileTypes.ts`。**需确认**: svg/ico 是否应出现在 FileDetailSidebar 的图片判断中。

---

### 2.7 提取 `src/types/` 共享接口 [前端]

**现状**: `PreviewVideoEntry`、`MaterialVersion`、`GlobalTask`/`GlobalTaskConfig` 等接口在 TaskPage (L17-57) 和 TaskListPage (L11-37) 重复定义。

**方案**: 新建 `src/types/task.ts` + `src/types/material.ts`，各处改为 import。

---

### 2.8 提取 `.view-btn` / `.sort-tab` 公共样式 [前端]

**现状**: TaskPage、MaterialsPage、GameIntroPage、ProjectPage、HomePage 各自定义了相同的 `.view-buttons`/`.view-btn`/`.sort-tabs`/`.sort-tab` 样式（约 20 行/处）。

**方案**: 移入 `design-system.css`，各页面删除重复定义。

**预计减少**: ~100 行 CSS

---

### 2.9 笔记相关函数封装 [前端]

**现状**: 5 个页面 (HomePage, ProjectPage, MaterialsPage, GameIntroPage, TaskPage) 各自定义了 `onPageNoteCheckbox`、`onPageNoteSave`、`onPageNoteUpdate`，逻辑完全相同。

**方案**: 在 `useNotes` composable 中增加 `usePageNote(dirPath, key)` 返回 `{ showNote, noteText, onCheckbox, onSave, onUpdate }`。

**预计减少**: ~80 行

---

### 2.10 PinboardCanvas 重绘节流 [前端/性能]

**现状**: `PinboardCanvas.vue` L413-414 两个 watch 监听 viewport 和 canvasAnnotations，滚轮缩放时每次 viewport 变化同步重绘。

**方案**: 用 `requestAnimationFrame` 节流 `renderCanvasAnnotations`。

---

### 2.11 NormalCard 视频缩略图懒加载 [前端/性能]

**现状**: `NormalCard.vue` L44-66 在 onMounted 中同步创建 video 元素 seek 首帧，大量视频文件时并发解码。

**方案**: 改为 IntersectionObserver 策略，进入视口后再创建 video 元素。

---

### 2.12 PinboardPage undo 栈限深 [前端/性能]

**现状**: `PinboardPage.vue` L133-134 undoStacks/redoStacks 无上限，每次操作都 JSON 深拷贝全部 annotations。

**方案**: 限制最大 50 步，超出丢弃最旧记录。

---

### 2.13 FileDetailSidebar resize 事件清理 [前端/内存]

**现状**: `FileDetailSidebar.vue` L371-391 startResize() 注册 window mousemove/mouseup，但 onBeforeUnmount 未清理。路由切换时组件卸载可能泄漏。

**方案**: 在 onBeforeUnmount 中补充 removeEventListener。

---

### 2.14 NoteDialog isEditMode 非响应式 Bug [前端/Bug]

**现状**: `NoteDialog.vue` L39-42 `isEditMode()` 是函数而非 computed，模板中 `v-if="isEditMode()"` 不会响应 NoteEditor 内部状态变化。

**方案**: 改为 `computed(() => editorRef.value?.mode === 'edit')`。

---

### 2.15 save_attendance_record 静默失败 [Rust/Bug]

**现状**: `commands.rs` L3470-3474 `let _ = fs::write(path, json)` 写入打卡记录失败被吞。

**方案**: 改为 `if let Err(e) = fs::write(...) { eprintln!("打卡记录写入失败: {e}"); }`

---

### 2.16 WebView eval() 返回值被忽略 [Rust/健壮性]

**现状**: `commands.rs` 打卡流程中 ~15 处 `let _ = webview_window.eval(...)` 忽略返回值。

**方案**: 关键步骤（填写密码、点击登录、点击打卡）改为 `eval(...).map_err(|e| format!("WebView 操作失败: {e}"))?`。

---

### 2.17 密码 JS 注入风险 [Rust/安全]

**现状**: `commands.rs` L3610-3630 密码通过 `format!` 拼接到 JS 字符串，仅替换了 `'` 和 `"`。反斜杠 `\` 或换行符会导致 JS 语法错误。

**方案**: 使用 `serde_json::to_string(&password)` 序列化为 JSON 字符串字面量（自动处理所有转义）。

---

### 2.18 usePinboard Array.from(rgba) 大数组 [前端/性能]

**现状**: `usePinboard.ts` L109 `Array.from(rgba)` 将 Uint8Array 转为普通 Array。600x600 图片 → 1,440,000 元素。

**方案**: 确认 Tauri 2.x 是否支持直接传 Uint8Array；若不支持，改用 base64 编码传输。

---

## Tier 3 — 低优先级（代码卫生）

| # | 优化项 | 涉及文件 |
|---|--------|---------|
| 3.1 | 提取 `openPinboard` 封装 | HomePage, ProjectPage, MaterialsPage, GameIntroPage, TaskPage |
| 3.2 | `.card-menu-btn`/`.priority-dot` 移入 design-system.css | ProjectCard, TaskCard |
| 3.3 | SVG 图标组件化（文件夹/笔记/全屏） | 跨 15+ 处 |
| 3.4 | SettingsPage 按 Tab 拆分 5 个子组件 | SettingsPage |
| 3.5 | rename/delete_material 的 dirs_to_scan 去重 | commands.rs L5362-5510 |
| 3.6 | 删除 scroll_to_bottom_via_wheel 死代码 | commands.rs L6398-6437 |
| 3.7 | 硬编码颜色 → CSS 变量 | TaskPage L2209 |
| 3.8 | `JSON.parse(JSON.stringify())` → `structuredClone()` | useSettings.ts L73 |
| 3.9 | FileDetailSidebar 全局样式加 `fds-` 前缀 | FileDetailSidebar.vue |
| 3.10 | DragMaterialRequest/CopyMaterialRequest 合并 | models.rs L186-198 |
| 3.11 | PinboardCanvas onOverlayMouseMove 空函数删除 | PinboardCanvas.vue L255-257 |
| 3.12 | lib.rs/scheduler.rs 时间解析函数去重 | lib.rs L240-258, scheduler.rs L206-229 |

---

## 执行顺序建议

```
Phase 1: 基础设施（新建共享模块，不改现有逻辑）
  → 1.2 dialog.css
  → 2.5 utils/format.ts
  → 2.6 config/fileTypes.ts
  → 2.7 types/*.ts
  → 2.8 .view-btn/.sort-tab 公共样式

Phase 2: Composable 提取（提取逻辑，各页面改为 import）
  → 1.3 useDragIntent
  → 1.4 useMultiSelect
  → 1.5 useAnnotationDrawing + canvasRenderer
  → 2.9 usePageNote

Phase 3: 组件拆分（大文件瘦身）
  → 1.6 TaskPage 拆分
  → 1.7 FileDetailSidebar 拆分
  → 1.8 控制面板统一

Phase 4: Rust 后端重构
  → 1.1 commands.rs 拆分
  → 2.1 scan_materials 预读优化
  → 2.3 打卡登录流程去重
  → 2.4 Prototype 函数合并

Phase 5: Bug 修复 + 性能 + 代码卫生
  → 2.2 base_name 误匹配 Bug
  → 2.14 NoteDialog Bug
  → 2.15 静默失败
  → 2.16-2.17 WebView 健壮性
  → 2.10-2.13 性能项
  → Tier 3 全部
```

---

**最后更新**: 2026-04-02
