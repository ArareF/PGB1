# PDF 预览功能实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 NormalCard（卡片缩略图）和 FileDetailSidebar（侧边栏详情）中支持 PDF 文件预览

**Architecture:** 前端新增 `pdf` 文件类型分支。NormalCard 显示红色 PDF 图标（WebView2 无法在 canvas 截帧）。FileDetailSidebar 用 `<iframe :src="convertFileSrc(file.path)">` 直接渲染 PDF，WebView2 内置 PDF 渲染引擎零依赖。需修改 tauri.conf.json 的 CSP 添加 `frame-src` 允许 asset:// 协议。

**Tech Stack:** Vue 3, Tauri 2.x (WebView2), convertFileSrc (asset:// 协议)

---

### Task 1: 修复 CSP，允许 iframe 加载 asset:// 资源

**Files:**
- Modify: `src-tauri/tauri.conf.json`

**背景：** 当前 CSP 缺少 `frame-src`，iframe 加载 `asset://` 协议的 PDF 会被浏览器安全策略拦截。

**Step 1: 修改 tauri.conf.json 的 CSP 字段**

在现有 CSP 字符串末尾追加 `frame-src 'self' asset: http://asset.localhost;`

当前值：
```
"default-src 'self' ipc: http://ipc.localhost https:; img-src 'self' asset: http://asset.localhost blob: https:; script-src 'self' 'unsafe-inline' https:; style-src 'self' 'unsafe-inline' https:; connect-src 'self' https:"
```

修改后：
```
"default-src 'self' ipc: http://ipc.localhost https:; img-src 'self' asset: http://asset.localhost blob: https:; script-src 'self' 'unsafe-inline' https:; style-src 'self' 'unsafe-inline' https:; connect-src 'self' https:; frame-src 'self' asset: http://asset.localhost"
```

**Step 2: 验证 JSON 格式正确**

用文本编辑器确认整个 JSON 结构无语法错误（没有多余逗号/引号不匹配）。

---

### Task 2: NormalCard.vue 新增 PDF 图标支持

**Files:**
- Modify: `src/components/NormalCard.vue`

**背景：** PDF 文件无法像视频那样截帧，也无法像 PSD 那样提取缩略图。卡片上显示醒目的红色 PDF 图标即可，与其他"普通文件"图标区分。

**Step 1: 在 script 区新增 PDF 类型检测**

在 `PSD_EXTS` 定义之后添加：
```ts
const PDF_EXTS   = new Set(['pdf'])
const isPdf      = computed(() => !props.file.is_dir && PDF_EXTS.has(props.file.extension))
```

**Step 2: 在 template 的 card-preview 区新增 PDF 分支**

在 `v-else-if="isPsd"` 的整个 img + div 块之后，`v-else-if="file.is_dir"` 之前，插入：

```html
<!-- PDF：红色图标 -->
<div v-else-if="isPdf" class="pdf-icon">
  <svg width="36" height="36" viewBox="0 0 36 36" fill="none">
    <rect width="36" height="36" rx="6" fill="#CC0000"/>
    <text x="18" y="25" font-family="sans-serif" font-size="12" font-weight="700" fill="#FFFFFF" text-anchor="middle">PDF</text>
  </svg>
</div>
```

**Step 3: 在 style 区新增 .pdf-icon 样式**

```css
.pdf-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0.85;
}
```

**Step 4: 手动验证**

启动应用（`npm run start`），进入游戏介绍页或项目素材页，确认 PDF 文件卡片显示红色 PDF 图标，而不是通用文件图标。

---

### Task 3: FileDetailSidebar.vue 新增 PDF iframe 预览

**Files:**
- Modify: `src/components/FileDetailSidebar.vue`

**背景：** WebView2（Chromium 内核）原生支持 PDF 渲染，`<iframe>` 指向 `convertFileSrc(file.path)` 即可显示完整 PDF，支持缩放、翻页等浏览器内置功能。

**Step 1: 新增 PDF 扩展名常量**

在 `PSD_EXTS` 定义之后添加：
```ts
const PDF_EXTS   = ['pdf']
```

**Step 2: 修改 fileType 计算属性**

在 `if (PSD_EXTS.includes(ext)) return 'psd'` 之后添加：
```ts
if (PDF_EXTS.includes(ext))   return 'pdf'
```

**Step 3: 在 template 新增 PDF 预览分支**

在 `<!-- PSD/PSB 预览 -->` 块之后，`<!-- 其他：文件类型图标 -->` 之前，插入：

```html
<!-- PDF 预览 -->
<div v-else-if="fileType === 'pdf'" class="preview-pdf-wrap">
  <iframe
    :key="file.path"
    :src="convertFileSrc(file.path)"
    class="preview-pdf-frame"
    frameborder="0"
  />
</div>
```

**Step 4: 在 style 区新增 PDF 相关样式**

在 `/* ─── PSD 预览 ─── */` 块之后添加：

```css
/* ─── PDF 预览 ─── */
.preview-pdf-wrap {
  width: 100%;
  flex: 1;
  min-height: 400px;
  border-radius: var(--radius-lg);
  overflow: hidden;
  flex-shrink: 0;
}

.preview-pdf-frame {
  width: 100%;
  height: 100%;
  min-height: 400px;
  border: none;
  display: block;
  border-radius: var(--radius-lg);
}
```

**Step 5: 手动验证**

1. 启动应用，进入游戏介绍页
2. 点击一个 PDF 文件，确认侧边栏出现 PDF 内容预览（而不是文件图标）
3. 确认可以在 iframe 内滚动翻页
4. 切换到另一个 PDF 文件，确认内容正确切换（:key 绑定确保 iframe 刷新）

---

### Task 4: CODE_INDEX.md 更新

**Files:**
- Modify: `CODE_INDEX.md`

**Step 1: 更新 NormalCard.vue 的描述**

将：
```
PSD/PSB 文件调用 `usePsdThumbnail`（256px）异步加载真实缩略图，失败降级为 PS 图标
```
改为：
```
PSD/PSB 文件调用 `usePsdThumbnail`（256px）异步加载真实缩略图，失败降级为 PS 图标；PDF 文件显示红色 PDF 图标
```

**Step 2: 更新 FileDetailSidebar.vue 的描述**

在 `**PSD/PSB**（`usePsdThumbnail` 800px 高清缩略图 + 「用 Photoshop 打开」按钮）` 之后添加：
```
**PDF**（iframe 直接渲染，WebView2 内置 PDF 引擎）
```
