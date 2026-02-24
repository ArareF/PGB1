# PGB1 全局索引

> **给 Agent**：每次新会话都是全新上下文，请先读这个文件，再按需查阅具体文档。
> **给产品总监**：Ctrl+F 搜关键词即可定位到任何设计细节的出处。

---

## 项目概要

- **产品**：PGB1 — 2D游戏特效师文件整理工具
- **技术栈**：Tauri 2.x（Rust + HTML/CSS/JS），目标 Windows
- **状态**：✅ **已打包发布 v2.2.0**（Windows NSIS 安装包，系统托盘，自定义图标）
- **角色**：产品总监（决策）+ Tech Lead / Agent（实现）

---

## 规范文档

| 文档 | 定位 | 什么时候看 |
|------|------|-----------|
| `CLAUDE.md` | 角色定义、协作契约、沟通协议、绝对红线 | 每次会话必读（自动加载） |
| `项目指南.md` | 核心原则（SSOT、模块化、防御性思维）、业务规则概要、沟通示例 | 每次会话必读 |
| `开发规范.md` | 代码规范、目录结构、质量检查清单（框架版，编程阶段补充） | 编程阶段 |
| `CODE_INDEX.md` | 全量源代码文件职责索引（50 个文件，~14500 行） | 编程阶段，了解代码现状 |

---

## 设计文档总览

### 界面与交互 → `design/界面设计.md`（1875行）

这是最大的设计文档，包含所有页面和功能的完整设计。

| 主题 | 章节 | 行号 |
|------|------|------|
| **整体布局** | 主窗口四区域（左侧栏+顶部导航+主功能区+窗口控制） | L8 |
| **页面导航** | 三级架构：主页→项目页→任务页 + 两个辅助页面 | L75 |
| **主页** | 项目卡片列表、更多菜单（日报打卡/程序设置） | L100 |
| **快捷方式栏** | 左侧边栏启动台，应用/文件夹/网页三种类型，添加/编辑/拖拽排序 | L122 |
| **项目页** | 任务卡片列表、快捷功能（游戏介绍/项目素材/AE工程/任务列表） | L266 |
| **任务页** | 双视图模式（树形分类 vs 名称排序）、素材展示 | L296 |
| **游戏介绍页** | 辅助页面，展示 00_Game Design & Doc | L643 |
| **项目素材页** | 辅助页面，展示 01/02/04/05 辅助素材 | L686 |
| **设置页** | 应用设置（工作流/翻译/日报打卡/通用，含预览帧率+透明背景） | L751 |
| **日报打卡** | Tauri WebView自动化打卡、提醒弹窗、定时任务、配置结构 | L874 |
| **翻译功能** | 全局快捷键呼出置顶窗口、Gemini API、双语自动检测 | L1261 |
| **毛玻璃/悬浮岛** | 视觉设计规范 | L1492 |
| **响应式设计** | 最小窗口1280×720、溢出策略 | L1511 |
| **侧边栏详情** | 素材详情侧边栏 | L1536 |
| **侧边栏操作按钮** | 重命名/删除（通用）、打开工程文件/帧速率修改（序列帧） | L1771 |
| **交互设计** | 页面导航、双视图切换、文件拖拽 | L1779 |
| **待明确问题** | 功能/视觉/交互待定项 | L1812 |

### 设计系统 → `design/DesignSystem.md`（1355行）

UI 规范的 SSOT，所有视觉参数在这里定义。

| 主题 | 章节 | 行号 |
|------|------|------|
| **颜色系统** | 基础色板、主题模式（明/暗）、语义色、透明度 | L32 |
| **间距系统** | 8px 基准、语义化间距 | L292 |
| **圆角系统** | 各级圆角值 | L336 |
| **毛玻璃效果** | 五级预设（轻/中/重/深/特深）+ CSS 示例 | L358 |
| **排版系统** | 字体（猫啃网糖圆体）、字号、行高、字重 | L454 |
| **阴影系统** | 阴影预设、悬停增强 | L550 |
| **动画系统** | 过渡时长、缓动函数 | L581 |
| **组件规范** | 按钮(4变体)、卡片、标签、输入框、悬浮岛、侧边栏、窗口控制 | L631 |
| **响应式断点** | | L962 |
| **Z-Index层级** | | L979 |
| **主题切换实现** | CSS变量 + Tauri窗口配置 | L1152 |

### 文件整理规则 → `design/文件命名与组织规则.md`（2048行）

业务逻辑的核心，定义项目结构和文件工作流。

| 主题 | 章节 | 行号 |
|------|------|------|
| **项目目录结构** | 六大目录（00~05），各目录职责 | L8 |
| **核心工作目录** | 03_Render_VFX/VFX/ 结构，功能分类列表 | L91 |
| **项目表单系统** | 全局任务清单+项目启用、新建项目流程、归档/时光机 | L215 |
| **Export工作流** | 四级子目录（00_original → 01_scale → 02_done → 03_preview） | L479 |
| **详细工作流** | 步骤1~5：原始导出→规范化→缩放→转换→完成→预览 | L579 |
| **格式转换** | 静帧(imagine) + 序列帧(TexturePacker) 工作流 | L705 |
| **素材进度追踪** | 进度链条、状态定义、判定逻辑、断裂检测 | L1460 |
| **文件命名规则** | 命名格式、排序逻辑、序列帧命名 | L1692 |
| **缩放比例档位** | 100%/70%/50%/40%，目录命名 | L1803 |
| **TexturePacker** | 输出规则 | L1828 |
| **预览视频命名** | 格式、版本号、breakdown | L1847 |
| **文件类型总结** | 静帧/序列帧/视频/文档/源文件 | L1899 |

### 任务系统 → `design/任务系统.md`（354行）

任务管理的业务逻辑。

| 主题 | 章节 | 行号 |
|------|------|------|
| **任务架构** | 树状结构（父任务→子任务→叶子任务） | L19 |
| **任务模板** | 模板定义、默认模板、管理功能 | L64 |
| **子任务管理** | 创建位置、显示方式、点击行为 | L117 |
| **进度判定** | 检测触发时机、判定规则、状态背景色 | L188 |
| **任务管理窗口** | 触发方式、窗口类型、操作行为 | L257 |

### 卡片设计 → `design/卡片设计.md`（389行）

四种卡片的视觉规范。

| 主题 | 章节 | 行号 |
|------|------|------|
| **项目卡片** | ICON+项目名+截止日期+进度条 | L23 |
| **任务卡片** | 预览图+任务名+子任务状态+进度 | L57 |
| **素材卡片** | 缩略图+文件名+类型标签+上传状态 | L104 |
| **普通卡片** | 辅助页面用 | L182 |
| **预览图生成** | 数据源规则、序列帧预览规范 | L217 |
| **通用交互** | 悬停、拖拽 | L326 |

### 自动化识别 → `design/自动化识别规则.md`（638行）

项目扫描、导入、刷新的算法逻辑。

| 主题 | 章节 | 行号 |
|------|------|------|
| **新项目创建** | 标准模板+任务勾选+自动创建文件夹 | L37 |
| **旧项目导入** | 自动扫描+结构匹配+任务识别 | 中段 |
| **刷新机制** | 切换页面/失焦回归/手动刷新 | 中段 |
| **识别规则** | 文件类型判定、目录结构解析 | 中段 |
| **异常处理** | 不合规文件、缺失目录 | 末段 |

### 打包发布 → `docs/打包发布指南.md`

给 Agent 的完整打包手册，**打包前必读**。

| 主题 | 内容 |
|------|------|
| **打包命令** | `npm run tauri build`，输出 NSIS `.exe` |
| **版本号更新** | 三处同步：`package.json` / `tauri.conf.json` / `Cargo.toml` |
| **图标管理** | `npx tauri icon icon.ico` 重新生成 |
| **安装程序图片** | BMP 格式，header 150×57，sidebar 164×314 |
| **CSP 配置** | media-src 必须单独声明；img-src 需含 data:；不要用 WiX（中文名会崩） |
| **覆盖安装** | 直接运行新版 .exe，用户数据不丢 |

---

### Prototype 特例 → `design/Prototype特例规则.md`（347行）

Prototype 功能分类的特殊处理（比普通分类多一层子分类）。

| 主题 | 章节 | 行号 |
|------|------|------|
| **特殊性说明** | 与普通分类的结构差异 | L18 |
| **7个固定子分类** | big_win / infoboard / loading_bonus / main_ui / spinbutton / symbol / total_win | 前段 |
| **nextcloud处理** | 保留子分类 + _original 目录 | 中段 |

---

## 关键决策速查

| 决策 | 结论 | 背景 |
|------|------|------|
| **打包格式** | NSIS only（`"targets": "nsis"`） | WiX 3 不支持中文 productName，会直接报错 |
| **关闭行为** | 最小化到系统托盘，不退出 | lib.rs CloseRequested 拦截 + hide() |
| **UI 缩放** | 固定值，默认 100%，无自动模式 | 自动模式（按窗口宽度缩放）效果差，已移除 |
| **软件名称** | PG素材管理系统，V2.0.0，开发者 Fuchikami | `src/config/app.ts` 为 SSOT |
| **CSP media-src** | 必须单独声明，含 asset:／blob:／data: | `<video>` 不继承 img-src，缺失打包后视频全灭 |
| 技术栈 | Tauri 2.x（不用 Electron） | Electron 不支持拖拽文件到外部浏览器 |
| UI 风格 | 毛玻璃（Glassmorphism），明暗双主题 | — |
| 自定义字体 | 猫啃网糖圆体 | `design/DesignSystem.md` L456 |
| 最小窗口 | 1280×720 | `design/界面设计.md` L1513 |
| 面包屑导航 | 不要 | 用返回按钮逐级返回 |
| 右键菜单 | 不要（快捷方式栏例外） | — |
| 刷新快捷键 | 不要 | 自动刷新 + 更多菜单手动刷新 |
| 悬浮窗（主窗口） | 暂不实现 | 2026-02-17 决定精简掉 |
| 转换进度悬浮窗 | 保留（独立小窗口显示进度+复制路径） | `design/界面设计.md` L497 |
| 前端框架 | Vue 3 | Tauri 官方支持、单文件组件适合界面密集型项目 |
| 截止日期 | 创建项目时输入，后续可修改 | `design/卡片设计.md` L44 |
| 撤销功能 | 不需要 | — |
| 预览视频版本 | 标记最新版本，不做历史管理 | `design/文件命名与组织规则.md` L1389 |
| 素材重命名 | 改基础名，所有版本同步改名 | `design/界面设计.md` - 侧边栏底部操作按钮 |
| 打卡逻辑 | 不做工作日/假期判断，所有弹窗有跳过按钮；三档 mode（off/auto/record_only）+ 日报独立 enabled | `design/界面设计.md` L874 |
| 翻译模式 | 只有双语自动检测，固定"简洁阳光亲切"风格 | `design/界面设计.md` L1261 |
| 翻译快捷键 | 支持计算器键（Calculator key） | — |

---

## 待明确事项（开发阶段解决）

- [x] ~~名称排序视图的卡片~~ — 与树形视图用同一种素材卡片，无需额外字段
- [x] ~~辅助文件展示方式~~ — 游戏介绍页和项目素材页都使用普通卡片（`卡片设计.md` - 普通卡片）
- [x] ~~任务管理窗口类型~~ — 模态弹窗
- [ ] 任务系统剩余细节（叶子任务状态标签、模板JSON结构、子任务确认弹窗样式） → 开发阶段解决
- [x] ~~UI 高保真原型~~ — 跳过，直接进开发
- [x] ~~悬浮窗~~ — 暂不实现

---

## 会话与历史

| 文档 | 内容 |
|------|------|
| `sessions/project_history.md` | 项目时间线、各阶段关键决策、里程碑 |
| `sessions/session_2026-02-13.md` | 启动重构、Tauri选型、双视图、核心功能 |
| `sessions/session_2026-02-14.md` | 三级页面结构、菜单设计 |
| `sessions/session_2026-02-15.md` | 任务系统深化、卡片规范、文档模块化 |
| `sessions/session_2026-02-16.md` | 转换功能、DesignSystem、自定义字体 |
| `sessions/session_2026-02-17.md` | 日报打卡、翻译功能、快捷方式栏、悬浮窗精简 |
| `sessions/session_2026-02-21.md` | 全局框选多选：useRubberBandSelect composable、五页面接入、GameIntro/Materials补齐多选按钮、TDZ Bug修复；UI hover Bug修复；任务完成判定加入预览视频上传状态（video_total/video_uploaded）：scan_tasks/scan_projects/TaskCard/TaskPage/子任务弹窗全链路更新；**副标题行固定滚动架构**：MainLayout page-wrapper height:100% + main-content overflow-y:hidden；HomePage 补齐 scroll-content 包装 |
| `sessions/session_2026-02-22.md` | **v2.2.0** — 视频版本管理修复：子版本号 `_7.1` 支持（regex_strip_version/extract_version_number 元组化）；前端 groupPreviewVideos 数字排序修复 `_9>_10`；outdated 橙色"需更新"标签（design-system 语义变量）；NC 上传旧版本自动清理；子任务弹窗时机修复（confirmPreviewUpload/toggleSubtaskCompletion 补检测、mark_upload_prompted 内存同步、visibilitychange 窗口回焦刷新） |

---

## 旧系统参考

| 系统 | 路径 | 技术栈 | 对应新功能 |
|------|------|--------|-----------|
| PG DINGDONG（打卡） | `D:\work\pgsoft\PG DINGDONG` | Python+PyQt6+Selenium | 日报打卡（Tauri WebView） |
| translator（翻译） | `D:\work\pgsoft\translator` | Electron+Gemini API | 翻译功能（全局快捷键窗口） |

---

**最后更新**：2026-02-22

本次会话（v2.2.0 — 视频版本管理与弹窗修复）：
- **子版本号支持**：`regex_strip_version` / `extract_version_number` 支持 `_7.1` 格式，返回 `(u32, u32)` 元组比较
- **前端分组修复**：`groupPreviewVideos` 正则 `/_\d+(\.\d+)?$/`，排序从 `localeCompare` 改数字比较（修复 `_9 > _10`）
- **outdated 橙色标签**：design-system 新增 `--progress-outdated` / `--tag-progress-outdated-bg` 语义变量；文案 `待更新` → `需更新`
- **NC 旧版本自动清理**：`copy_preview_to_nextcloud` 复制前删除同组旧版本文件
- **弹窗时机修复**：`confirmPreviewUpload` / `toggleSubtaskCompletion` 补 `checkSubtaskAutoPrompt()`；`mark_upload_prompted` 后同步内存 `upload_prompted_tasks`（修复 stale read）；新增 `visibilitychange` 窗口回焦自动刷新

之前会话（v2.1.0 — UI 优化）：
- **TitleBar 返回按钮**：箭头 SVG 从 20→40px；leave 动画 `position:absolute` 导致 `align-self:stretch` 失效向上跳动，加 `top:0; bottom:0` 修复
- **ProjectCard 菜单**：`<Teleport to="body">` 脱离父级 `glass-subtle` 合成层，使 `glass-medium` 毛玻璃生效；`position: fixed` + 动态坐标；`z-index: var(--z-dropdown)`；菜单样式移至全局 `<style>` 块
- **StatusBar 配置面板**：加 `<Transition name="config-panel">` 进出场动画（`translateY(-6px) scale(0.95)` + opacity）

之前会话（打包发布阶段）：
- 软件名改为「PG素材管理系统」V2.0.0，开发者 Fuchikami；新增 `src/config/app.ts` 集中管理；设置页加「关于」tab
- Git 初始化，初始提交 + tag v2.0.0
- 系统托盘：关闭按钮改为隐藏到后台，托盘左键恢复，右键菜单（显示窗口/退出）
- 图标：从 `icon.ico` 生成全平台尺寸
- 打包：NSIS 目标，中文界面，免 UAC，自定义 header/sidebar BMP 图片
- 修复：hotkey.rs release 模式翻译窗口 URL 指向 index.html（应为 /translator）
- 修复：CSP 缺少 media-src 导致打包后视频/视频缩略图全部失效
- 移除 UI 自动缩放模式，默认固定 100%
- 新增 `docs/打包发布指南.md`

之前会话（2026-02-21）：Sidebar 视觉优化；ShortcutDialog 图标预览；useRubberBandSelect 全局框选；TitleBar flipWidth Bug 修复；Prototype Bug 修复；游戏介绍页「启动原型」功能；**副标题行固定滚动架构修复**：MainLayout `.page-wrapper { height:100% }` + `.main-content { overflow-y:hidden }`（原 min-height:100% + overflow-y:auto 导致玻璃面板自滚，页面内部 flex 滚动分区失效）；HomePage 补齐 `.scroll-content` 包装
