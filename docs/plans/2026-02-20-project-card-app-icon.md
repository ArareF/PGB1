# Project Card AppIcon 实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 项目卡片从 `01_Preproduction/` 自动识别名字含 `appicon` 的文件（PNG 优先，其次 PSD/PSB），用其作为项目卡片的 ICON 缩略图，替代现有固定 SVG 占位符。

**Architecture:** Rust 端 `scan_projects` 扫描时顺带查找 appicon 文件，通过 `ProjectInfo.app_icon` 字段（Option<String>，文件绝对路径）传回前端。前端 `ProjectCard.vue` 根据文件扩展名决定渲染方式：PNG 直接 `convertFileSrc` 显示，PSD/PSB 走已有的 `usePsdThumbnail` 异步加载，无文件时降级为现有 SVG 占位符。

**Tech Stack:** Rust（fs 遍历）、Vue 3、`@tauri-apps/api/core convertFileSrc`、`usePsdThumbnail` composable（已有）

---

### Task 1：Rust 端——models.rs 加字段

**Files:**
- Modify: `src-tauri/src/models.rs`（ProjectInfo struct，第 47 行附近）

**Step 1：在 ProjectInfo 末尾加 `app_icon` 字段**

在 `pub default_ae_file: Option<String>,` 下方（第 47 行），`}` 之前，插入：

```rust
    /// AppIcon 文件的绝对路径（来自 01_Preproduction/，名含 appicon，优先 PNG 其次 PSD/PSB）
    pub app_icon: Option<String>,
```

**Step 2：验证编译通过**

```bash
cd src-tauri && cargo check 2>&1 | head -30
```

预期：有编译错误——`scan_projects` 和 `create_project` 构造 `ProjectInfo` 时缺少字段。这是正常的，下一个 Task 修复。

---

### Task 2：Rust 端——commands.rs 实现查找逻辑

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1：在 `scan_projects` 函数中，`projects.push(ProjectInfo { ... })` 之前（第 80 行附近），插入查找 appicon 的逻辑**

```rust
        // 查找 01_Preproduction/ 下名字含 appicon 的文件（大小写不敏感）
        let app_icon = find_app_icon(&path.join("01_Preproduction"));
```

**Step 2：在 `projects.push(ProjectInfo { ... })` 的字段列表中加入 `app_icon`**

在 `default_ae_file: config.default_ae_file,` 下方加：
```rust
            app_icon,
```

**Step 3：在 `create_project` 返回的 `ProjectInfo` 中加 `app_icon: None`**

找到 `create_project` 函数里的 `Ok(ProjectInfo { ... })` 块（第 2896 行附近），在 `default_ae_file: None,` 之后加：
```rust
            app_icon: None,
```

**Step 4：在文件末尾（或 `scan_projects` 附近）添加辅助函数 `find_app_icon`**

```rust
/// 在指定目录下查找名字含 "appicon"（大小写不敏感）的文件
/// 优先返回 PNG，其次返回 PSD/PSB，都没有则返回 None
fn find_app_icon(preproduction_dir: &Path) -> Option<String> {
    let entries = fs::read_dir(preproduction_dir).ok()?;

    let mut png_candidate: Option<PathBuf> = None;
    let mut psd_candidate: Option<PathBuf> = None;

    for entry in entries.flatten() {
        let file_path = entry.path();
        if !file_path.is_file() {
            continue;
        }
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !file_name.contains("appicon") {
            continue;
        }

        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "png" => {
                if png_candidate.is_none() {
                    png_candidate = Some(file_path);
                }
            }
            "psd" | "psb" => {
                if psd_candidate.is_none() {
                    psd_candidate = Some(file_path);
                }
            }
            _ => {}
        }
    }

    png_candidate
        .or(psd_candidate)
        .map(|p| p.to_string_lossy().to_string())
}
```

**Step 5：验证编译通过**

```bash
cd src-tauri && cargo check 2>&1 | head -30
```

预期：无错误（允许有 warning）。

---

### Task 3：前端——useProjects.ts 同步 ProjectInfo 接口

**Files:**
- Modify: `src/composables/useProjects.ts`

**Step 1：在 `ProjectInfo` 接口末尾加 `app_icon` 字段**

在 `default_ae_file: string | null` 之后加：
```typescript
  app_icon: string | null
```

**Step 2：验证 TypeScript 无类型错误**

```bash
npx vue-tsc --noEmit 2>&1 | head -30
```

预期：无新增 TS 错误。

---

### Task 4：前端——ProjectCard.vue 渲染 AppIcon

**Files:**
- Modify: `src/components/ProjectCard.vue`

**Step 1：在 `<script setup>` 顶部加 import**

在现有 `import { computed } from 'vue'` 之后加：
```typescript
import { ref, onMounted, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { usePsdThumbnail } from '../composables/usePsdThumbnail'
```

> 注意：`computed` 已经 import，不要重复，合并成：
> `import { computed, ref, onMounted, watch } from 'vue'`

**Step 2：在 `progressPercent` computed 之后，`</script>` 之前，加图标加载逻辑**

```typescript
// AppIcon 渲染逻辑
const iconSrc = ref<string | null>(null)
const { getPsdThumbnail } = usePsdThumbnail()

async function loadIcon() {
  const iconPath = props.project.app_icon
  if (!iconPath) {
    iconSrc.value = null
    return
  }
  const ext = iconPath.split('.').pop()?.toLowerCase() ?? ''
  if (ext === 'png') {
    iconSrc.value = convertFileSrc(iconPath)
  } else if (ext === 'psd' || ext === 'psb') {
    iconSrc.value = await getPsdThumbnail(iconPath, 128)
  } else {
    iconSrc.value = null
  }
}

onMounted(loadIcon)
watch(() => props.project.app_icon, loadIcon)
```

**Step 3：修改 `<template>` 中的 `.card-icon` 区域**

将现有：
```html
    <!-- 左侧 ICON 占位 -->
    <div class="card-icon">
      <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
      </svg>
    </div>
```

替换为：
```html
    <!-- 左侧 ICON -->
    <div class="card-icon">
      <img
        v-if="iconSrc"
        :src="iconSrc"
        class="card-icon-img"
        alt=""
      />
      <svg
        v-else
        width="32"
        height="32"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
      </svg>
    </div>
```

**Step 4：在 `<style scoped>` 中加 `.card-icon-img` 样式**

在 `.card-icon { ... }` 块之后加：
```css
.card-icon-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  border-radius: var(--radius-md);
}
```

**Step 5：启动 dev 验证效果**

```bash
npm run tauri dev
```

验证清单：
- [ ] 有 AppIcon PNG 的项目：卡片左侧显示图片缩略图
- [ ] 有 AppIcon PSD 的项目：卡片左侧异步加载 PSD 缩略图（加载中短暂显示 SVG 占位）
- [ ] 无 AppIcon 的项目：卡片左侧显示原有文件夹 SVG 占位
- [ ] 两个主题（明/暗）下图标显示正常

---

## 注意事项

- `usePsdThumbnail` 的 `getPsdThumbnail` 返回值是 `string | null`，PSD 加载失败时返回 `null`，此时 `iconSrc` 保持 `null`，自动降级显示 SVG 占位——防御性已覆盖，无需额外处理。
- `find_app_icon` 只扫描 `01_Preproduction/` 一层（`read_dir` 非递归），符合实际文件放置习惯，不做深层扫描。
- 项目没有 `01_Preproduction/` 目录时，`fs::read_dir` 返回 `Err`，`ok()?` 直接返回 `None`，安全。
