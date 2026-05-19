# Composables 详情

> `src/composables/` 下 22 个组合式函数的关键导出、状态管理、架构决策。
> 顶层索引见 [CODE_INDEX.md](../../CODE_INDEX.md#4-composables22)。

---

## 数据加载三件套

### useProjects.ts（50 行）

`loadProjects()` → 调用 `scan_projects` → `projects[]`。

**ProjectInfo 接口字段**：`completed_subtasks`, `upload_prompted_tasks`, `completed_tasks`, `default_ae_file`, `app_icon`, `note`, `priority`

### useTasks.ts（43 行）

`loadTasks(projectPath)` → 调用 `scan_tasks` → `tasks[]`。

**TaskInfo 接口字段**：`material_total`, `material_uploaded`, `video_total`, `video_uploaded`, `note`, `priority`

### useMaterials.ts（40 行）

`loadMaterials(taskPath)` → 调用 `scan_materials` → `materials[]`。

**MaterialInfo 接口字段**：`scales`（比例列表）, `fps`（序列帧帧率，转换前 null）

### useDirectoryFiles.ts（41 行）

`loadFiles()`, `openInExplorer()` — 通用一层目录扫描 + 打开文件管理器。

`FileEntry` 含可选 `thumbnail_path?: string`（PSD/PSB 磁盘缓存命中时由 `scan_directory` 填入）。

---

## 模块级单例（全局状态）

### useNavigation.ts（75 行）

**职责**：导航状态管理（模块级单例），驱动 TitleBar。

**关键导出**：`setNavigation()`, `goBack()`, `routeDirection`, `setRouteDirection()`

**NavAction 字段**：
- `onLongPress(btnRect: DOMRect)`：长按 500ms 回调，接收按钮位置用于锚定下拉面板
- `active`：全亮强调（蓝色背景 + 描边）
- `hint`：弱描边强调（只 inset box-shadow 不填色，与 active 互斥，用于"有活但非优先下一步"）
- `variant`：`'default' | 'success'`（success = 绿色玻璃，用于「结束加班」按钮）

**路由方向**：`routeDirection` ref + `setRouteDirection()` 方法，供 TitleBar 和 MainLayout 消费实现方向感知动画。

### useTheme.ts（30 行）

`initTheme()`, `toggleTheme()` — 明暗主题切换，localStorage 持久化。

### useScale.ts（28 行）

`initScale()`, `setManualScale()` — 全局 UI 缩放单例。

**基准**：1920px，clamp [0.67, 1.25]，同步缩放 `#app` + `body`（覆盖 Teleport 元素）。支持用户手动覆盖（0 = 自动）。

**默认值已改为 1.0**（原自动模式基于 `window.innerWidth` 宽屏会超 1.0 被夹到 1.25）。

### useStatusBar.ts（504 行）

**职责**：状态栏数据单例（模块级 + refCount）。分钟级 tick，供 StatusBar 组件消费。

**数据源**：
- 打卡时间：`load_attendance_config` 读上下班 + 午休时间
- 节假日：`CalendarRegion`（auto/CN/JP/none），auto 走 `fetch_ip_country`（Rust 代理 ipapi.co，7 天缓存），CN 走 `fetch_cn_holiday_type`（timor.tech，含调休），其他走 `fetch_nager_holidays`（date.nager.at，按年缓存）
- 打卡状态：每分钟 tick 调 `load_attendance_record`，`hasClockIn` / `hasClockOut` 驱动胶囊显隐
- 加班状态：`isOvertime`（localStorage `pgb1-overtime-active` 持久化）

**番茄钟状态机**：5 阶段 `idle → work → work-done → break → break-done → idle`，秒级倒计时，归零发系统通知。

**配置项（localStorage）**：`showPomodoro` / `pomodoroWork`（默认 25m）/ `pomodoroBreak`（默认 5m）/ `calendarRegion`（默认 auto）

**关键架构决策（Sprint 3·Y-20 audit）**：顶部 8 行注释说明为什么保留 refCount 模式而非重构 provide/inject ——
1. StatusBar.vue 嵌在 TitleBar 无 v-if，`refCount ≥ 1` 恒成立
2. Y-3 已修 `pendingAlignTimeout` 跟踪
3. 对齐 timeout 回调 `refCount === 0` short-circuit
三重保险，重构 ROI 负。

**暴露**：`timeStr` / `dateStr` / `holidayLabel` / `hasClockIn` / `hasClockOut` / `workedMinutes` / `countdownMinutes` / `isLunch` / `toLunchMinutes` / `lunchLeftMinutes` / `formatMinutes` / `pomodoroPhase` / `pomodoroDisplay` / `onPomodoroClick` / `reloadHoliday` / `isOvertime` / `startOvertime` / `endOvertime`

---

## 应用设置与偏好

### useSettings.ts（126 行）

**职责**：应用设置 CRUD（读 `load_settings` / 写 `save_settings`）。

**关键防火手记**：`newSettings` 是 Vue reactive Proxy，含 `__v_isRef` 等 Symbol key——必须先 JSON 深拷贝脱壳成纯对象，否则 Tauri IPC / `structuredClone` 都会抛 `DataCloneError`。

### useUpdater.ts（127 行）

自动更新检查/下载/安装。暴露：`updateAvailable` / `updateInfo` / `downloading` / `progress` / `installUpdate` / `skipVersion` / `dismiss`。被 UpdateDialog 和 AboutSettings 消费。

---

## 笔记系统

### useNotes.ts（168 行）

**关键导出**：
- `useNotes(dirPath)`：返回 `{ notes, loading, loadNotes, getNote, hasNote, hoverPreview, previewProgress, saveNote }`
- `usePageNote(getNote, saveNote, noteKey)`：页面级笔记快捷封装（返回 `pageNote`/`isExpanded`/`savePageNote`）
- `stripMarkdown(text)`：剥离 `[text](url)` 命名链接（→text）、`**`/`*`/checklist 前缀（保留裸 URL）
- `toggleCheckbox(text, lineIndex)`：切换指定行 `[ ]` ↔ `[x]`

**常量**：`NOTE_PREVIEW_LIMIT = 39`

**行为**：`saveNote` 乐观更新 + 失败回滚。接受 `Ref<string>` 或 `string` 参数。

---

## 贴图板与 PDF 翻译

### usePinboard.ts（205 行）

**签名**：`usePinboard(dirPath, canvasKey)`

**职责**：管理 `pins` / `canvasAnnotations` / `viewport` 状态。

**关键方法**：
- `loadPinboard` → invoke `get_pinboard`
- `savePinboard` → invoke `save_pinboard`（含 pins + viewport + canvasAnnotations）
- `pasteImage(viewportCenter?)` → clipboard readImage → RGBA → invoke `save_pin_image` → 写 `.pgb1_pins/{id}.png`
  - `viewportCenter` 有值时贴图居中于该点，无值时 fallback 随机偏移
- `deletePin` → invoke `delete_pin_image` + 移除 pin
- `getPinImageUrl` 用 `convertFileSrc` 构建 asset URL
- `bringToFront` 调整 zIndex

接受 `Ref<string> | string` 参数。

### usePdfTranslate.ts（245 行）

**签名**：`usePdfTranslate(filePath)`

**职责**：PDF 翻译全局状态。模块级 `Map<filePath, PdfTranslateSession>` 管理翻译会话（组件卸载不中断翻译）。

**Session 字段**：`state` / `progress` / `error` / `outputPath` / `showingTranslated` / `retryInfo`

**startTranslation 流程**：`loadSettings` → `extract_pdf_pages_text` → 逐页 `translate_text_once`（跳过空白页）→ `build_translated_pdf`

**重试监听**：模块级 `listen('pdf-translate-retry')` 更新重试状态。

**自动检测**：`checkExisting` 首次访问时调 `check_translated_pdf_exists` 自动检测 `_zh.pdf`。

**返回**：`state` / `progress` / `error` / `activePdfSrc` / `start` / `toggleView` / `reset`

---

## 缓存与图像

### useFrameCache.ts（57 行）

`loadSequenceFrames()` — 序列帧 LRU 缓存（max 10 序列 / 120 帧）。

### usePsdThumbnail.ts（41 行）

**关键导出**：
- `getPsdThumbnail(path, maxSize)`：PSD 缩略图模块级缓存。key = `path@maxSize`，并发去重（同一 key 只发一个 invoke）。调用 `extract_psd_thumbnail`，返回 `convertFileSrc(cachePath)` asset URL
- `invalidatePsdCache(path, maxSize)`：清除指定 key 的 JS 缓存，在 Rust 磁盘缓存不命中时调用（防止同 session 内文件被修改后 JS 缓存返回旧图）

---

## 多选与拖拽

### useRubberBandSelect.ts（103 行）

`useRubberBandSelect()` — 框选多选逻辑。

**流程**：`mousedown`（空白区域）→ `mousemove`（视口矩形 + `data-path` 碰撞）→ `onSelect` 回调。

**防冲突**：`justFinished` ref 屏蔽框选后 `click` 事件。`onContainerScroll` 终止框选防止起点失效。

### useDragIntent.ts（36 行）

`createDragHandler(onDragStart, shouldIgnore?, threshold?)` — 拖拽意图检测。

**逻辑**：`mousedown` → 移动距离超过阈值（默认 5px）→ 触发 `onDragStart` 回调，区分点击与拖拽。

### useMultiSelect.ts（86 行）

`useMultiSelect(options)` — 多选状态封装。

**职责**：统一封装 `isMultiSelect` / `selectedPaths` / `toggleSelectAll` / `togglePath` 等多选逻辑 + 框选集成（`useRubberBandSelect`），多页面复用。

---

## 表单状态机

### useOnboardingForm.ts（250 行）

**签名**：`useOnboardingForm(onComplete)`

**职责**：新手引导表单（Sprint 3·Y-17 从 OnboardingDialog.vue 剥离）。

**4 步向导状态机**：`language` → `project-dir` → `tool-paths` → `attendance`

**行为**：
- `onMounted` 调 `scan_app_shortcuts` 自动补齐 Imagine/TexturePacker CLI/GUI 路径（CLI/GUI 互推）
- `finish()` 合并 settings + attendance 配置写回 + 调用 onComplete 回调
- `canProceed` 计算步骤校验

**暴露**：所有 form state + 导航 + 选择器 + finish

### useShortcutForm.ts（217 行）

**签名**：`useShortcutForm(onSave)`

**职责**：快捷方式表单（Sprint 3·Y-17 从 ShortcutDialog.vue 剥离）。

**核心状态**：`type`（app/folder/web）+ `path` + `name` + `customIconPath`

**行为**：
- `watch(type)` 切换到 app 时 `loadAppList`（`scan_app_shortcuts`）
- `fetchIconPreview` 根据类型调 `extract_exe_icon` / `fetch_favicon`
- `browseCustomIcon` 通过 `copy_icon_to_cache` 缓存用户选的图标
- `handleSave` 组装 payload 回调 `onSave`

---

## TaskPage 抽出的两个 Composable（Sprint 3·Y-2b）

### usePreviewVideos.ts（180 行）

**签名**：`usePreviewVideos({ taskFolderPathRef, nextcloudPreviewPathRef, onAfterUpload? })`

**职责**：预览视频（`03_preview` 目录）逻辑封装。

**6 个 state**：`previewGroups` / `videoThumbnails` / `selectedPreviewVideo` / `selectedPreviewGroup` / `showPreviewUploadConfirm` / `draggedPreviewFile`

**4 个纯函数**：`previewGroupKey` / `extractVersion` / `groupPreviewVideos` / `captureGroupThumbnails`

**4 个方法**：`loadPreviewGroups` / `clearSelection` / `confirmPreviewUpload` / `cancelPreviewUpload`

**2 个 FileEntry 适配 computed**：`selectedPreviewVideoAsFileEntry` / `selectedPreviewGroupVersionsAsFileEntries`

**刻意不包含**：依赖 `useMultiSelect.allPaths` 的 `selectPreviewVideo` / `onPreviewVideoMouseDown`——避免与 useMultiSelect 形成循环依赖，作为薄 wrapper 留在父组件 TaskPage。

### useMaterialSidebar.ts（262 行）

**签名**：`useMaterialSidebar({ taskFolderPathRef, scrollRef, materials, getNote, saveNote, refresh, onPreviewSelectionCleared })`

**职责**：素材侧边栏（选中 / 重命名 / 删除 / 笔记 / 帧率编辑）。

**7 个 state**：`selectedMaterial` / `versions` / `sidebarDialog` / `renameInput` / `sidebarNoteText` / `editingFps` / `fpsInput`

**12 个方法**：`selectMaterial` / `closeSidebar` / `onSidebarNoteSave` / `openRenameDialog` / `openDeleteDialog` / `closeSidebarDialog` / `confirmRename` / `confirmDelete` / `startEditFps` / `cancelEditFps` / `confirmEditFps` / `openTpsFile`

**watch**：`watch(selectedMaterial)` → `sidebarNoteText` 自动同步

**preserveCardPosition（关键防火手记）**：
- Action 前后读取卡片屏幕 Y 坐标计算 delta，补偿 `scroll-content.scrollTop` 消除卡片位置跳变
- **参考点用 `data-path` 精确锁定**（原版用 `.material-card.selected`，在 wasOpen 切换时会查到两张不同卡片导致 scrollTop 被拉飞，closeSidebar 时 `.selected` 消失导致补偿被跳过）

**跨 composable 互斥**：通过 `onPreviewSelectionCleared` 回调闭包延迟求值。

### useArchivedMaterials.ts（45 行，v2.8.13 新增）

**签名**：`useArchivedMaterials(projectPath: () => string)`

**职责**：时光机素材归档数据源，封装后端三命令。

**导出**：
- state：`versions: Ref<ArchivedMaterialVersion[]>` / `loading: Ref<boolean>`
- 方法：`load()` 调 `list_archived_materials`（含后端 60 天懒 GC）、`restore(version)` 调 `restore_archived_material`（拒绝式冲突）、`remove(version)` 调 `delete_archived_material_version`

**调用方**：`TimeMachinePage.vue`
