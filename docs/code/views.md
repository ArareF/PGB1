# 页面详情

> `src/views/` 下 19 个页面的数据流、子组件依赖、核心交互（OvertimePage 已于 2026-06-10 删除）。
> 顶层索引见 [CODE_INDEX.md](../../CODE_INDEX.md#5-页面19)。

---

## 主流程（三级页面）

### HomePage.vue（449 行）

**职责**：项目列表 + 新建 + 排序 + 加班按钮。

**数据流**：
- `loadProjects()`（useProjects）
- `sortedProjects` computed 驱动
- 点击跳转项目页

**UI 布局**：
- 顶部 `page-header` 固定副标题行（`.home-page { height:100%; overflow:hidden }` + `.scroll-content { flex:1; overflow-y:auto }`）
- 副标题旁文件夹按钮：`projectRootDir` ref onMounted 预加载，`v-if` 条件渲染，点击 `openInExplorer`
- `.card-grid`：`grid-template-columns: repeat(auto-fill, minmax(var(--card-*-width), 1fr))`
- `<TransitionGroup name="card">` 交错入场 + `.card-move` FLIP 排序动画

**排序控件**：page-header 右侧三档切换（默认/截止日期/优先度），localStorage 持久化（`pgb1-home-sort`）。

**Sharp Grid**：不再维护主页局部试点开关；通过 `useTheme().theme` 向 ProjectCard 传入 `craft`，深色使用精装卡片，浅色保留原版结构。

**截止日期排序算法**：
- `parseDeadline()` 用 `DATE_PATTERN = /^\d{4}-\d{2}-\d{2}$/` 校验（文字备注如"转交了"视为无日期）
- 排序键：① 优先度（急→高→普→停）② 有效日期在前/文字备注沉底 ③ 完成状态（未完成先）④ 逾期→未到期（近→远）

**笔记系统**：NoteDialog 编辑项目卡片笔记（`card:{name}` key），双事件模型（save 关窗 / update 静默）。`useNotes(projectRootDir)`。

**加班按钮**：监听 `overtime-started` 事件 → `startOvertime()`，`isOvertime=true` 时 `setNavigation` 注入「结束加班」按钮（`variant:'success'` 绿色玻璃），点击 `open_reminder_window('overtime')`。监听 `overtime-ended` / `clock-progress` 成功后 `endOvertime()` 清除按钮。

### ProjectPage.vue（528 行）

**职责**：任务列表 + 快捷功能 + 排序。

**快捷功能**：
- 游戏介绍 → GameIntroPage
- 项目素材 → MaterialsPage
- 打开 AE：单击打最新（或默认）`.aep`，**长按弹锚定式下拉面板**（Teleport to body + `position:fixed` 锚定按钮位置 + 外部点击关闭），选择后 `set_default_ae_file` 持久化
- 任务列表 → TaskListPage（传 `projectPath` + `enabledTasks` query）

**排序控件**：sub-title-bar 右侧两档（默认/优先度），localStorage `pgb1-project-sort`。`PRIORITY_ORDER={high:0, medium:1, low:3}`，null fallback 2。

**TaskCard action 处理**：`onTaskAction` 接收优先度 action，invoke `set_task_priority` 后刷新。

**笔记系统**：NoteDialog 编辑任务卡片笔记（`card:{task_name}` key）；sub-title-bar 旁 `.note-btn` 打开项目卡片笔记（`card:{projectId}` key，与 HomePage 共用）。双 `useNotes`。

### TaskListPage.vue（600 行）

**职责**：任务管理页面（路由版替代弹窗）。

**路由参数**：`route.params.projectId` + `route.query.projectPath` + `route.query.enabledTasks`

**双 Tab**：任务启用 / 模板编辑。确定/取消均返回 ProjectPage。

**v2.8.13 重构**：时光机 Tab 抽离为独立 `TimeMachinePage.vue`，此页面瘦身 -274 行（含 state / logic / template / 样式 + 内嵌 confirm/alert 弹窗）。

### TimeMachinePage.vue（545 行）

**职责**：时光机独立页面（v2.8.13 新增）。

**路由参数**：`route.params.projectId` + `route.query.projectPath`

**双 Tab**：
- 任务归档：复用原 TaskListPage Tab 3 的 `list_archived_tasks` / `restore_archived_task` / `delete_archived_version`
- 素材归档：新接入 `useArchivedMaterials`（list / restore / remove 封装），按 `task_name → base_name → timestamp` 三级分组展示，显示 size + stages 提示

**冲突恢复策略**：后端拒绝式。`restore_archived_material` 遇到目标位置已存在同名文件时返回错误 + 冲突清单，前端用 alert 弹窗展示，用户需先删冲突版本再恢复。

**内嵌 confirm/alert 弹窗**：Teleport to body + glass-strong，`white-space: pre-wrap` 支持后端返回的多行冲突清单。

### TaskPage.vue（1472 行）—— 最大页面

**职责**：素材浏览主页面（树形/名称双视图 + Phase 5a-5d + 预览视频）。

**Sprint 3·Y-2b 重构后结构**：
- 预览视频 state / 纯函数 / 单文件上传 → `usePreviewVideos.ts`
- 素材侧边栏 state / 方法 / preserveCardPosition → `useMaterialSidebar.ts`
- 父组件保留：依赖 `useMultiSelect` 的 `selectPreviewVideo` / `onPreviewVideoMouseDown` wrapper + 混合拖拽 `performDrag` + 上传/规范化/子任务/刷新/导航

**路径 SSOT**：`taskFolderPathRef` / `nextcloudPathRef` / `nextcloudPreviewPathRef` 三个 Ref 统一供 script/template/composable 消费。

**Loading overlay（防火手记）**：
- loading 状态从 `<p v-if="loading">` 改成 main-content 顶部 absolute 浮标 + `<Transition name="fade">`
- 原因：`v-if` 卸载 tree-view 导致 `scroll-content.scrollHeight` 收缩，浏览器钳制 `scrollTop` 到 0，refresh 后滚动位置丢失
- 影响场景：帧率编辑 / 重命名 / 删除 / 上传 / 窗口切换 / 手动刷新

**树形视图分组**：
- 普通任务：按缩放比例分组（原始 / [100] / [70] / [50]）
- Prototype 任务两级分组：先按子分类（symbol/big_win/...），再按缩放比例子分组
- 均用 `section-label` / `group-label` 渲染

**Phase 流**：
- **Phase 5a**：多选 + 拖拽上传 + nextcloud 复制
- **Phase 5b**：规范化（NormalizationDialog）
- **Phase 5c**：缩放（跳转 ScalePage）
- **Phase 5d**：格式转换（跳转 ConvertPage）

**03_preview 预览视频区块**：
- 页面底部，按 baseName 分组（去 `_01/_02` 版本号后缀），每组一张卡片
- 截帧缩略图 + 上传状态标签（已上传 / 待更新 / 未上传） + 版本数
- 点击打开 FileDetailSidebar（版本列表可切换）
- 拖拽导出最新版，拖拽后弹确认弹窗复制到 `nextcloud/preview/`（`_breakdown` 到 `preview/breakdown/`）

**预览视频多选全链路**：
- 选中 key = 最新版本 path（`previewGroupKey(g)`）
- `useMultiSelect.allPaths` 合并素材 path + 预览视频 key
- `cardSelector` 扩展为 `.material-card[data-path], .preview-video-card[data-path]`
- `performDrag` 签名扩展接收 `previewGroupsToDrag`，内部合并素材文件路径 + 预览视频最新版 path
- `confirmUpload` 分流：素材批量 `copy_to_nextcloud`，预览视频循环调 `copy_preview_to_nextcloud`

**三按钮双级高亮**：
- 规范化 / 缩放 / 转换三个 action 按工作流优先级（规范化 > 缩放 > 转换）选出 primary 全亮（`active`），其余有活按钮仅描边（`hint`）
- `hasNormalizeWork` ref（异步 `preview_normalize` 检测）+ `hasScaleWork` / `hasConvertWork` computed
- `watch([三 flag])` 变化时重刷 `updateNavigation`

**笔记系统**：
- MaterialCard 传入 `:has-note` / `:note-preview`
- 自定义侧边栏内嵌 NoteEditor（watch selectedMaterial 切换）
- sub-title-bar 旁 `.note-btn` 打开任务卡片笔记（`card:{taskId}` key）
- 双 `useNotes`（taskFolderPathRef 素材笔记 + projectPathRef 页面笔记）

---

## Phase 5 工作流页面

### NormalizePage.vue（601 行）

**职责**：规范化执行页面（Phase 5b+），取代旧 `NormalizationDialog` 弹窗。

**布局**：素材列表占满 main-content（每素材一行：缩略图 + 名称 + 类型角标 + 操作勾选）；全局开关面板 Teleport 到 `#content-row`。

**数据流**：`scan_normalize_items`（全量盘点 `00_original`，序列帧合并为一项、已命名素材也列出）→ `selections[]` 与 `items[]` 平行 → `execute_normalize_v2`（按内容操作→命名操作顺序执行，发 `normalize-progress`）。

**三操作 + 资格判定**：
- 命名规范化：`needs_rename` 才可选（否则灰显「已规范」）
- 自适应画布：仅 `static && is_png`
- 添加黑底：仅 `static && is_png && is_add_or_screen`（base 按 `_` 切分含 add/screen）

**全局开关**：命名规范化(ON)/自适应画布(OFF)/添加黑底(OFF)/执行前备份(ON)；`watch` 全局开关→批量重置有资格行（批量设置语义，手动覆盖会被下次全局切换重置）。

---

### ScalePage.vue（406 行）

**职责**：素材缩放执行页面（Phase 5c）。

**布局**：控制面板 Teleport 到 `#content-row`，手动 `glass-medium`（无 backdrop-filter，与 main-content 同层兄弟）。

**过滤条件**：`imageMaterials` = `material_type=image && progress!='uploaded' && scales.length===0`（只显示完全未缩放的素材）。

**操作流程**：
1. 用户选中卡片 → 选比例 → 点"应用"标注到 `scaleMap`
2. 执行批量缩放
3. 监听 `scaling-progress` 事件，控制面板底部显示进度条 + "正在缩放 X/Y" + 当前文件名

**多选**：`useRubberBandSelect`（`isEnabled=ref(true)` 始终开启）。

### ConvertPage.vue（737 行）

**职责**：格式转换执行页面（Phase 5d）。

**分区**：静帧默认全选，序列帧需手动标注 FPS 才算「已注释」。

**TP 预设折叠面板**：侧边栏"开始制作"按钮上方，可展开收起，含 `Scale`（f64）和 `WebP Quality`（u32），失焦保存到全局设置。invoke `start_conversion` 时传 `tp_scale` / `tp_webp_quality`。

**事件监听**：`sequence-conversion-failed` / `conversion-organized`

**v2.5.2 进度去重修复**：
- `expectedNames` 白名单 + `organizedNames` Set 去重
- 只统计当前会话选中素材且每名只计一次
- `handleStart` 先调 `stop_conversion` 清理旧会话防事件泄漏
- `onUnmounted` 调 `stop_conversion` 防残留 watcher

**跳过序列帧总数收敛修复**：
- `sequence-conversion-failed` 监听器新增 `failedNames` Set（对称于 `organizedNames`）
- 跳过的序列帧（TP 未点发布）从 `conversionProgress.total` 剔除
- 保证 `current/total` 能收敛到相等，解锁「完成转换」按钮

---

## 辅助浏览页面

### GameIntroPage.vue（448 行）

**职责**：浏览 `00_Game Design & Doc` 目录。

**特殊功能**：
- **游戏原型检测**：mount 时调 `find_game_exe` 递归扫描（Unity / Godot），找到 exe 则顶部导航动态插入「启动原型」按钮
- 快捷功能区「项目素材」按钮跳转 MaterialsPage

**多选 + 拖拽**：`useRubberBandSelect`（`isEnabled=isMultiSelect`） + 多选批量拖拽

**FileDetailSidebar 操作**：传 `allow-actions`，`@rename` → `rename_file` + `loadFiles`，`@delete` → `delete_file` + `loadFiles`

**笔记系统**：NormalCard 传 `:has-note` / `:note-preview`；FileDetailSidebar 传 `:note` / `@save-note`；sub-title-bar 旁 `.note-btn` + 可折叠页面笔记（`page` key）。

### MaterialsPage.vue（726 行）

**职责**：4 个分组素材库，每个分组内按素材系列聚合成卡。

**分组**：`01_Preproduction` / `02_Production` / `03_Render_VFX/VFX/PSD` / `05_Outside`

**素材系列合并（v2.8.18）**：`scan_directory` 结果过 `groupIntoSeries()`（`utils/materialSeries.ts`），
`MaterialGroup.files` → `MaterialGroup.series`。四个分组统一启用同一套规则 —— `02` / `05` 里的文件
（`Snipaste_2025-10-08_14-40-51`、`AP_1`、`ChatGPT Image 2026年7月21日 …`）不含 `_YYMMDD` token，
天然不触发合并，已用真实数据验证零误伤。

- **卡片身份**：`series.key`（归一化基础名）；`data-path` / 多选键走 `series.primary.path`（最新版 PSD）
- **cover vs primary**：缩略图用 `cover`（图片格式优先，jpg 秒出），点击 / 拖出 / 重命名用 `primary`（PSD 优先）
- **全选范围**：每个系列的 `primary`，不是全部文件 —— 合并卡代表最新版，旧版走侧边栏
- **侧边栏**：`selectedSeries`（卡片身份） + `selectedFile`（当前预览的具体版本）双 ref；
  `versions` 传 `flattenVersions()`，`versionLabelOf` 传 `versionLabel()`（日期 + 尾缀）。
  系列版本是**新→旧**，与预览视频的旧→新相反，所以必须覆盖标题，否则「最新版本」标在最旧那行
- **重命名 / 删除**：仍是单文件 `rename_file` / `delete_file`，作用于 `selectedFile`，不做整组改名

**特殊行为**：
- **空目录也渲染分组**（显示"将文件拖入此处"提示），新建项目时可直接拖入；目录不存在时 `import_files` 自动创建
- 多选：`iterateSeries()` 生成器跨 group/subGroup 遍历，全选 / 笔记查找共用
- 分组标题旁文件夹按钮：`.folder-btn` 公共类，每个 group/subGroup 标题后各一个

**笔记系统**：多目录笔记管理 `groupNotesMap: Record<string, Record<string, string>>`，`refreshAll` 加载各分组笔记；
`seriesHasNote` / `seriesNotePreview` 按系列聚合（主文件优先，否则取系列内第一条有笔记的版本，
避免旧版笔记因合并而消失）；`getFileNote` 保持 `?? undefined` 语义 —— 空串要显示笔记编辑区，
不能改成 `||`；页面级笔记 `useNotes(projectPathRef)` key `page:materials`。

---

## 设置页家族

### SettingsPage.vue（590 行）

**职责**：全局设置页父组件（Sprint 3·Y-2a 从 1038 → 590 行，-43%）。

**职责边界**：
- Tab 导航
- `editSettings` 共享状态
- 保存闭环
- 5 个子组件的 `v-show` 切换
- `sidebar-footer` 保存按钮（三 Tab 共用 `handleSave`，attendance 通过 `attendanceRef.value.save()` 调子组件自包含的 save）

**数据流分层**：
- `workflow` / `translation` / `general`：用 `v-model="settingsModel"`（writable computed 包住 `editSettings` 避免 null 问题，`defineModel` 在子组件侧深度修改 `editSettings` 触发父 `watch(deep)` 设 `isDirty`）
- `attendance`：用 `ref="attendanceRef"` + `defineExpose({save, isDirty, isSaving, saved})` 完全自包含
- `about`：无 props 内部消费 `useUpdater()`

**样式穿透**：父组件 `<style scoped>` 的共享 form 样式（`.settings-section` / `.form-group` / `.form-input` 等）全部用 `:deep()` 穿透到子组件 scoped 边界，避免 5 次重复。

**v-show 而非 v-if**：
1. 避免切 Tab 时 AttendanceSettings 卸载丢失未保存字段
2. 消除首次进入 attendance 的冷启动延迟

**语言切换副作用**：子组件只 `locale.value = val` + `saveSettings` + `emit('persisted')`，父组件 `watch(language)` 重新调 `setNavigation` 让 `t('settings.title')` 用新 locale 重求值。

**出勤引导**：`route.query.guide === 'attendance'` 时自动弹出 `settingsAttendance` 专属批注。

### settings/AboutSettings.vue（32 行）

关于 Tab。纯展示：`APP_NAME` / `APP_VERSION` / `APP_DEVELOPER` + `useUpdater` 的检查更新按钮。无 props。

### settings/WorkflowSettings.vue（45 行）

工作流 Tab。`defineModel<AppSettings>` + `pickFile`，三个路径字段（`imaginePath` / `texturePackerCliPath` / `texturePackerGuiPath`） + browse 按钮。

### settings/TranslationSettings.vue（56 行）

翻译 Tab。`defineModel<AppSettings>` 纯表单：API key / 模型下拉（含手动输入）/ 全局快捷键 / 计算器键开关 / 默认语言对。

### settings/GeneralSettings.vue（121 行）

通用 Tab。`defineModel<AppSettings>` + `useTheme` + `useScale` + `useI18n`。项目根目录 / 语言 / 主题 / 默认帧率 / 透明背景 / 开机自启 / UI 缩放档位（75%-150%）。

**关键行为**：语言/缩放是运行时偏好，立即持久化——`onLanguageChange` / `onScaleChange` 直接 `await saveSettings(settings.value)` 后 `emit('persisted')` 让父组件重置 `isDirty`。

### settings/AttendanceSettings.vue（293 行）

日报打卡 Tab。**完全自包含**：
- 18 个出勤专属 state
- `init()` 加载 `load_attendance_config` + `save_attendance_password`
- `save()` 保存（trim + 邮箱校验 + `reschedule_attendance`）
- `handleTestClock` / `handleTestDailyReminder`
- `onMounted` 监听 `clock-test-progress` 事件（`onUnmounted` 清理）
- 2 秒 saved 高亮
- `defineExpose({save, isDirty, isSaving, saved})` 给父组件 `sidebar-footer` 保存按钮使用

**布局**：mode 三段切换（off/auto/record_only） + 四档时间 input + 账号密码（show/hide） + 测试连接按钮。

---

## 独立窗口页面

### ReminderPage.vue（449 行）

**职责**：日报打卡提醒弹窗。

**支持类型**：`clock-in` / `clock-out` / `daily-report` / `overtime` 四种。

**加班流程**：`clock-out` 弹窗点「加班」→ emit `overtime-started` 事件 + 关窗（不再弹出加班时间设置窗口），主页显示「结束加班」按钮。

**漏打退勤检测**：`clock-in` 弹窗 `onMounted` 时检查 localStorage 加班残留或 attendance record 中 `last_clock_in > last_clock_out`，命中则显示橙色警告条「昨天忘记打退勤卡了」+ emit `overtime-ended` 清除加班按钮。

### PinboardPage.vue（910 行）

**职责**：贴图板独立窗口（900×700 WebviewWindow，无装饰 + 透明 + Acrylic 毛玻璃）。

**标签系统**：
- 浏览器式多标签
- 各页面点击贴图板按钮时 invoke `open_pinboard_window` → 已存在窗口 emit `pinboard-open-tab` 事件添加/切换标签，不存在则创建新窗口（URL query params 编码初始标签）
- 关闭最后一个标签自动关闭窗口

**工具栏**：
- 粘贴按钮
- 标注工具（选择/画笔/箭头/矩形/椭圆/文字/橡皮擦）
- 笔刷大小滑块（画笔 1-20 / 橡皮擦 5-50 / 文字 10-48，工具独立记忆）
- 颜色选择（5 色）
- 撤销/重做（Ctrl+Z / Ctrl+Alt+Z / Ctrl+Shift+Z）
- 归位（Home 键/按钮，有贴图时计算包围盒自适应缩放居中）
- 缩放

**窗口拖拽**：标签栏 `data-tauri-drag-region` spacer。

**撤销/重做双栈设计**：选中 pin 时操作 pin 级标注栈，未选中时操作画布级标注栈。切换标签时自动保存当前画布。

### TranslatorPage.vue（409 行）

**职责**：翻译悬浮窗（独立 400×250 WebviewWindow，`always_on_top`）。

**布局**：顶部胶囊拖拽条 + 毛玻璃输入框 + 语言对选择器 [中英/中日/英日] + 翻译/撤回。

**触发**：Ctrl+Enter

**流式翻译**：invoke `translate_text_stream` → listen `translate-chunk` / `translate-done` / `translate-error`，首个 chunk 替换原文、后续 chunk 追加，失败自动恢复原文。

**等待动画**：`isWaiting` 控制 textarea 呼吸透明度（`breathe` keyframes 0.35↔1，1.6s），首个 chunk 到达即停止。

**清理**：`onUnmounted` 清理监听器。
