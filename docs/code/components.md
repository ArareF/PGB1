# 组件详情

> `src/components/` 下 31 个 Vue 组件的 Props、职责细节、架构决策。
> 顶层索引见 [CODE_INDEX.md](../../CODE_INDEX.md#3-组件31)。

---

## 卡片家族

### ProjectCard.vue（490 行）

**Props**：`project: ProjectInfo`

**功能**：
- 图标 + 名称 + 截止日期 + 进度条 + 优先度圆点 + ··· 菜单 + 笔记图标
- `AppIcon` 加载策略：PNG 用 `convertFileSrc` 直接渲染；PSD/PSB 调 `getPsdThumbnail(128)`；无图标降级 SVG 占位
- 进度条分母 = 无子任务父任务数 + 所有子任务数
- 优先度四档：`急` / `高` / `普`（= null）/ `停`，圆点 9px 实色无文字
- 菜单项：重命名 / 修改截止日期 / 删除 / 批注 / 优先度四档

**架构决策（防火手记）**：
- 根元素用 `<div>` 而非 `<button>`，避免嵌套 button
- ··· 菜单 `Teleport to body`：父级 `glass-subtle` 的 `backdrop-filter` 会创建合成层导致子级毛玻璃失效；菜单用 `position: fixed` + `getBoundingClientRect()` 动态锚定
- `@note-save` 处理 NoteTooltip checkbox 切换（乐观更新 `project.note`）

### TaskCard.vue（271 行）

**Props**：`task`, `subtaskProgress?`

**进度判定**：
- 有子任务：按子任务进度（0/N 灰 / 进行中 X/N 黄 / 完成绿），不看文件上传
- 无子任务（叶子）：未开始灰 / 制作中蓝（有素材未全传）/ 完成绿（需 `filesAllUploaded()`）
- 大小取 nextcloud 目录

**架构决策**：
- 单根节点：`<Teleport>` 移至 `<button>` 内部，兼容 `<TransitionGroup>` 入场 / FLIP 动画
- 菜单只含优先度选择器（不走 EditProjectDialog）

### MaterialCard.vue（283 行）

**Props**：`material`, `multiSelect?`, `checked?`, `hasNote?`, `notePreview?`

**序列帧角标**：显示 fps（转换后才显示）；`SequencePreview :key="${path}-${fps}"`，fps 变化时强制重挂使动画速度即时更新。

**架构决策**：手动 `glass-subtle`（不用 backdrop-filter），避免大量卡片创建独立合成层。

### NormalCard.vue（366 行）

**Props**：`file: FileEntry`, `hasNote?`, `notePreview?`

**素材系列合并用的覆盖 Props**（全部可选，不传即原行为）：`displayName?`（覆盖卡片名）、
`subLabel?`（名称下方副标题，如「最新 260807」）、`versionCount?`（>1 时右上角角标，
与左上角多选框对称）、`formatLabel?`（覆盖右下角格式标签，如 `PSD·JPG`）、
`selectionPath?`（覆盖 `data-path`，让合并卡以最新版 PSD 作为多选 / 框选身份）。
角标配色走 `--card-version-badge-*` token。

**多类型支持**：
- 视频：`onMounted` canvas 截帧
- PSD/PSB 两级缩略图策略：① `file.thumbnail_path` 有值（scan 时磁盘缓存命中）→ `loading="lazy"` 直接渲染；② 无缓存 → `IntersectionObserver` 进视口后触发 IPC（先清 JS 缓存防旧图污染），降级为 PS 图标直到加载完成
- PDF：红色 PDF 图标

**多选三件套**：`multiSelect?` + `checked?` + `data-path` + `card-checkbox-shared`

### SequencePreview.vue（115 行）

**Props**：`folderPath`, `fps?`, `maxWidth?`, `transparent?`

Canvas 序列帧动画播放器，`mount` 后自动循环播放，LRU 缓存。`transparent=true` 时 `clearRect` 透明背景 + 棋盘格 CSS。

### ImageViewer.vue（127 行）

**Props**：`src`

通用可缩放/拖拽图片查看器（滚轮缩放 + 鼠标拖拽）。TaskPage 侧边栏和 FileDetailSidebar 共用。

---

## 侧边栏体系

### SidebarShell.vue（371 行）

**Props**：`show`, `title`, `widthPercent?`, `teleportTarget?`, `teleportDisabled?`, `hasActions?`

**职责**：侧边栏外壳（从 FileDetailSidebar + TaskPage 提取统一）。封装容器、拖拽调宽（localStorage 持久化）、全屏 FLIP 动画（Esc 退出）、进出场动画。

**三个 slot**：
- `#default`（scoped: `isFullscreen` + `toggleFullscreen`）
- `#actions`（底部操作按钮）
- `#overlay`（弹窗覆盖层）

**架构决策**：
- 手动 `glass-strong`（避免 backdrop-filter 兄弟冲突）
- 通用信息展示样式（`sidebar-section` / `info-list` / `version-card`）内置

### FileDetailSidebar.vue（619 行）

**Props**：`file: FileEntry | null`, `widthPercent?`, `versions?: FileEntry[]`, `versionLabelOf?`, `allowActions?`, `note?`, `teleportTarget?`, `teleportDisabled?`

**使用 SidebarShell 作外壳**。支持多类型预览：
- 图片：ImageViewer，`aspect-ratio: 4/3` 自适应
- 视频：VideoPlayer 子组件
- TXT：`read_text_file`
- PSD/PSB：`usePsdThumbnail` 800px 高清 + 「用 Photoshop 打开」按钮；**首次打开延迟 300ms** 等进场动画结束；每次请求前清 JS 缓存保证感知修改
- PDF：PdfPreviewSection 子组件（iframe + 翻译集成）
- 其他：图标占位

**版本历史**：`versions` prop 传入多版本列表，点击 emit `select-version` 切换播放。版本卡片式布局：左列版本标签 + 文件大小，右侧扩展名 + 打开文件夹按钮。

**`versionLabelOf?: (file, index) => string`**：覆盖版本条目标题。不传时按「最新版本 / 版本 N」编号，
**该编号假设 versions 是旧→新**（预览视频如此）。MaterialsPage 的素材系列是新→旧，必须传此函数
（传日期 + 尾缀），否则「最新版本」会标在最旧那一行。

**`allowActions=true`**：底部显示重命名/删除按钮，内联弹窗 overlay。emit `rename(newName)` / `delete()` 由父页面执行 invoke + 刷新。

**笔记**：可选 `note` prop + `save-note` emit，有值时 `sidebar-body` 内显示 NoteEditor section。

### VideoPlayer.vue（272 行）

**Props**：`src: string`, `isFullscreen: boolean`

从 FileDetailSidebar 提取。自定义播放控制条（播放/暂停 + 进度条 + 时间显示），`object-fit: contain` + `flex: 1` 全屏适配。

### PdfPreviewSection.vue（235 行）

**Props**：`filePath: string`, `isFullscreen: boolean`

从 FileDetailSidebar 提取。iframe 直接渲染（WebView2 内置 PDF 引擎） + `usePdfTranslate` 集成翻译 UI（进度 / 切换 / 重试）。

### FolderBrowserDialog.vue（425 行）

**Props**：`show`, `initialPath`（emits: `close`）

**职责**：文件夹浏览弹窗（Teleport to body）。点击子文件夹卡片 → 弹窗内展示内容，支持递归进入。内部 `pathStack` 路径栈驱动面包屑。

**交互**：
- 弹窗尺寸默认 70vw × 75vh，四边 + 四角 8 方向拖拽调整（最小 40%，最大 95%）
- `localStorage: pgb1-folder-browser-size` 持久化
- 复用 NormalCard 展示文件 + FileDetailSidebar 查看详情（`teleportTarget=".fb-body"`）

---

## 标题与状态

### TitleBar.vue（422 行）

**职责**：顶部标题栏，消费 `useNavigation()`。左侧标题岛（`flex-shrink: 0` 不压缩）+ 中间岛（嵌入 StatusBar）+ 右侧功能岛（独自承担窄窗口压缩）。

**动作按钮**：
- `NavAction` 支持：`onLongPress(btnRect: DOMRect)`（长按 500ms 回调）、`active`（全亮强调）、`hint`（弱描边强调，与 active 互斥）、`variant`（`default` / `success` 绿色玻璃）
- 滚轮横滚支持

**架构决策（防火手记）**：
- 左岛手动 glass：不用 `glass-medium` 类（与 center 岛相邻会触发 backdrop-filter 兄弟冲突）
- `.title-text-wrap` 包裹层防止转场动画 leave 态 `position:absolute` 标题文字侵入返回按钮
- JS FLIP 宽度动画（`watch flush:pre/post` + `flipWidth`）
- **flipWidth bug 修复**：读 toWidth 前先清除残留内联样式（`style.width = ''`）再 `offsetWidth` 强制 layout，防止快速连续导航污染 toWidth
- 返回按钮 leave 动画：`.nav-back-leave-active.back-btn` 加 `top:0; bottom:0`，防止 `position:absolute` 脱离 flex 后向上跳动
- hover `translateY(-2px)` 裁切修复：`.title-bar-center` 覆盖 `overflow:visible`；`.title-bar-actions` 加 `padding-block:6px` 建立 ink overflow 缓冲区

### StatusBar.vue（577 行）

**职责**：状态栏组件（嵌入 TitleBar 中间岛）。

**布局**：
- 左列：时间 / 日期 / 节假日标签（短文案 + `white-space:nowrap` + `flex-shrink:0`）
- 右列：已工作胶囊（需 `hasClockIn && !hasClockOut`） + 倒计时胶囊（`hasClockOut` 后显示"下班咯"，午休显示 `午休 Xm`）
- 最右：番茄钟按钮（无形态纯光晕，`::before + filter:blur(16px) + isolation:isolate`）
- 番茄钟状态色：空闲白极淡 / 专注蓝 / 超时红绿交替 / 休息绿 / 休息结束绿呼吸

**长按配置面板**：长按 500ms 弹出（Teleport to body），含 5 个 boolean 开关 + 番茄钟时长 + 假日日历地区下拉（自动/中国/日本/不显示）。

---

## 左侧栏

### Sidebar.vue（509 行）

**职责**：左侧快捷方式栏，iOS 风格交互。

**交互**：
- 单击启动，长按 500ms 进入编辑模式（图标抖动 + 右上角红色 × 删除徽章），点击空白退出
- 编辑模式内拖拽重排（`pointermove + elementFromPoint`，实时更新 `displayOrder`）
- 拖拽排序 FLIP 动画：`<TransitionGroup name="sort">` + `.sort-move { transition: transform 200ms }`

**添加图标自动提取**：应用 → `extract_exe_icon` 256px，网页 → `fetch_favicon`。

**架构决策**：
- 手动 glass：不用 `glass-medium` 类（与 main-content 相邻会触发 backdrop-filter 兄弟冲突）
- Hover 光晕：`::before` 伪元素蓝色模糊光晕（`filter:blur(14px)` + `isolation:isolate`），无边框无阴影
- 编辑抖动：6 个不规则关键帧 + `nth-child(2n/3n/4n)` 错相，避免整齐同步

### ShortcutDialog.vue（541 行）

**Props**：`show?`（emits: `save`, `cancel`）

**职责**：快捷方式添加弹窗（仅添加，无编辑）。表单状态机、路径选择、图标预览、应用列表扫描全部下沉到 [`useShortcutForm`](composables.md#useshortcutform)。

**类型分支**：
- 应用：扫描开始菜单/桌面 `.lnk` + 搜索过滤 + 手动浏览备用
- 文件夹：浏览选择
- 网页：手动输入 URL

**图标预览区**：44×44 可点击图标框，悬停出现铅笔遮罩，点击唤起文件选择框（PNG/JPG/ICO/BMP/WEBP）→ `copy_icon_to_cache`。「预览自动图标」按钮：有选中目标时显示，在弹窗内预取并填入预览。

---

## 弹窗体系

### CreateProjectDialog.vue（130 行）

新建项目弹窗（项目名 + 截止日期）。日期标准化支持 `20260616` / `2026-06-16` 两种格式。Teleport to body + 进出场动画。

### EditProjectDialog.vue（168 行）

**Props**：`project: ProjectInfo`, `mode: 'rename' | 'deadline' | 'delete'`（emits: `updated`, `deleted`, `cancel`）

项目管理弹窗，通过 mode 复用三种操作：
- `rename`：预填项目名，调用 `rename_project`
- `deadline`：预填 + 日期标准化，调用 `update_project_deadline`
- `delete`：红色确认，调用 `delete_project`（移入回收站）

### OnboardingDialog.vue（481 行）

**Props**：`show`（emits: `complete[mode]`）

首次引导 4 步向导：语言选择 → 项目目录 → 工具路径 → 打卡模式。表单状态机 / 系统扫描 / 保存闭环全部下沉到 [`useOnboardingForm`](composables.md#useonboardingform)，SFC 只保留 template + style。

**步骤校验**：项目目录必填；工具路径需 Imagine + TexturePacker CLI；未填好时「下一步」按钮灰化。

**完成时 emit 打卡模式值**：App.vue 据此决定是否跳转设置页出勤 Tab。

### PageGuideOverlay.vue（125 行）

**Props**：`show`, `annotations: GuideAnnotation[]`

通用页面指引遮罩。Teleport to body，全屏半透明遮罩 + fixed 定位批注气泡（支持上下左右箭头），点击任意处关闭。`white-space: pre` 支持 `\n` 手动换行。

### NormalizationDialog.vue（455 行）

**Props**：`taskPath`

规范化预览弹窗（Phase 5b）。扫描并识别静帧（去 `_01`）与序列帧（归类），展示变更预览，支持一键执行。

### ConversionDialog.vue（389 行）

**Props**：`taskPath`, `materials`

格式转换选择弹窗（Phase 5d）。分区列出未转换的静帧与序列帧，序列帧强制要求输入帧率，提交后开启后端转换会话。

### SubtaskDialog.vue（251 行）

**Props**：`show`, `enabledSubtasks`, `completedSubtasks`, `autoPrompt`, `revertPrompt`, `hasChanges`

子任务管理弹窗（从 TaskPage 提取）。子任务启用/完成状态切换、自动提醒/恢复确认、变更预览。

### UpdateDialog.vue（248 行）

自动更新提醒弹窗。消费 `useUpdater()`：`updateAvailable` / `updateInfo` / `downloading` / `progress` / `installUpdate` / `skipVersion` / `dismiss`。挂载在 App.vue 根层级。

### UploadConfirmDialog.vue（60 行）

**Props**：`fileCount`

上传确认弹窗（拖拽后询问是否已上传到网盘）。Teleport to body + 进出场动画。

---

## 笔记系统

### NoteTooltip.vue（148 行）

**Props**：`target: HTMLElement | null`, `text: string`（emits: `save`）

**职责**：笔记悬停预览气泡（可交互）。接收完整笔记原文，内部用 NoteRenderer 渲染。

**checkbox 支持**：`localText` ref（watch props.text 同步），NoteRenderer `@toggle-checkbox` → 更新 localText + emit `save(text)` 向外传递；卡片组件 `@save` 转发为 `note-save`，页面直接持久化（乐观更新）。

**悬停桥接**：鼠标离开卡片后 150ms 缓冲（`BRIDGE_DELAY`），期间进入 tooltip 则取消隐藏，实现从卡片到 tooltip 的平滑过渡。

### NoteRenderer.vue（136 行）

**Props**：`text: string`（emits: `toggle-checkbox`）

**职责**：笔记渲染组件。逐行解析 markdown 子集：
- `[text](url)` → 命名链接
- `https?://` → 裸链接（`openUrl` 跳外部浏览器）
- `- [ ] ` / `- [x] ` → checkbox
- `**..**` → `<strong>`
- `*..*` → `<em>`

**架构决策**：
- `v-for` 复合 key `${idx}-${line.checked}`：checkbox 切换时强制重建 DOM 节点，规避 WebView2 `:checked` 属性更新不触发重绘
- XSS 安全（Vue 模板拼接，不 `innerHTML`）

### NoteEditor.vue（274 行）

**Props**：`modelValue: string`（emits: `update:modelValue`, `save`, `toggle-checkbox`）

**职责**：笔记编辑器（render / edit 双模式）。
- 渲染模式：NoteRenderer + 编辑按钮
- 编辑模式：迷你工具栏（B / I / 链接 / 清单 4 按钮）+ textarea + 进度条
- 空 `modelValue` 自动进编辑模式
- 工具栏通过 `selectionStart/End` 精确插入语法
- `defineExpose({ mode })` 供父组件读取

### NoteDialog.vue（164 行）

**Props**：`show`, `title`, `note`（emits: `save`, `update`, `cancel`）

笔记弹窗（双模式适配）。内嵌 NoteEditor（`:save-on-blur="false"`），渲染模式底部仅「关闭」，编辑模式底部「保存 + 取消」。

**双事件模型**：
- `save` = 显式保存（关闭弹窗）
- `update` = checkbox 切换静默保存（不关闭弹窗）

---

## 贴图板

### PinboardCanvas.vue（574 行）

**Props**：`pins`, `viewport`, `activeTool`, `activeColor`, `strokeSize`, `fontSize`, `canvasAnnotations`
**Emits**：`select-pin`, `deselect`, `update-pin`, `delete-pin`, `add-annotation`, `remove-annotation`, `add-canvas-annotation`, `remove-canvas-annotation`, `update-viewport`

**职责**：贴图板自由画布。
- 平移：中键 / 空格 + 左键拖拽
- 滚轮缩放：`MAX_ZOOM=1.0`（不超过 100%）
- 渲染 PinItem 子组件（透传 strokeSize / fontSize）

**画布标注层**：`<canvas>` overlay，支持画笔 / 箭头 / 矩形 / 椭圆 / 文字 / 橡皮擦在画布背景绘制。世界坐标系：`toWorldCoords(e)` 屏幕→世界坐标，`ctx.setTransform(zoom, 0, 0, zoom, panX, panY)` viewport 变换渲染。

**PS 橡皮擦**：`globalCompositeOperation = 'destination-out'` 像素级擦除（3 pass 多次绘制消除抗锯齿残留）。

**笔刷光标指示器**：跟随鼠标的圆形 div，大小 = `strokeSize × zoom`（仅 pen/eraser）。

### PinItem.vue（639 行）

**Props**：`pin`, `imageUrl`, `isSelected`, `activeTool`, `activeColor`, `canvasZoom`, `strokeSize`, `fontSize`
**Emits**：`select`, `update-position`, `update-size`, `delete`, `add-annotation`, `remove-annotation`

**职责**：单张贴图组件。可拖拽移动 + 8 方向 resize 手柄缩放（角拖拽保持宽高比）。

**Pin 级标注**：内嵌 `<canvas>` overlay，支持画笔 / 箭头 / 矩形 / 椭圆 / 文字 / 橡皮擦（标注坐标归一化为 0-1，与 pin 尺寸无关）。

**鼠标事件**：绘制 `mousemove / mouseup` 绑定 window（拖出贴图边界仍可继续绘制）。

**尺寸规范**：
- 删除按钮 28×28，`z-index: 20`
- resize 手柄 8px，`z-index: 20`
- 标注画布 `z-index: 1`
