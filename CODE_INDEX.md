# PGB1 代码索引

> 全量源代码文件职责目录视图。新会话快速了解代码现状用。
> 详细信息（Props / 状态 / 防火手记 / 架构决策）见 [`docs/code/*.md`](docs/code/)。
> 最后更新: 2026-04-17

---

## 文件统计

| 目录 | 文件数 | 总行数 | 说明 |
|------|--------|--------|------|
| `src/components/` | 31 | 9893 | Vue UI 组件 |
| `src/composables/` | 23 | 2959 | 组合式函数（逻辑复用） |
| `src/views/` | 19 | 8975 | 页面（含 `settings/` 子目录 5 个 Tab 子组件） |
| `src/styles/` | 4 | 1789 | CSS 设计系统 |
| `src/layouts/` | 1 | 321 | 主布局 |
| `src/types/` | 2 | 46 | TypeScript 类型定义 |
| `src/utils/` | 1 | 23 | 工具函数 |
| `src/config/` | 3 | 99 | 配置 SSOT（app/onboarding/fileTypes） |
| `src/i18n/` + `src/locales/` | 3 | 1318 | 国际化（vue-i18n + zh-CN + en） |
| `src/router/` + 入口 | 5 | 167 | 路由 + main/App/vite-env |
| `src-tauri/src/` | 22 | 9544 | Rust 后端 |
| **合计** | **113** | **35696** | |

---

## 详情索引

| 二级文档 | 包含什么 |
|---------|---------|
| [docs/code/components.md](docs/code/components.md) | 31 个组件的 Props、关键公共类、笔记体系接入 |
| [docs/code/composables.md](docs/code/composables.md) | 22 个 composable 的关键导出、状态、模块级单例说明 |
| [docs/code/views.md](docs/code/views.md) | 18 个页面的数据流、与组件/composable 的依赖图 |
| [docs/code/rust-backend.md](docs/code/rust-backend.md) | Rust 命令清单（约 75 个）+ 数据模型 + 调度器 |
| [docs/code/styles-system.md](docs/code/styles-system.md) | CSS 变量 SSOT、毛玻璃层级、动画系统、公共类 |

---

## 关键架构模式（速记）

| 模式 | 要点 |
|------|------|
| **数据流** | 页面 → Composable → `invoke` → Rust 命令 → 文件系统 → DTO → ref → UI |
| **状态管理** | 模块级单例（`useNavigation` / `useTheme` / `useScale`）+ 普通可组合式 |
| **SSOT** | 视觉参数全走 CSS 变量，文案走 i18n，配置走 `src/config/*.ts`，业务常量走 Rust 模型 |
| **侧边栏架构** | TaskPage/GameIntroPage 用 `<Teleport to="#content-row">` 传送到 MainLayout 层级 |
| **Prototype 特例** | 后端按 `split_prototype_name` 多扫一层子分类，前端按 `name` 中 `/` 分组 |
| **进度计算** | 分母 = 无子任务父任务数 + 所有子任务数；叶子任务看 `nextcloud` 全上传 |
| **转换流程** | 静帧监控 `01_scale/` → Imagine webp → 移 `02_done/[img-XX]/`；序列帧走 TP CLI |
| **考勤调度** | `scheduler.rs` 常驻 3 定时任务（出勤/退勤/日报）+ 临时加班；WebView 自动化 |
| **翻译系统** | `hotkey.rs` 独立线程 Win32 消息循环 + SSE 流式 Gemini API |

---

## 1. 入口与配置

| 文件 | 行数 | 职责 |
|------|------|------|
| `src/main.ts` | 12 | Vue 3 + Router + i18n 初始化入口 |
| `src/App.vue` | 74 | 根组件，引导判定 + MainLayout + UpdateDialog |
| `src/i18n/index.ts` | 15 | vue-i18n 实例（legacy:false, zh-CN 默认） |
| `src/locales/zh-CN.ts` | 668 | 中文 locale（24 个 namespace） |
| `src/locales/en.ts` | 635 | 英文 locale（结构对齐 zh-CN） |
| `src/config/app.ts` | 10 | 软件元信息 SSOT：`APP_NAME` / `APP_VERSION` / `APP_DEVELOPER` |
| `src/config/onboarding.ts` | 81 | 引导数据 SSOT：`PAGE_INTROS` + `PAGE_GUIDE_ANNOTATIONS` |
| `src/config/fileTypes.ts` | 8 | 文件扩展名常量 SSOT（IMAGE/VIDEO/TEXT/PSD/PDF） |
| `src/router/index.ts` | 74 | 10 条路由定义 |
| `src/vite-env.d.ts` | 7 | Vite 类型声明 |

---

## 2. 布局

| 文件 | 行数 | 职责 |
|------|------|------|
| `src/layouts/MainLayout.vue` | 321 | 主布局 = TitleBar + Sidebar + main-content，`#content-row` 侧边栏 Teleport target |

---

## 3. 组件（31）

| 文件 | 行数 | 一句话职责 |
|------|------|-----------|
| `ProjectCard.vue` | 490 | 项目卡片（图标 / 截止 / 进度 / 优先度 / 笔记 / ··· 菜单） |
| `TaskCard.vue` | 271 | 任务卡片（子任务进度标签 / 优先度 / 笔记） |
| `MaterialCard.vue` | 283 | 素材卡片（序列帧预览 + fps 角标 / 笔记） |
| `NormalCard.vue` | 324 | 通用文件卡片（视频截帧 / PSD 缩略图 / PDF） |
| `SequencePreview.vue` | 115 | Canvas 序列帧动画播放器 + LRU 缓存 |
| `ImageViewer.vue` | 127 | 可缩放/拖拽图片查看器（滚轮 + 鼠标拖拽） |
| `FolderBrowserDialog.vue` | 425 | 文件夹浏览弹窗（路径栈 + 8 方向拖拽调宽） |
| `SidebarShell.vue` | 371 | 侧边栏外壳（拖拽调宽 + 全屏 FLIP + 进出场动画） |
| `FileDetailSidebar.vue` | 597 | 文件详情侧边栏（图/视/TXT/PSD/PDF + 版本历史 + 重命名删除） |
| `VideoPlayer.vue` | 272 | 视频播放器（自定义控制条） |
| `PdfPreviewSection.vue` | 235 | PDF iframe 预览 + 翻译 UI 集成 |
| `TitleBar.vue` | 422 | 顶部标题栏（返回 + 动态按钮 + 嵌入 StatusBar） |
| `StatusBar.vue` | 577 | 状态栏（时钟 / 日期 / 打卡胶囊 / 倒计时 / 番茄钟） |
| `Sidebar.vue` | 509 | 左侧快捷方式栏（iOS 风格长按编辑 + 拖拽排序） |
| `ShortcutDialog.vue` | 541 | 快捷方式添加弹窗（三类 + 图标预览，表单走 `useShortcutForm`） |
| `WindowControls.vue` | 92 | 窗口控制按钮（最小化 / 最大化 / 关闭） |
| `CreateProjectDialog.vue` | 130 | 新建项目弹窗（名称 + 截止日期） |
| `EditProjectDialog.vue` | 168 | 项目管理弹窗（mode 复用：重命名 / 截止 / 删除） |
| `OnboardingDialog.vue` | 481 | 首次引导 4 步向导（表单走 `useOnboardingForm`） |
| `PageGuideOverlay.vue` | 125 | 通用页面指引遮罩（批注气泡） |
| `NormalizationDialog.vue` | 455 | 规范化预览弹窗（Phase 5b） |
| `ConversionDialog.vue` | 389 | 格式转换选择弹窗（Phase 5d） |
| `SubtaskDialog.vue` | 251 | 子任务管理弹窗（从 TaskPage 抽取） |
| `NoteTooltip.vue` | 148 | 笔记悬停预览气泡（可交互 checkbox） |
| `NoteRenderer.vue` | 136 | 笔记渲染（markdown 子集 + 命名链接 + checkbox） |
| `NoteEditor.vue` | 274 | 笔记编辑器（render/edit 双模式） |
| `NoteDialog.vue` | 164 | 笔记弹窗（双模式适配 + 双事件模型） |
| `PinboardCanvas.vue` | 574 | 贴图板自由画布（画笔/箭头/矩形/椭圆/文字/橡皮擦） |
| `PinItem.vue` | 639 | 单张贴图组件（拖拽 + 8 方向 resize + pin 级标注） |
| `UpdateDialog.vue` | 248 | 自动更新提醒弹窗（消费 `useUpdater`） |
| `UploadConfirmDialog.vue` | 60 | 上传确认弹窗（拖拽后网盘询问） |

---

## 4. Composables（22）

| 文件 | 行数 | 一句话职责 |
|------|------|-----------|
| `useNavigation.ts` | 75 | 导航状态模块级单例，驱动 TitleBar 动作按钮 |
| `useProjects.ts` | 50 | `scan_projects` 数据加载 |
| `useTasks.ts` | 43 | `scan_tasks` 数据加载 |
| `useMaterials.ts` | 40 | `scan_materials` 数据加载 |
| `useDirectoryFiles.ts` | 41 | 通用目录扫描 + `openInExplorer` |
| `useFrameCache.ts` | 57 | 序列帧 LRU 缓存（max 10 序列 / 120 帧） |
| `useTheme.ts` | 30 | 明暗主题切换 + localStorage |
| `useScale.ts` | 28 | UI 全局缩放（1920 基准，clamp [0.67, 1.25]） |
| `useSettings.ts` | 126 | 应用设置 CRUD（JSON 深拷贝脱壳 Vue Proxy） |
| `usePsdThumbnail.ts` | 41 | PSD 缩略图并发去重缓存 |
| `useStatusBar.ts` | 504 | 状态栏数据单例（时钟 / 打卡 / 节假日 / 番茄钟 / 加班） |
| `useNotes.ts` | 168 | 笔记系统（hoverPreview + 乐观保存 + checkbox toggle） |
| `usePinboard.ts` | 205 | 贴图板 CRUD + 粘贴剪贴板图片 |
| `usePdfTranslate.ts` | 245 | PDF 翻译全局状态（模块级 Map 跨组件持续） |
| `useRubberBandSelect.ts` | 103 | 框选多选（视口矩形 + data-path 碰撞） |
| `useDragIntent.ts` | 36 | 拖拽意图检测（区分点击与拖拽） |
| `useMultiSelect.ts` | 86 | 多选状态封装 + 框选集成 |
| `useOnboardingForm.ts` | 250 | 新手引导 4 步表单状态机 |
| `useShortcutForm.ts` | 217 | 快捷方式表单（type 切换 + 图标预览） |
| `usePreviewVideos.ts` | 180 | 预览视频分组 / 截帧 / 上传 |
| `useMaterialSidebar.ts` | 262 | 素材侧边栏（选中 / 重命名 / 删除 / preserveCardPosition） |
| `useArchivedMaterials.ts` | 45 | 素材归档时光机数据源（list / restore / delete） |
| `useUpdater.ts` | 127 | 自动更新检查 / 下载 / 安装 |

---

## 5. 页面（18）

| 文件 | 行数 | 职责 |
|------|------|------|
| `HomePage.vue` | 448 | 项目列表 + 新建 + 三档排序 + 加班按钮 |
| `ProjectPage.vue` | 528 | 任务列表 + 快捷功能（游戏介绍/项目素材/AE/任务列表）+ 两档排序 |
| `TaskListPage.vue` | 600 | 任务管理页面（启用 / 模板 双 Tab；时光机已抽为独立页面） |
| `TimeMachinePage.vue` | 545 | 时光机独立页面（任务归档 / 素材归档 双 Tab） |
| `TaskPage.vue` | 1472 | 素材浏览主页面（树形/名称双视图 + Phase 5a–5d + 预览视频） |
| `ScalePage.vue` | 406 | 素材缩放执行页面（Phase 5c + 进度反馈） |
| `ConvertPage.vue` | 737 | 格式转换执行页面（Phase 5d + TP 预设折叠面板） |
| `GameIntroPage.vue` | 448 | 00_Game 浏览 + 游戏原型启动按钮 |
| `MaterialsPage.vue` | 655 | 项目素材页面（4 分组浏览 + 多目录笔记管理） |
| `SettingsPage.vue` | 590 | 设置页父组件（5 Tab 导航 + 保存闭环） |
| `settings/AboutSettings.vue` | 32 | 关于 Tab（版本 / 检查更新） |
| `settings/WorkflowSettings.vue` | 45 | 工作流 Tab（三个工具路径） |
| `settings/TranslationSettings.vue` | 56 | 翻译 Tab（API / 快捷键 / 默认语言对） |
| `settings/GeneralSettings.vue` | 121 | 通用 Tab（主题 / 语言 / 缩放 / 自启） |
| `settings/AttendanceSettings.vue` | 293 | 日报打卡 Tab（完全自包含 + `defineExpose`） |
| `ReminderPage.vue` | 449 | 打卡提醒弹窗页面（支持 4 种 type） |
| `OvertimePage.vue` | 230 | 加班时间设置弹窗（已弃用，未删除） |
| `PinboardPage.vue` | 910 | 贴图板独立窗口（多标签 + 撤销/重做 + 归位） |
| `TranslatorPage.vue` | 409 | 翻译悬浮窗（流式翻译 + 呼吸动画） |

---

## 6. 样式

| 文件 | 行数 | 职责 |
|------|------|------|
| `styles/design-system.css` | 1538 | **SSOT**：颜色 / 间距 / 排版 / 圆角 / 阴影 / 过渡 / 毛玻璃变量 / 优先度 token / 公共类 |
| `styles/glass.css` | 86 | 毛玻璃工具类（subtle/medium/strong） + backdrop-filter 兄弟冲突规则 |
| `styles/dialog.css` | 120 | 弹窗公共样式（overlay / content / btn 变体 / 进出场动画） |
| `styles/reset.css` | 45 | 基础重置 + 字体引用 |

---

## 7. 工具 / 类型定义

| 文件 | 行数 | 关键导出 |
|------|------|---------|
| `src/utils/format.ts` | 23 | `formatSize(bytes)`, `normalizeDeadline(raw)` |
| `src/types/task.ts` | 27 | `GlobalTask*` / `ApplyTaskResult` / `ArchivedVersion` |
| `src/types/material.ts` | 19 | `PreviewVideoEntry` / `MaterialVersion` |

---

## 8. Rust 后端（22）

### 主模块

| 文件 | 行数 | 职责 |
|------|------|------|
| `main.rs` | 6 | 应用入口 |
| `lib.rs` | 351 | Tauri 初始化 + 命令注册 + 插件 + Acrylic + hotkey + autolaunch 同步 |
| `models.rs` | 637 | 数据模型（29 struct + 3 enum） |
| `conversion.rs` | 144 | 转换会话管理（含 Prototype 双路径 + tp_scale/tp_webp_quality） |
| `hotkey.rs` | 144 | 全局快捷键（独立线程 Win32 消息循环） |
| `scheduler.rs` | 280 | 考勤调度器 + 日报 90 秒预热 |

### commands/ 子模块

| 文件 | 行数 | 职责 |
|------|------|------|
| `commands/mod.rs` | 24 | 子模块 `pub use` 重导出 |
| `commands/scanning.rs` | 1681 | 扫描命令（`scan_projects`/`scan_tasks`/`scan_materials` 等 + DirSnapshot 缓存） |
| `commands/attendance.rs` | 1183 | 考勤命令（打卡 / 日报 / 提醒 / Credential Manager） |
| `commands/conversion.rs` | 911 | 转换 / 缩放命令（Phase 5b/c/d） |
| `commands/projects.rs` | 717 | 项目管理命令（`mutate_project_config` 原子 helper 统一读改写） |
| `commands/shortcuts.rs` | 591 | 快捷方式命令（图标提取 / favicon / find_game_exe） |
| `commands/helpers.rs` | 521 | 公共辅助（DirSnapshot / PSD 缩略图 / mutate_project_config） |
| `commands/translation.rs` | 340 | 翻译命令入口（SSE 流式 Gemini） |
| `commands/translation/pdf_reflow.rs` | 450 | PDF 内容流提取 + 流式排版 |
| `commands/translation/pdf_font.rs` | 212 | CJK 字体处理（微软雅黑 Type0） |
| `commands/translation/pdf_cmds.rs` | 194 | PDF 命令整合（`build_translated_pdf`） |
| `commands/files.rs` | 701 | 文件操作（重命名 / 删除 / 回收站）+ 素材归档（archive/list/restore/delete 四命令 + 60 天 GC） |
| `commands/pinboard.rs` | 182 | 贴图板 CRUD（RGBA→PNG） |
| `commands/holiday.rs` | 148 | 外部 API 代理（IP 检测 / 节假日） |
| `commands/settings.rs` | 69 | 设置 CRUD |
| `commands/notes.rs` | 37 | 笔记 CRUD |

---

> 旧版 CODE_INDEX（338 行 / 33k tokens）因信息密度过高已按方案 B 分级重构。
> Props 列表、CSS 陷阱、Sprint 防火手记等详情全部下沉到 [`docs/code/*.md`](docs/code/)。
