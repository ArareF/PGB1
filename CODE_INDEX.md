# PGB1 代码索引

> 全量源代码文件职责说明，按目录分组。新会话快速了解代码现状用。
> 最后更新: 2026-02-27（UI 迭代：TransitionGroup FLIP 动画修复、文字对比度提升、打开文件夹按钮下沉副标题栏）

---

## 文件统计

| 目录 | 文件数 | 总行数 | 备注 |
|------|--------|--------|------|
| src/components/ | 19 | ~4800 | UI 组件（新增 OnboardingDialog.vue、PageGuideOverlay.vue） |
| src/composables/ | 9 | ~700 | 逻辑组件（useStatusBar ~430 行，useScale 简化为 ~25 行） |
| src/views/ | 12 | ~5400 | 页面 |
| src/styles/ | 3 | ~790 | CSS 设计系统（新增 --text-2xs / --glass-light-blur / --panel-blur token） |
| src/i18n + src/locales/ | 3 | ~1100 | 国际化：i18n 实例 + zh-CN/en locale 文件（含 onboarding/pageGuide namespace） |
| src/其他 | 8 | ~250 | 入口、路由、配置（含 onboarding.ts）、布局 |
| src-tauri/src/ | 7 | ~6850 | Rust 后端（共 61 个命令） |
| **合计** | **54** | **~17700** | |

---

## 1. 入口与配置

| 文件 | 行数 | 职责 |
|------|------|------|
| `src/main.ts` | 10 | 应用入口，初始化 Vue 3 + Router + i18n，加载样式 |
| `src/App.vue` | ~65 | 根组件，initTheme()，加载 settings 后 initScale(uiScale) + 设置 locale。首次启动检测 `onboarded`，未引导时显示 OnboardingDialog，引导完成后渲染 MainLayout。**引导→设置跳转**：`onOnboardingComplete(mode)` 接收打卡模式，mode !== 'off' 时 `router.push` 到设置页出勤 Tab 并触发出勤配置指引 |
| `src/i18n/index.ts` | 12 | vue-i18n 实例：`legacy:false`, 默认 zh-CN，fallback zh-CN |
| `src/locales/zh-CN.ts` | ~470 | 中文 locale（22 个 namespace，300+ key，含 onboarding 新增 projectDirHint/autoDetected/toolNotFound） |
| `src/locales/en.ts` | ~455 | 英文 locale（结构与 zh-CN 完全对齐） |
| `src/config/app.ts` | 8 | 软件元信息 SSOT：`APP_NAME`、`APP_VERSION`、`APP_DEVELOPER` |
| `src/config/onboarding.ts` | ~90 | 引导数据 SSOT：`PageIntro`/`GuideAnnotation` 接口，`PAGE_INTROS`（9 页介绍，仅被 PageGuideOverlay 的更多菜单消费），`PAGE_GUIDE_ANNOTATIONS`（各页面批注坐标，含 `settingsAttendance` 出勤引导专用批注） |
| `src/router/index.ts` | 52 | 9 条路由：`/` → HomePage, `/project/:id` → ProjectPage, `/project/:id/task/:taskId` → TaskPage, game-intro, materials, **`/project/:id/task-list` → TaskListPage**, `/reminder/:type` → ReminderPage, `/overtime` → OvertimePage, `/translator` → TranslatorPage |
| `src/vite-env.d.ts` | 1 | Vite 类型声明 |

---

## 2. 布局

| 文件 | 行数 | 职责 |
|------|------|------|
| `src/layouts/MainLayout.vue` | ~185 | 主布局：顶部行（TitleBar + WindowControls + 更多菜单）+ 内容行（Sidebar + main-content）。`#content-row` 是侧边栏 Teleport 目标。**动画**：`<Transition name="page-forward/back" mode="out-in">` 路由切换（方向感知）；更多菜单 `<Transition name="dropdown">`。**滚动架构**：`.main-content`（玻璃面板）`overflow-y: hidden`，`.page-wrapper` `height: 100%`——面板不自滚，各页面自己通过 `height:100%; overflow:hidden` + 内部 `.scroll-content { flex:1; overflow-y:auto }` 管理滚动，保证副标题行固定。**更多菜单按钮**：手动 glass-medium（无 backdrop-filter）。**更多菜单下拉**：手动 glass-strong（无 backdrop-filter，未 Teleport to body）。**Hover**：按钮 hover = text-primary + border-heavy + translateY(-2px) + shadow，active 弹回 |

---

## 3. 组件

| 文件 | 行数 | Props | 职责 |
|------|------|-------|------|
| `ProjectCard.vue` | ~490 | `project: ProjectInfo` | 项目卡片（图标+名称+截止日期+进度条）。根元素为 `<div>`（非 `<button>`，避免嵌套）。**AppIcon**：`onMounted` 读 `project.app_icon`，PNG 用 `convertFileSrc`，PSD/PSB 调 `getPsdThumbnail(128px)`，无图标降级为 SVG 占位。进度条计算：无子任务的父任务用 `completed_tasks`，有子任务的父任务用 `completed_subtasks`，分母 = 无子任务父任务数 + 所有子任务数。**Hover 菜单**：右上角 ··· 按钮（opacity 过渡），展开重命名/修改截止日期/删除三项，emit `action` 事件。**菜单 Teleport to body**：避免父级 `glass-subtle` 的 `backdrop-filter` 创建合成层导致子级毛玻璃失效，菜单使用 `glass-medium` 类 + `position: fixed` + 动态坐标（`menuBtnRef.getBoundingClientRect()`），`z-index: var(--z-dropdown)`。菜单样式在全局（非 scoped）`<style>` 块。**动画**：`···` 按钮 hover 出现加 `scale(0.85→1)` 动画；卡片下拉菜单加 `<Transition name="card-menu">`。**优先度圆点**：名称前 `.priority-dot--{high/medium/low}` 9px 纯色实心圆，无文字（null 不显示）；菜单固定展示四档 `急/高/普/停`（'normal'→null），选中态高亮，直接调用 set_project_priority + emit refresh。全局 `<style>` 块含 `.priority-pill--{high/medium/normal/low}` 样式，TaskCard 复用 |
| `TaskCard.vue` | ~265 | `task, subtaskProgress?` | 任务卡片（名称+动态进度标签+大小）。**有子任务**：未开始 0/N（灰）/ 进行中 X/N（黄）/ 已完成（绿）。**无子任务（叶子任务）**：未开始（灰）/ 制作中（蓝，有素材未全上传）/ 已完成（绿），不显示数字。大小取 nextcloud 目录。**右上角 ··· 菜单**：Teleport to body，同款 ProjectCard 菜单机制；仅含优先度选择器（急/高/普/停四档固定展示），emit action(task, 'priority', value\|null) 给 ProjectPage 处理。**优先度圆点**：名称前 `.priority-dot` 9px 实心圆，null 不显示。**单根节点**：`<Teleport>` 移至 `<button>` 内部（Teleport 内容仍挂载到 body，comment 占位无影响），使组件兼容 `<TransitionGroup>` 入场/FLIP 动画 |
| `MaterialCard.vue` | ~245 | `material, multiSelect?, checked?` | 素材卡片（序列帧=SequencePreview, 静帧=img, 进度标签+大小）。序列帧角标显示 fps（转换后才显示，转换前隐藏）。SequencePreview `:key` 绑定 `${path}-${fps}`，fps 变化时强制重挂使动画速度即时更新。fps 和 transparent 从 `useSettings().settings.preview` 读取 |
| `NormalCard.vue` | ~237 | `file: FileEntry` | 普通文件卡片（游戏介绍/项目素材页用）。视频文件 onMounted canvas 截帧；PSD/PSB 文件调用 `usePsdThumbnail`（256px）异步加载真实缩略图，失败降级为 PS 图标；PDF 文件显示红色 PDF 图标；multiSelect?/checked? props + data-path + card-checkbox-shared 多选三件套 |
| `SequencePreview.vue` | ~110 | `folderPath, fps?, maxWidth?, transparent?` | Canvas 序列帧动画播放器，mount 后自动循环播放，LRU 缓存。`transparent=true` 时 clearRect 透明背景 + 棋盘格 CSS，否则黑色背景 |
| `ImageViewer.vue` | ~80 | `src` | 通用可缩放/拖拽图片查看器（滚轮缩放 + 鼠标拖拽），供 TaskPage 侧边栏和 FileDetailSidebar 共用 |
| `FileDetailSidebar.vue` | ~780 | `file: FileEntry \| null, widthPercent?, versions?: FileEntry[]` | **普通文件侧边栏**（游戏介绍/项目素材页/任务页预览视频用）。**手动 glass**：不用 `glass-strong` 类（与 main-content 相邻会触发 backdrop-filter 兄弟冲突），手动 `background/border/box-shadow`。支持图片（ImageViewer）、视频（自定义播放控制条）、TXT（read_text_file）、**PSD/PSB**（`usePsdThumbnail` 800px 高清缩略图 + 「用 Photoshop 打开」按钮）、**PDF**（iframe 直接渲染，WebView2 内置 PDF 引擎）、其他（图标占位）。`open_file` 用系统关联程序打开文件。可选 `versions` prop 传入多版本列表，点击 emit `select-version` 切换播放。**版本历史卡片**：卡片式布局（与 TaskPage 其他版本一致），左列版本标签+文件大小、右侧扩展名+打开文件夹按钮，active 状态高亮。Teleport to #content-row。sidebar 过渡同时动画 `transform + width` |
| `TitleBar.vue` | ~250 | — | 顶部标题栏（返回按钮+标题+快捷功能区），消费 useNavigation()。返回箭头 SVG 40×40。支持 action 长按（500ms，`pointerdown` 计时，`onLongPress(btnRect)` 回调传递按钮 DOMRect）。`active` 属性控制按钮强调样式（蓝色背景+描边）。中间岛集成 **StatusBar**（常驻状态栏）。布局：左侧标题岛 `flex-shrink: 0` 不压缩，右侧功能岛独自承担窄窗口压缩。两岛 `align-items: flex-end` 底部对齐，右侧功能岛高度由内容撑开（比标题岛矮）。有 actions 时状态栏+分隔线+按钮共存，按钮区域支持滚轮横滚。**左岛手动 glass**：不用 `glass-medium` 类（与 center 岛相邻会触发 backdrop-filter 兄弟冲突），手动 `background/border/box-shadow`。**标题文字裁切**：`.title-text-wrap` 包裹层（`overflow:hidden; position:relative`），防止转场动画 leave 态 `position:absolute` 标题文字侵入返回按钮区域。**动画**：JS FLIP 宽度动画（`watch flush:pre/post` + `flipWidth`）；**flipWidth Bug 修复**：读 toWidth 前先清除残留内联样式（`style.width/transition/overflow = ''`）再 `offsetWidth` 强制 layout，防止快速连续导航时旧内联宽度污染 toWidth 导致岛宽卡死；标题/返回按钮/操作区 `<Transition name="nav-forward/back">` 方向感知滑入；**返回按钮 leave 动画修复**：`.nav-back-leave-active.back-btn` 加 `top:0; bottom:0`，防止 `position:absolute` 脱离 flex 后 `align-self:stretch` 失效导致向上跳动；新增 `leftIslandRef`、`centerIslandRef` template ref。**Hover**：`.action-btn` normal 态有微弱阴影（`0 1px 3px`）；`:hover` = `translateY(-2px)` + `--bg-active` + `border-color: --border-medium` + shadow（`0 2px 6px`）；`:active` 弹回 `translateY(0)`。**hover 裁切修复**：`glass-medium` 的 `overflow:hidden` 会裁切 translateY(-2px)，在 `.title-bar-center` 覆盖为 `overflow:visible`；`overflow-x:auto` 强制 `overflow-y:auto` 导致 ink overflow 被裁切，在 `.title-bar-actions` 加 `padding-block:6px` 建立缓冲区 |
| `StatusBar.vue` | ~570 | — | **状态栏组件**（嵌入 TitleBar 中间岛）。左列：时间/日期/节假日标签（短文案）。右列：已工作胶囊（需 `hasClockIn && !hasClockOut`）+ 倒计时胶囊（需 `hasClockIn`，`hasClockOut` 后显示"下班咯"，午休显示`午休 Xm`/`午休中`）。最右：**番茄钟按钮**（无形态纯光晕：`::before` + `filter:blur(16px)` + `isolation:isolate`）——空闲=白色极淡，专注=蓝色，超时=红绿交替动画，休息=绿色，休息结束=绿色呼吸。长按 500ms 弹出配置面板（Teleport to body，`<Transition name="config-panel">` 进出场动画：`translateY(-6px) scale(0.95)` + opacity，`transform-origin: top right`）：5 个 boolean 开关 + 番茄钟时长 + **假日日历地区下拉**（自动/中国/日本/不显示，切换后即时刷新） |
| `Sidebar.vue` | ~310 | — | 左侧快捷方式栏。**手动 glass**：不用 `glass-medium` 类（与 main-content 相邻会触发 backdrop-filter 兄弟冲突），手动 `background/border/box-shadow`。iOS 风格交互：单击启动，长按 500ms 进入编辑模式（图标抖动 + 右上角红色 × 删除徽章），点击空白退出。编辑模式内拖拽重排（`pointermove + elementFromPoint`，实时更新 `displayOrder`）。**拖拽排序动画**：`<TransitionGroup name="sort">` + `.sort-move { transition: transform 200ms }` FLIP 动画，其他图标平滑滑走。添加时自动提取图标（应用=`extract_exe_icon` 256px，网页=`fetch_favicon`）。[+] 固定在底部。**Hover**：`::before` 伪元素蓝色模糊光晕（`filter:blur(14px)` + `isolation:isolate`，`opacity` 0→0.45），无边框无阴影，`translateY(-2px)` 上浮；`:active` = `translateY(0) scale(0.95)`。**编辑模式抖动**：`0.45s linear infinite`，6个不规则关键帧（±3~4deg + 微量 translateY），`nth-child(2n/3n/4n)` 错开相位避免整齐同步 |
| `ShortcutDialog.vue` | ~280 | — (emits: save, cancel) | **快捷方式添加弹窗**（仅添加，无编辑）。应用类型：扫描开始菜单/桌面 `.lnk` 列表 + 搜索过滤 + 手动浏览备用。文件夹：浏览选择。网页：手动输入 URL。名称自动填充。Teleport to body。`show?: boolean` prop 控制内部 `v-if`，完整进出动画（遮罩 opacity + 内容 translateY/scale）。**图标预览区**（表单底部分隔行）：44×44 可点击图标框，未自定义时显示类型占位 SVG，已自定义显示实际图片 + 蓝色边框；悬停出现铅笔遮罩，点击唤起文件选择框（PNG/JPG/ICO/BMP/WEBP）→ 调用 `copy_icon_to_cache` 缓存为 PNG。**"预览自动图标"按钮**：有选中目标时显示，点击在弹窗内预取图标并填入预览（app=`extract_exe_icon`，web=`fetch_favicon`）。`emit.save` 增加 `custom_icon: string \| null`，有值时 Sidebar.vue 跳过重复提取 |
| `UploadConfirmDialog.vue` | 100 | `fileCount` | 上传确认弹窗（拖拽后询问是否已上传到网盘），Teleport to body。`show?: boolean` prop 控制内部 `v-if`，完整进出动画（遮罩 opacity + 内容 translateY/scale） |
| `WindowControls.vue` | 92 | — | 窗口控制按钮（最小化/最大化/关闭） |
| `CreateProjectDialog.vue` | 209 | — (emits: created, cancel) | **新建项目弹窗**（项目名+截止日期输入）。日期标准化（支持 20260616 / 2026-06-16 格式），调用 create_project 命令，Teleport to body。`show?: boolean` prop 控制内部 `v-if`，完整进出动画（遮罩 opacity + 内容 translateY/scale） |
| `EditProjectDialog.vue` | ~210 | `project: ProjectInfo, mode: 'rename'\|'deadline'\|'delete'` (emits: updated, deleted, cancel) | **项目管理弹窗**，通过 mode 复用三种操作。rename：输入框预填项目名，调用 rename_project；deadline：输入框预填截止日期+日期标准化，调用 update_project_deadline；delete：红色确认弹窗，调用 delete_project（移入回收站），Teleport to body。`show?: boolean` prop 控制内部 `v-if`，完整进出动画（遮罩 opacity + 内容 translateY/scale） |
| `AttendanceDialog.vue` | ~210 | — (emits: close) | **日报打卡设置弹窗**（考勤时间+日报时间+URL+账号密码），密码存 Windows Credential Manager，保存后自动调用 reschedule_attendance，Teleport to body |
| `OnboardingDialog.vue` | ~350 | `show: boolean` (emits: complete[mode]) | **首次引导步骤式弹窗**。4 步：语言选择→项目目录→工具路径→打卡模式。**步骤校验**：项目目录必填、工具路径需 Imagine + TP CLI，未填好"下一步"按钮灰化。**工具路径探测**：onMounted 调用 `scan_app_shortcuts` 扫描 Imagine 和 TexturePacker（CLI/GUI 互推）。完成时 emit 携带打卡模式值，App.vue 据此决定是否跳转设置页。Teleport to body |
| `PageGuideOverlay.vue` | ~125 | `show: boolean, annotations: GuideAnnotation[]` (emits: close) | **通用页面指引遮罩**。Teleport to body，全屏半透明遮罩 + fixed 定位批注气泡（支持上下左右箭头），点击任意处关闭。`white-space: pre` 支持 `\n` 手动换行，不自动断行。各页面通过 `PAGE_GUIDE_ANNOTATIONS[pageId]` 传入批注数据 |
| `NormalizationDialog.vue` | ~240 | `taskPath` | **规范化预览弹窗**，扫描并识别静帧（去 _01）与序列帧（归类），展示变更预览，支持一键执行，Teleport to body。`show?: boolean` prop 控制内部 `v-if`，完整进出动画（遮罩 opacity + 内容 translateY/scale） |
| `ConversionDialog.vue` | ~410 | `taskPath, materials` | **格式转换选择弹窗**，分区列出未转换的静帧与序列帧，序列帧强制要求输入帧率，提交后开启后端转换会话，Teleport to body。`show?: boolean` prop 控制内部 `v-if`，完整进出动画（遮罩 opacity + 内容 translateY/scale） |

---

## 4. Composables

| 文件 | 行数 | 关键导出 | 职责 |
|------|------|---------|------|
| `useNavigation.ts` | ~70 | `setNavigation()`, `goBack()`, `routeDirection`, `setRouteDirection()` | 导航状态管理（模块级单例），驱动 TitleBar。NavAction 支持 `onLongPress(btnRect: DOMRect)`（长按回调，接收按钮位置用于锚定下拉面板）和 `active`（强调样式）。新增路由方向状态（`routeDirection` ref + `setRouteDirection()` 方法），供 TitleBar 和 MainLayout 消费实现方向感知动画 |
| `useProjects.ts` | 45 | `loadProjects()` | 调用 `scan_projects` → `projects[]`。ProjectInfo 接口含 completed_subtasks, upload_prompted_tasks, completed_tasks, default_ae_file, **app_icon** |
| `useTasks.ts` | 40 | `loadTasks(projectPath)` | 调用 `scan_tasks` → `tasks[]`。TaskInfo 接口含 material_total, material_uploaded, **video_total, video_uploaded** |
| `useMaterials.ts` | ~45 | `loadMaterials(taskPath)` | 调用 `scan_materials` → `materials[]`。MaterialInfo 接口含 scales（比例列表）、fps（序列帧帧率，转换前 null） |
| `useDirectoryFiles.ts` | 39 | `loadFiles()`, `openInExplorer()` | 通用目录扫描 + 打开文件管理器 |
| `useFrameCache.ts` | 57 | `loadSequenceFrames()` | 序列帧 LRU 缓存（max 10 序列, 120 帧） |
| `useTheme.ts` | 30 | `initTheme()`, `toggleTheme()` | 明暗主题切换，localStorage 持久化 |
| `useScale.ts` | ~60 | `initScale()`, `setManualScale()` | 全局 UI 缩放单例。基准 1920px，clamp [0.67, 1.25]，同步缩放 #app + body（覆盖 Teleport 元素）。支持用户手动覆盖（0 = 自动）。**注意**：自动模式基于 `window.innerWidth`，宽屏（>1920px）会超出 1.0 被夹到 1.25，窄窗口会偏小；默认值已改为 1.0 |
| `usePsdThumbnail.ts` | ~35 | `getPsdThumbnail(path, maxSize)` | PSD 缩略图模块级缓存。key = `path@maxSize@mtime`，文件修改后自动失效。并发去重（同一 key 只发一个 invoke）。调用 `extract_psd_thumbnail` + `get_file_mtime` |
| `useStatusBar.ts` | ~420 | `useStatusBar()`, `saveConfig()`, `reloadConfig()` | 状态栏数据单例。分钟级 tick，从 `load_attendance_config` 读上下班+午休时间。**节假日**：`CalendarRegion`（auto/CN/JP/none），auto 模式用 `ipapi.co/country/` 检测 IP（7天缓存），CN 走 timor.tech（含调休概念），其他国家走 date.nager.at（按年缓存）；标签简短：`休息日 🎉`/`调休`/`明天休 🎉`。**打卡状态感知**：每分钟 tick 调 `load_attendance_record`，`hasClockIn`/`hasClockOut` 驱动胶囊显隐（未打卡不显示，打下班卡触发"下班咯"）。**番茄钟**：5 阶段状态机（idle→work→work-done→break→break-done→idle），秒级倒计时，归零发系统通知。配置项（localStorage）：`showPomodoro`/`pomodoroWork`（25m）/`pomodoroBreak`（5m）/`calendarRegion`（auto）。暴露：`timeStr`/`dateStr`/`holidayLabel`/`hasClockIn`/`hasClockOut`/`workedMinutes`/`countdownMinutes`/`isLunch`/`toLunchMinutes`/`lunchLeftMinutes`/`formatMinutes`/`pomodoroPhase`/`pomodoroDisplay`/`onPomodoroClick`/`reloadHoliday` |
| `useRubberBandSelect.ts` | ~75 | `useRubberBandSelect()` | 框选多选逻辑。mousedown（空白区域）→ mousemove（视口矩形 + data-path 碰撞）→ onSelect 回调。justFinished ref 屏蔽框选后 click 事件。onContainerScroll 终止框选防止起点失效 |

---

## 5. 页面

| 文件 | 行数 | 复杂度 | 职责 |
|------|------|--------|------|
| `HomePage.vue` | ~260 | 低 | 项目列表 + [+] 新建项目按钮 + CreateProjectDialog 集成，点击跳转项目页。接收 ProjectCard 的 action 事件，控制 EditProjectDialog 的显示和 mode，操作完成后调用 loadProjects 刷新。`.card-grid` 改为 `display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--card-*-width), 1fr))`；`<TransitionGroup name="card">` 卡片交错入场 + `.card-move` FLIP 排序动画。**布局**：`.home-page { height:100%; overflow:hidden }` + `.page-header`（固定副标题行）+ `.scroll-content { flex:1; overflow-y:auto }`（与其他页面统一滚动架构）。**副标题旁文件夹按钮**：`projectRootDir` ref 在 onMounted 从 settings 预加载，按钮 `v-if="projectRootDir"` 条件渲染，点击 `openInExplorer`；更多菜单已移除「打开项目文件夹」。**排序控件**：page-header 右侧三档切换（默认/截止日期/优先度），选择结果 localStorage 持久化（`pgb1-home-sort`），`sortedProjects` computed 驱动。**截止日期排序**：`parseDeadline()` 用 `DATE_PATTERN=/^\d{4}-\d{2}-\d{2}$/` 校验（文字备注如"转交了"视为无日期）；排序键：① 优先度（急→高→普→停）② 有效日期在前/文字备注沉底 ③ 完成状态（未完成先）④ 逾期→未到期（近→远）。**优先度排序**：单键按优先度+名称字母序。处理 ProjectCard @refresh 事件重载列表 |
| `ProjectPage.vue` | ~450 | 中 | 任务列表 + 快捷功能（游戏介绍/项目素材/打开AE/任务列表）。「打开AE」单击打最新（或默认）.aep，**长按弹出锚定式下拉面板**（Teleport to body，`position:fixed` 锚定按钮位置，外部点击关闭，与 StatusBar 配置面板同款风格），选择后设为默认并打开（持久化到 .pgb1_project.json），有默认时按钮蓝色强调。计算子任务进度传给 TaskCard。「任务列表」跳转 TaskListPage（传 projectPath + enabledTasks query）。`.card-grid` 改为 `display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--card-*-width), 1fr))`；`<TransitionGroup name="card">` 卡片交错入场 + `.card-move` FLIP 排序动画。**副标题旁文件夹按钮**：`.folder-btn` 图标按钮紧跟副标题，点击 `openInExplorer(projectPath)`；更多菜单已移除「打开项目文件夹」。**排序控件**：sub-title-bar 右侧两档切换（默认/优先度），选择结果 localStorage 持久化（`pgb1-project-sort`）；`PRIORITY_ORDER={high:0,medium:1,low:3}`，null fallback 2（急→高→普→停）；onTaskAction 接收 TaskCard action 事件，invoke set_task_priority 后刷新 |
| `TaskListPage.vue` | ~270 | 中 | **任务管理页面**（路由页面版，替代弹窗）。通过 `route.params.projectId` + `route.query.projectPath/enabledTasks` 接收参数。三 Tab：任务启用/模板编辑/时光机。确定/取消均返回 ProjectPage |
| `TaskPage.vue` | ~1900 | **高** | 素材浏览主页面。**树形视图分组**：普通任务按缩放比例分组（原始/[100]/[70]/[50]）；**Prototype 任务两级分组**：先按子分类（symbol/big_win/…），再按缩放比例子分组（原始/[100]/…），均用 section-label/group-label 渲染。名称视图平铺。**Phase 5a**：多选+拖拽上传+nextcloud 复制。**Phase 5b**：规范化。**Phase 5c**：缩放。**Phase 5d**：格式转换。**侧边栏**（手动 glass，不用 `glass-strong`，避免 backdrop-filter 兄弟冲突）：通用（重命名/删除）；序列帧专属：帧率行内联编辑 + [修改] 按钮；底部 `.sidebar-action-btn` 无 backdrop-filter。**sidebar-dialog**（重命名/删除弹窗）：手动 glass-strong（在 Teleport 到 #content-row 的侧边栏内，与 main-content 同层）。**03_preview 预览视频区块**：页面底部，按 baseName 分组（去 _01/_02 版本号后缀），每组一张卡片，截帧缩略图+上传状态标签（已上传/待更新/未上传）+版本数，点击打开 FileDetailSidebar（版本列表可切换），拖拽导出最新版，拖拽后弹确认弹窗复制到 nextcloud/preview/（breakdown 到 preview/breakdown/）。sidebar 过渡同时动画 `transform + width` 消除主内容区突变；useRubberBandSelect 集成（isEnabled=isMultiSelect） |
| `ScalePage.vue` | ~465 | **中** | **素材缩放执行页面**（Phase 5c）。控制面板 Teleport 到 #content-row，手动 glass-medium（无 backdrop-filter，与 main-content 同层兄弟）；useRubberBandSelect（isEnabled=ref(true)，始终开启）。`imageMaterials` 过滤条件：`material_type=image && progress!='uploaded' && scales.length===0`（只显示完全未缩放的素材）。缩放比例是标注器：用户选中卡片 → 选比例 → 点"应用"标注到 scaleMap → 执行批量缩放 |
| `ConvertPage.vue` | ~780 | **中** | **格式转换执行页面**（Phase 5d）。控制面板 Teleport 到 #content-row，手动 glass-medium（无 backdrop-filter，与 main-content 同层兄弟）。静帧默认全选，序列帧需手动标注 FPS 才算「已注释」。**TP 预设折叠面板**：侧边栏"开始制作"按钮上方，可展开收起，包含 Scale（f64）和 WebP Quality（u32）输入框，失焦时保存到全局设置。invoke `start_conversion` 时传 `tp_scale` / `tp_webp_quality`。监听 `sequence-conversion-failed` / `conversion-organized` 事件；useRubberBandSelect（isEnabled=ref(true)，始终开启） |
| `GameIntroPage.vue` | ~230 | 低 | 浏览 00_Game Design & Doc 目录，支持 FileDetailSidebar。`.card-grid` 改为 `display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--card-*-width), 1fr))`；`<TransitionGroup name="card">` 卡片交错入场；多选开关 + 全选 + useRubberBandSelect（isEnabled=isMultiSelect）+ 多选批量拖拽。**副标题旁文件夹按钮**：`.folder-btn` 图标按钮紧跟副标题，点击 `openInExplorer(dirPath)`；TitleBar actions 已移除「打开文件夹」（原在快捷功能区）。**游戏原型检测**：mount 时调用 `find_game_exe` 递归扫描（Unity / Godot），找到 exe 则顶部导航动态插入「启动原型」按钮（`refreshNav()` 模式） |
| `MaterialsPage.vue` | ~250 | 中 | 4 个分组素材库（01_Preproduction / 02_Production / 03_Render_VFX/VFX/PSD / 05_Outside）。**空目录也渲染分组**（显示"将文件拖入此处"提示），新建项目时可直接拖入；目录不存在时 import_files 自动创建。`.card-grid` 改为 `display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--card-*-width), 1fr))`；`<TransitionGroup name="card">` 卡片交错入场；多选开关 + 全选（跨 group/subGroup 收集 allFiles）+ useRubberBandSelect。**分组标题旁文件夹按钮**：`.folder-btn` 公共类（已迁入 design-system.css），每个 group/subGroup 标题后各一个 |
| `SettingsPage.vue` | ~1010 | **高** | **全局设置页面**。5 Tab 导航（工作流、翻译、日报打卡、通用设置、关于）。内置本地编辑副本 `editSettings`。**出勤引导**：`route.query.guide === 'attendance'` 时自动弹出 `settingsAttendance` 专属批注（新手引导跳转触发）。**开机自启修复**：`save_settings` 中 `autolaunch.disable()` 前先 `is_enabled()` 检查，避免条目不存在时 OS error 2。 |
| `ReminderPage.vue` | ~260 | 中 | **日报打卡提醒弹窗**，支持 clock-in/clock-out/daily-report/overtime 四种类型 |
| `OvertimePage.vue` | ~140 | 低 | **加班时间设置弹窗**（快捷按钮 +30分/+1小时/+2小时 + 自定义输入） |
| `TranslatorPage.vue` | ~250 | 低 | **翻译悬浮窗**（独立 400×500 WebviewWindow，always_on_top）。顶部胶囊拖拽条 + 毛玻璃输入框 + 语言对选择器[中英/中日/英日] + 翻译/撤回。Ctrl+Enter 触发。调用 `translate_text` → Gemini API |

---

## 6. 样式

| 文件 | 行数 | 职责 |
|------|------|------|
| `reset.css` | 45 | 基础重置，字体引用 `var(--font-family-base)`，根字号 14px |
| `design-system.css` | ~830 | **SSOT**：颜色（冷科技蓝色板+冷蓝灰中性色）、间距、排版（URW DIN + 更纱黑体）、圆角（工业风收窄）、卡片、标签、过渡、毛玻璃变量。暗色主题 v2.0 冷色工业终端风格。`.section-label`+`.group-label` 标题样式。**新增 token**：`--overlay-backdrop`（弹窗遮罩）、`--canvas-bg`（Canvas 背景）。弱化 Material 阴影，改用透明度+冷蓝边框拉层级。**Hover SSOT**：`--shadow-card-hover` 含 ring 光晕（`0 0 0 1px rgba(100,180,255,0.30)`）覆盖所有卡片；`--card-hover-lift` = -3px。**优先度 token**：菜单胶囊用 `--priority-{h/m/l}-{bg/text/active}`（半透明）；卡片圆点用 `--priority-{high/medium/low}-dot`（= color-danger/warning/success 纯实色）。**深色模式 `--text-tertiary`**：`#6B6E77`（原 `#4A4D54` 对比度仅 2.2:1，已提升至 3.8:1）。**TransitionGroup FLIP**：`.card-move { transition: transform ... }` 使所有 `name="card"` 分组在排序时平滑位移。**公共类 `.folder-btn`**：28×28 图标按钮，透明背景，hover 蓝色 wash，供各页面副标题旁打开文件夹按钮复用 |
| `glass.css` | ~75 | 毛玻璃工具类：`.glass-subtle`, `.glass-medium`, `.glass-strong`。**backdrop-filter 兄弟冲突规则**（顶部注释）：同层 flex 兄弟只能一个带 backdrop-filter，其余手动 bg/border/shadow。`overflow: clip`（非 hidden）；`::after` 噪点 `z-index: -1`，子元素不强制 `z-index` |

---

## 7. Rust 后端

### 文件概览

| 文件 | 行数 | 职责 |
|------|------|------|
| `main.rs` | 6 | 应用入口 |
| `lib.rs` | ~230 | Tauri 初始化、命令注册（61 个）、插件注册（opener/drag/dialog/clipboard/notification/**autostart**）、Windows Acrylic 毛玻璃、调度器初始化 + 补打检测、hotkey 全局快捷键初始化、**启动时同步 autolaunch 状态** |
| `models.rs` | ~495 | 数据模型（24 个 struct + 3 个 enum）。ProjectConfig 新增 default_ae_file 字段。新增 PreviewVideoEntry（含 upload_status）。新增 **PreviewSettings**（default_fps/background_transparent），AppSettings 加 preview 字段。`GeneralSettings.ui_scale` 默认值 `1.0`（首次运行不再使用自动缩放）。`GeneralSettings.auto_start: bool`（开机自启，默认 false）。**WorkflowSettings** 新增 `tp_scale: f64`（默认 0.5）/ `tp_webp_quality: u32`（默认 80）。**StartConversionRequest** 新增同名字段 |
| `commands.rs` | ~5830 | 61 个命令实现 + 辅助函数（含 regex_strip_version、**send_ctrl_end**）。psd + base64 依赖。**`copy_icon_to_cache`**、**`collect_scales_for_proto_sequence`**（Prototype 序列帧专用 scale 收集）、**`find_game_exe`**（递归游戏原型检测，支持 Unity / Godot）。**`send_ctrl_end()`**（Win32 SendInput 发送真实 Ctrl+End 按键，Google Docs canvas 专用）。**`load_settings`** 首次运行仅创建空默认值（工具路径探测已移至前端 OnboardingDialog） |
| `hotkey.rs` | ~143 | **全局快捷键**：`start_hotkey_listener`（独立线程 Win32 消息循环）、`do_toggle_window`、`parse_shortcut`。支持计算器键（0xB7） |
| `scheduler.rs` | ~190 | **考勤调度器**：AttendanceScheduler、create_reminder_window（400×200 毛玻璃置顶弹窗，**visible(false) 创建** + Rust 侧 500ms 延迟 show() 双保险，由 ReminderPage onMounted 调 show()）、calc_duration_until |
| `conversion.rs` | ~140 | **转换管理**：ConversionSession 状态管理（含 `tp_scale`/`tp_webp_quality` TP 预设参数）、`bring_window_to_front`（Win32 API）、`handle_file_event`（监控 01_scale/ 递归）。**双路径支持**：普通任务 `[XX]/file.webp`，Prototype `[XX]/{subcat}/file.webp`，目标分别为 `[img-XX]/` 和 `[img-XX]/{subcat}/` |

### 已注册命令

| 命令 | 参数 | 返回 | 职责 |
|------|------|------|------|
| `scan_projects` | root_dir | Vec\<ProjectInfo\> | 扫描项目根目录。同时计算 completed_tasks，调用 `find_app_icon` 查找 01_Preproduction/ 下名含 appicon 的文件。ProjectInfo 含 default_ae_file, **app_icon** |
| `scan_tasks` | project_path | Vec\<TaskInfo\> | 扫描 Export 目录下的任务。大小取 nextcloud 目录大小。同时统计 03_preview 视频上传进度（video_total/video_uploaded） |
| `scan_directory` | dir_path | Vec\<FileEntry\> | 通用一层目录扫描（目录在前，文件在后） |
| `scan_materials` | task_path | Vec\<MaterialInfo\> | **核心**：扫描 00_original，关联各阶段判定进度。返回 scales、fps。支持 Prototype |
| `list_sequence_frames` | dir_path | Vec\<String\> | 列出序列帧目录的帧文件路径（排序） |
| `scan_material_versions` | task_path, base_name, material_type | Vec\<MaterialVersion\> | 扫描素材在各工作流阶段的版本列表。序列帧原始版本 folder_path 指向目录本身（而非父目录） |
| `open_in_explorer` | path | () | Windows explorer 打开路径 |
| `collect_drag_files` | task_path, materials | Vec\<String\> | **Phase 5a**：收集素材最终产物路径用于 OS 级拖拽（优先 02_done > 01_scale > 00_original）。支持 Prototype |
| `copy_to_nextcloud` | task_path, material_names | CopyResult | **Phase 5a**：复制 02_done 文件到 nextcloud/（排除 .tps）。Prototype 保留子分类 + 额外复制 _original |
| `import_files` | source_paths, target_dir | ImportResult | 通用文件导入（外部文件/目录复制到目标目录，同名跳过） |
| `load_global_tasks` | root_dir | GlobalTaskConfig | 加载全局任务清单（.pgb1_global_tasks.json），不存在则创建默认 8 任务模板 |
| `save_global_tasks` | root_dir, config | () | 保存全局任务清单 |
| `apply_task_changes` | project_path, enabled_tasks | ApplyTaskResult | **核心**：对比新旧启用列表，创建/归档任务文件夹（Export + nextcloud），更新 .pgb1_project.json。Prototype 特例：`00_original`/`02_done` 下创建 7 个固定子分类目录；`01_scale` 只建空目录（子分类目录由缩放操作按需创建）。**不操作 PSD**（PSD 8 个固定子目录在 `create_project` 时一次性创建） |
| `list_archived_tasks` | project_path | Vec\<ArchivedVersion\> | **时光机**：扫描 .archived_tasks/，按任务名分组+时间倒序。同时清理 60 天过期归档 |
| `restore_archived_task` | project_path, task_name, timestamp | () | 时光机：恢复归档，冲突检查，更新 enabled_tasks |
| `delete_archived_version` | project_path, task_name, timestamp | () | 时光机：删除指定归档版本 |
| `preview_normalize` | task_path | Vec\<NormalizePreviewItem\> | **Phase 5b**：扫描 00_original，识别静帧/序列帧，返回预览。支持 Prototype |
| `execute_normalize` | items | () | **Phase 5b**：物理执行重命名和移动操作 |
| `execute_scaling` | requests | () | **Phase 5c**：对静帧执行高质量缩放（Lanczos3），整理至 `01_scale/[XX]` |
| `start_conversion` | app_handle, request | () | **Phase 5d**：启动转换会话，递归监控 `01_scale/`，检测新 .webp 自动移到 `02_done/[img-XX]/` |
| `stop_conversion` | — | () | **Phase 5d**：停止转换会话，终止 Imagine 进程 |
| `execute_sequence_conversion` | app_handle, sequences | () | **Phase 5d**：序列帧 TexturePacker 转换流程，整理三件套到 `02_done/[an-XX-YY]/` |
| `create_project` | root_dir, project_name, deadline? | ProjectInfo | **新建项目**：校验项目名，创建标准目录骨架 + .pgb1_project.json。含 nextcloud/preview/breakdown/ 和 PSD/ 下 8 个固定子目录（`PSD_SUBCATEGORIES` 常量） |
| `toggle_subtask_completion` | project_path, subtask_key | Vec\<String\> | 切换子任务完成状态，持久化到 completed_subtasks |
| `mark_upload_prompted` | project_path, task_name, prompted | () | 标记/取消任务的上传提醒状态 |
| `load_attendance_config` | app_handle | AttendanceConfig | 加载日报打卡配置 |
| `save_attendance_config` | app_handle, config | () | 保存日报打卡配置 |
| `save_attendance_password` | username, password | () | 保存打卡密码到 Windows Credential Manager |
| `load_attendance_password` | username | String | 读取打卡密码 |
| `execute_clock_action` | app_handle, action | String | **打卡自动化**：WebView 登录 → 导航到打刻 → 点击出勤/退勤 → 更新记录 |
| `show_clock_webview` | app_handle | () | 前台显示打卡 WebView |
| `close_clock_webview` | app_handle | () | 关闭打卡 WebView 窗口 |
| `open_daily_report` | app_handle | () | WebView 打开日报 URL，后台 spawn 轮询 Win32 SendInput Ctrl+End 滚动到底部（10 次 × 3 秒，Google Docs canvas 专用） |
| `test_reminder` | app_handle, reminder_type | () | 设置页测试用：spawn 异步触发指定类型的提醒弹窗（避免 sync 命令死锁，复用 create_reminder_window） |
| `load_attendance_record` | app_handle | AttendanceRecord | 加载本地打卡记录 |
| `save_attendance_record` | app_handle, record | () | 保存本地打卡记录 |
| `schedule_overtime_reminder` | app_handle, scheduler, minutes | () | 创建一次性加班定时提醒 |
| `show_overtime_dialog` | app_handle | () | spawn 异步创建加班时间设置弹窗（避免 sync 命令死锁） |
| `reschedule_attendance` | app_handle, scheduler | () | 重置所有定时任务 |
| `translate_text` | api_key, model, lang_a, lang_b, text | String | **翻译**：Rust 后端调用 Gemini API（reqwest）。支持中/英/日三语自动检测互译 |
| `toggle_translator_window` | app_handle | () | 切换翻译窗口显示/隐藏 |
| `load_shortcuts` | app_handle | Vec\<Shortcut\> | 从 app_config_dir/shortcuts.json 加载快捷方式列表 |
| `save_shortcuts` | app_handle, shortcuts | () | 序列化写入 shortcuts.json |
| `launch_shortcut` | shortcut_type, path | () | 启动快捷方式：app=直接运行exe，folder=explorer打开，web=cmd start打开 |
| `scan_app_shortcuts` | — | Vec\<AppShortcut\> | 扫描开始菜单+桌面 .lnk，COM 解析目标 exe 路径 |
| `extract_exe_icon` | app_handle, exe_path, icon_id | String | 提取 exe 图标为 PNG（SHGetImageList JUMBO 256px → BGRA→RGBA） |
| `fetch_favicon` | app_handle, url, icon_id | Option\<String\> | 获取网页 favicon（HTML解析+favicon.ico降级，验证≥32px） |
| `rename_material` | task_path, base_name, new_base_name, material_type | () | 重命名素材所有版本（00_original/01_scale/**/02_done/**/nextcloud），序列帧同步重命名目录+内部帧文件 |
| `delete_material` | task_path, base_name, material_type | () | 删除素材所有版本文件，序列帧用 remove_dir_all |
| `read_text_file` | path | String | 读取文本文件内容（UTF-8），供 FileDetailSidebar TXT 预览用 |
| `find_game_exe` | root_dir | Option\<String\> | 递归遍历目录树，检测游戏原型启动程序。Unity：`UnityCrashHandler64.exe` 指纹 → 同目录另一 exe；Godot：`.pck` 同名配对 → `{stem}.exe`（跳过 `.console.exe`）。任意深度均可识别 |
| `open_file` | path | () | 用系统关联程序打开指定文件（ShellExecuteW "open"），.tps → TexturePacker，.aep → After Effects |
| `rename_sequence_fps` | task_path, base_name, old_fps, new_fps | () | 修改序列帧帧率：重命名 02_done/[an-XX-{old}]/ → [an-XX-{new}]/（按 base_name 匹配精准定位） |
| `set_default_ae_file` | project_path, file_name | () | 设置项目默认 AE 工程文件名，写入 .pgb1_project.json |
| `update_project_deadline` | project_path, deadline? | () | 更新项目截止日期，写入 .pgb1_project.json |
| `set_project_priority` | project_path, priority? | () | 设置项目优先度写入 .pgb1_project.json |
| `set_task_priority` | project_path, task_name, priority? | () | 设置任务优先度写入 .pgb1_project.json 的 task_priorities Map |
| `delete_project` | project_path | () | 将项目目录**移入回收站**（Windows SHFileOperationW + FOF_ALLOWUNDO），含安全检查（.pgb1_project.json 必须存在） |
| `rename_project` | project_path, new_name | ProjectInfo | 重命名项目目录（fs::rename）+ 更新 .pgb1_project.json 的 project_name 字段，返回新的 ProjectInfo |
| `scan_preview_videos` | task_path, nextcloud_preview_path | Vec\<PreviewVideoEntry\> | 扫描 03_preview/，对比 nextcloud/preview/（及 breakdown/）判断每个文件的上传状态（uploaded/outdated/none） |
| `copy_preview_to_nextcloud` | file_path, nextcloud_preview_path | () | 复制预览视频到 nextcloud/preview/，含 _breakdown 的自动路由到 preview/breakdown/ |
| `extract_psd_thumbnail` | path, max_size | Option\<String\> | **async**：用 psd crate 合并图层，resize 到最长边 max_size px，编码为 JPEG base64 data URI。spawn_blocking 不阻塞主线程 |
| `get_file_mtime` | path | u64 | 返回文件修改时间（Unix 秒），供前端 PSD 缓存失效判断用 |

### 数据模型

| 模型 | 用途 |
|------|------|
| `ProjectConfig` | .pgb1_project.json 文件结构（enabled_tasks, archived_tasks, completed_subtasks, upload_prompted_tasks, **default_ae_file**）。新增 priority: Option\<String\>；task_priorities: HashMap\<String,String\>（任务优先度 Map） |
| `ProjectInfo` | 项目信息 DTO（含 completed_tasks, **default_ae_file**, **app_icon**）。新增 priority: Option\<String\> |
| `TaskInfo` | 任务信息 DTO（含 material_total, material_uploaded, **video_total, video_uploaded**，大小取 nextcloud 目录）。新增 priority: Option\<String\> |
| `FileEntry` | 文件/目录条目 DTO |
| `PreviewVideoEntry` | 预览视频文件条目 DTO（name, path, extension, size_bytes, **upload_status**） |
| `MaterialInfo` | 素材信息 DTO（含 scales: Vec\<u32\>、fps: Option\<u32\>） |
| `MaterialVersion` | 素材版本 DTO（含 stage, scale, folder_path, size_bytes）。序列帧原始版本 folder_path = 目录本身 |
| `DragMaterialRequest` | 拖拽请求 DTO（name, material_type） |
| `CopyMaterialRequest` | 复制请求 DTO（name, material_type） |
| `CopyResult` | 复制结果 DTO（copied_count, errors） |
| `ImportResult` | 导入结果 DTO（imported_count, skipped_count, errors） |
| `GlobalTaskConfig` | 全局任务清单配置（tasks: Vec\<GlobalTask\>） |
| `GlobalTask` | 全局任务（name + children: Vec\<GlobalTaskChild\>） |
| `GlobalTaskChild` | 子任务（name） |
| `ApplyTaskResult` | 任务变更结果（created, archived, errors） |
| `ArchivedVersion` | 归档版本信息（task_name, timestamp, display_time, path） |
| `AttendanceConfig` | 日报打卡配置（**mode**: "off"/"auto"/"record_only" + attendance + daily_report + username） |
| `AttendanceSettings` | 考勤设置（clock_in_time, clock_out_time, url, **lunch_start_time?**, **lunch_end_time?**） |
| `DailyReportSettings` | 日报设置（**enabled: bool** + time + url） |
| `AttendanceRecord` | 本地打卡记录（last_clock_in, last_clock_out 日期字符串） |
| `MaterialType` | 枚举：Image, Sequence, Video, Other |
| `MaterialProgress` | 枚举：None, Original, Scaled, Done, Uploaded |
| `ShortcutType` | 枚举：App, Folder, Web |
| `Shortcut` | 快捷方式 DTO（id, shortcut_type, name, path, icon_cache, order） |
| `ShortcutsConfig` | shortcuts.json 文件结构（shortcuts: Vec\<Shortcut\>） |
| `AppShortcut` | Windows 应用快捷方式（name, target_path） |

---

## 关键架构模式

**数据流**：页面 → Composable → Tauri invoke → Rust 命令 → 文件系统 → DTO → ref → UI

**状态管理**：模块级单例（useNavigation, useTheme）+ 普通可组合式（useProjects 等）

**样式 SSOT**：所有视觉参数通过 CSS 变量，零硬编码。组件只用 `var(--*)`

**侧边栏架构**：TaskPage/GameIntroPage 用 `<Teleport to="#content-row">` 将侧边栏传送到 MainLayout 层级，与 main-content 平级。侧边栏样式使用非 scoped style 块

**Prototype 特例**：后端自动检测任务名，多扫一层子分类目录。前端按 `name` 中的 `/` 分组。名称格式 `"subcat/basename"`

**任务管理系统**：三层架构——全局任务清单（.pgb1_global_tasks.json）→ 项目启用列表（.pgb1_project.json 的 enabled_tasks）→ 文件系统（Export/ + nextcloud/ 目录）。归档到 .archived_tasks/{TaskName}/timestamp_{YYYY-MM-DD_HH-MM}/

**进度计算规则**：分母 = 无子任务的父任务数 + 所有子任务数（有子任务的父任务本身不计入）。无子任务父任务的完成判定 = nextcloud 目录全素材已上传（completed_tasks）；有子任务父任务的完成判定 = 所有子任务在 completed_subtasks 中

**转换流程**：静帧 → 监控 01_scale/（递归）→ 检测新 .webp → 按所在 [XX] 目录名解析比例 → 移到 02_done/[img-XX]/。序列帧 → 从 00_original/ 读取 → TexturePacker CLI（--scale/--webp-quality/--opt 从全局设置 `tp_scale`/`tp_webp_quality` 读取，--opt 按序列帧名尾缀 "normal" 自动判定 RGBA8888/RGB888）→ patch .tps globalSpriteSettings.scale（1→tp_scale）→ GUI 用户调整 → 检测 .webp 是否存在（否则删 .tps + emit `sequence-conversion-failed`）→ `parse_tps_scale` 锚定 globalSpriteSettings 读取实际 scale → 整理三件套到 02_done/[an-XX-YY]/

**考勤调度系统**：scheduler.rs 管理 3 个常驻定时任务（出勤/退勤/日报）+ 1 个临时加班任务。Arc\<Mutex\<AttendanceScheduler\>\> 作为 Tauri State 管理。提醒弹窗 = 独立 WebviewWindow（400×200 毛玻璃置顶），指向 Vue 路由 `/reminder/:type`

**翻译系统**：hotkey.rs 在独立线程运行 Win32 消息循环，监听全局热键。首次按键时动态创建 400×500 可调大小 always_on_top WebviewWindow，加载 `/translator` 路由，延迟 50ms 应用 Acrylic 毛玻璃。Gemini API 在 Rust 后端通过 reqwest 调用，API Key 不暴露给前端。
