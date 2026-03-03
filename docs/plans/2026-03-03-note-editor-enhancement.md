# 笔记编辑器增强设计方案

> 日期：2026-03-03
> 状态：已定稿，待实施

## 目标

在现有纯 textarea 笔记系统上增加轻量渲染能力。核心：**链接可点击跳转外部浏览器**。辅助：checklist 可交互勾选、基础粗体/斜体。重度排版交给用户自定义网页链接。

## 约束

- 存储格式不变（纯字符串，`.pgb1_notes.json`）
- Rust 端零改动
- 零新依赖，自写渲染函数
- 只支持 4 种语法子集，不做完整 markdown

## 交互流程

### 默认 = 渲染视图
- NoteDialog（弹窗）和 FileDetailSidebar（侧边栏）行为一致
- 打开后看到渲染结果：链接蓝色可点击、checkbox 可勾选、粗体/斜体已渲染
- 空笔记时直接进编辑模式（没内容看什么渲染）

### 编辑模式
- 点「编辑」按钮切到 textarea
- 出现迷你工具栏 + textarea + 进度条
- 点「保存」或失焦 → 保存并切回渲染视图

### Tooltip
- 剥离 markdown 语法符号后取前 39 字符纯文本，行为不变

## 支持的语法

| 语法 | 渲染结果 | 交互 |
|------|---------|------|
| `**文字**` | **粗体** | 纯展示 |
| `*文字*` | *斜体* | 纯展示 |
| `https://example.com` | 蓝色链接 | 点击 → `shell.open()` 外部浏览器 |
| `- [ ] 待办` / `- [x] 已完成` | checkbox + 文字 | 点击 → 切换状态，自动保存 |

不支持：标题、列表（非 checklist）、代码块、图片、表格。

## 迷你工具栏

4 个按钮，点击插入语法模板到 textarea 光标位置：

| 按钮 | 图标 | 点击行为 |
|------|------|---------|
| 粗体 | **B** | 选中文字 → 包裹 `**选中**`；未选中 → 插入 `**粗体文字**` 并选中占位文字 |
| 斜体 | *I* | 同上，`*斜体文字*` |
| 链接 | 🔗 | 插入 `https://` 并选中 |
| 清单 | ☑ | 当前行首插入 `- [ ] `，光标留末尾 |

操作细节：插入后自动 focus 回 textarea，通过 `selectionStart`/`selectionEnd` 操控光标和选区。

## 组件架构

### 新增
- **NoteRenderer.vue**：接收 markdown 字符串，输出渲染后 HTML。处理链接点击（`shell.open`）、checkbox 勾选（emit 事件）

### 改造
- **NoteEditor.vue**：增加 `mode: 'render' | 'edit'` 双模式。渲染模式 = NoteRenderer + 编辑按钮；编辑模式 = 工具栏 + textarea + 进度条 + 保存按钮。空内容自动进编辑模式
- **NoteDialog.vue**：适配双模式。渲染模式底部「关闭」；编辑模式底部「保存/取消」
- **NoteTooltip.vue**：调用 `stripMarkdown()` 剥离语法符号后截取

### useNotes.ts 新增
- `stripMarkdown(text): string` — 剥离 `**`、`*`、`- [ ] `、`- [x] ` 前缀，URL 保留原文
- `toggleCheckbox(text, lineIndex): string` — 切换指定行 `[ ]` ↔ `[x]`，返回新文本

### 渲染函数
- 自写 ~60 行，逐行处理
- URL 匹配：`https?://\S+`
- XSS 防御：`textContent` 拼接 DOM 节点，不直接 innerHTML 用户输入

## i18n 新增 key

- `note.edit` — 编辑
- `note.close` — 关闭（渲染模式下的弹窗按钮）
- 工具栏 tooltip：`note.toolbar.bold`、`note.toolbar.italic`、`note.toolbar.link`、`note.toolbar.checklist`
