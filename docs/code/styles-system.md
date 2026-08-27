# 样式系统详情

> `src/styles/` 下 4 个 CSS 文件的 SSOT 变量、毛玻璃层级、动画系统、公共类清单。
> 顶层索引见 [CODE_INDEX.md](../../CODE_INDEX.md#6-样式)。

---

## design-system.css（1590 行）—— SSOT

**地位**：所有视觉参数的唯一真相源。组件只用 `var(--*)`，严禁硬编码。

### 颜色系统

- 冷科技蓝色板 + 冷蓝灰中性色
- 暗色主题 v2.0 冷色工业终端风格
- **深色模式 `--text-tertiary`**：`#6B6E77`（原 `#4A4D54` 对比度仅 2.2:1，已提升至 3.8:1）

### 设计语言

- 间距系统：8px 基准 + 语义化
- 排版：URW DIN + 更纱黑体（中文自定义字体：猫啃网糖圆体）
- 圆角：工业风收窄
- 卡片、标签、过渡规范
- 毛玻璃变量（5 级预设）
- **弱化 Material 阴影**，改用透明度 + 冷蓝边框拉层级
- **`--card-version-badge-*`**（v2.8.18）：素材系列合并卡右上角版本数角标的底色 / 文字色 / 边框色

### 新增 token

- `--overlay-backdrop`（弹窗遮罩）
- `--canvas-bg`（Canvas 背景）

### Hover SSOT

- `--shadow-card-hover`：含 ring 光晕 `0 0 0 1px rgba(100,180,255,0.30)` 覆盖所有卡片
- `--card-hover-lift`：`-3px`

### 优先度 token

- 菜单胶囊：`--priority-{h/m/l}-{bg/text/active}`（半透明）
- 卡片圆点：`--priority-{high/medium/low}-dot`（= `color-danger` / `warning` / `success` 纯实色）

### 标题样式

- `.section-label` + `.group-label`：大分组/小组标题样式

### TransitionGroup FLIP 动画

- `.card-move { transition: transform ... }` 使所有 `name="card"` 分组在排序时平滑位移

### 公共类清单

| 公共类 | 作用 |
|--------|------|
| `.folder-btn` | 28×28 图标按钮，透明背景，hover 蓝色 wash |
| `.view-btn` | 视图切换公共样式 |
| `.sort-tab` | 排序标签页公共样式 |
| `.control-panel` | 控制面板公共类（ScalePage / ConvertPage 共用） |
| `.sidebar-actions` / `.sidebar-action-btn` | 侧边栏底部悬浮操作按钮（TaskPage + FileDetailSidebar 共用），`.danger` 变体 hover 红色 |
| `.sidebar-dialog-overlay` / `.sidebar-dialog` / `.sidebar-dialog-*` | 侧边栏内联弹窗 |
| `.note-rendered` | 笔记渲染视图容器（链接 / checkbox / 粗体 / 斜体样式） |
| `.note-toolbar` / `.note-toolbar-btn` | 笔记工具栏 4 按钮 |
| `.note-edit-btn` | 笔记渲染视图右上角编辑按钮 |
| `.card-checkbox-shared` | 多选三件套公共 checkbox |
| `.priority-dot--{high/medium/low}` | 9px 纯色实心圆优先度指示 |
| `.priority-pill--{high/medium/normal/low}` | 优先度菜单胶囊 |

---

## glass.css（86 行）—— 毛玻璃工具类

**提供类**：`.glass-subtle` / `.glass-medium` / `.glass-strong`

### 关键架构决策（顶部注释）

> **backdrop-filter 兄弟冲突规则**
>
> 同层 flex 兄弟只能有一个带 `backdrop-filter`，其余必须手动 `background / border / box-shadow`。
>
> 原因：多个兄弟同时 `backdrop-filter` 会各自创建独立合成层，相邻层之间产生黑色伪影或完全失效。

### 技术细节

- `overflow: clip`（非 `hidden`）：避免 ink overflow 被裁切
- `::after` 噪点层 `z-index: -1`：不强制子元素 `z-index`
- 五级玻璃预设值在 `design-system.css` 中定义

### 手动 glass 实战清单（触发冲突的位置）

| 位置 | 原因 |
|------|------|
| TitleBar 左岛 | 与 center 岛相邻 |
| MainLayout 更多菜单按钮 | 与 TitleBar glass-medium 视觉重叠会产生黑色伪影 |
| MainLayout 更多菜单下拉 | 手动 `--dropdown-menu-bg` 高不透明度 |
| Sidebar 左侧栏 | 与 main-content 相邻 |
| ScalePage / ConvertPage 控制面板 | Teleport 到 `#content-row`，与 main-content 同层兄弟 |
| TaskPage sidebar-dialog | 侧边栏内的弹窗与 main-content 同层 |
| MaterialCard / NormalCard | 大量卡片避免各自创建合成层 |
| TaskPage sidebar-action-btn | 侧边栏底部按钮 |

---

## dialog.css（120 行）—— 弹窗公共样式

**从多个 Dialog 组件提取统一**。

### 结构类

| 类 | 作用 |
|------|------|
| `.dialog-overlay` | 遮罩 + 居中布局 |
| `.dialog-content` | 玻璃面板 |
| `.dialog-title` | 标题区 |
| `.dialog-body` | 内容区 |
| `.dialog-actions` | 底部按钮区 |
| `.dialog-btn` | 按钮变体（primary / danger / ghost） |

### 进出场动画

- 遮罩 `opacity` 过渡
- 内容 `translateY` + `scale` + `opacity`
- 统一时长 + 缓动函数

---

## reset.css（45 行）

**基础重置**：
- 字体引用 `var(--font-family-base)`
- 根字号 14px
- box-sizing / margin / padding 规范化

---

## 动画系统速查

| 动画 | 实现 | 应用位置 |
|------|------|---------|
| 卡片交错入场 | `<TransitionGroup name="card">` | HomePage / ProjectPage / GameIntroPage / MaterialsPage 等 |
| FLIP 排序 | `.card-move { transition: transform }` | 所有带 `<TransitionGroup>` 的列表 |
| 路由切换方向感知 | `<Transition name="page-forward/back" mode="out-in">` | MainLayout |
| 下拉菜单 | `<Transition name="dropdown">` | MainLayout 更多菜单 |
| 卡片菜单 | `<Transition name="card-menu">` | ProjectCard 下拉菜单 |
| 导航按钮滑入 | `<Transition name="nav-forward/back">` | TitleBar 标题/返回按钮/操作区 |
| 配置面板 | `<Transition name="config-panel">` | StatusBar 番茄钟配置 |
| 弹窗进出场 | translateY + scale + opacity | 所有 Dialog 组件 |
| 工具提示 | `<Transition name="tooltip">` | NoteTooltip |
| JS FLIP 宽度 | `watch flush:pre/post` + `flipWidth` | TitleBar 岛宽动画 |
| Sidebar 拖拽排序 FLIP | `<TransitionGroup name="sort">` + `.sort-move` | Sidebar 编辑模式 |
| Loading 淡入 | `<Transition name="fade">` | TaskPage loading overlay |
| 呼吸动画 | `breathe` keyframes 0.35↔1 | TranslatorPage 等待动画 |
| Sidebar 编辑抖动 | 6 个不规则关键帧 + `nth-child(2n/3n/4n)` 错相 | Sidebar iOS 风格编辑模式 |

---

## Z-Index 层级

见 `design/DesignSystem.md` L979 的完整定义。关键层级：
- `--z-dropdown`：下拉菜单（ProjectCard 菜单、StatusBar 配置面板）
- 侧边栏 > 弹窗遮罩 > 全屏浮层

---

## 主题切换

**实现**：CSS 变量 + Tauri 窗口配置。见 `design/DesignSystem.md` L1152。

**运行时切换**：`useTheme.initTheme()` / `toggleTheme()` 通过切换 `data-theme` 属性触发 CSS 变量重新求值，localStorage 持久化。
