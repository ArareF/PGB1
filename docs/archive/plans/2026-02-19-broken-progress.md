# 断裂链条检测实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为素材进度判定新增 `broken`（断裂）状态，当检测到最高阶段存在但中间环节缺失时标红提示「请检查」。

**Architecture:** 新增断裂检测逻辑：先找到最高阶段（uploaded/done/scaled），再往回验证中间环节是否完整。Rust 后端新增 `Broken` 枚举变体，并重写 `determine_progress_image` / `determine_progress_sequence` / `determine_progress_prototype_seq` 三个函数加入断裂检测。前端链路：`useMaterials.ts` 类型 → `MaterialCard.vue` 样式 → `TaskPage.vue` 标签文字，全部补充 `broken` 支持。

**Tech Stack:** Rust（serde snake_case enum），Vue 3，CSS 变量（`--color-danger`）

---

## 断裂规则

**静帧链条**：`00_original → 01_scale → 02_done → nextcloud`

| 检测到最高阶段 | 往回验证 | 断裂条件 |
|---|---|---|
| `uploaded` | 02_done 是否存在该文件 | 不存在 → broken |
| `done` | 01_scale 是否存在该文件 | 不存在 → broken |
| `scaled` | 无需往回验证 | — |

**序列帧链条**：`00_original → 02_done（含.webp）→ nextcloud`（跳过 01_scale）

| 检测到最高阶段 | 往回验证 | 断裂条件 |
|---|---|---|
| `uploaded` | 02_done 是否有 .webp | 不存在 → broken |
| `done`（有.webp） | 无需往回验证 | — |
| `[an-XX-YY]` 存在但无 .webp | — | broken（三件套不完整） |

---

## Task 1：Rust — 新增 Broken 枚举变体

**Files:**
- Modify: `src-tauri/src/models.rs`

**Step 1: 在 MaterialProgress 枚举中新增 Broken 变体**

当前（约第 94 行）：
```rust
pub enum MaterialProgress {
    None,
    Original,
    Scaled,
    Done,
    Uploaded,
}
```

改为：
```rust
pub enum MaterialProgress {
    None,
    Original,
    Scaled,
    Done,
    Uploaded,
    /// 链条断裂（中间环节缺失，需要用户检查）
    Broken,
}
```

**Step 2: 编译确认无报错**

```bash
cd src-tauri && cargo build 2>&1 | tail -5
```

期望：`Finished dev profile` 或仅有 warning，无 error。

---

## Task 2：Rust — 重写静帧进度判定加入断裂检测

**Files:**
- Modify: `src-tauri/src/commands.rs`（`determine_progress_image` 函数，约第 841 行）

**Step 1: 替换 `determine_progress_image` 函数**

当前逻辑（从高到低检查，无断裂检测）：
```rust
fn determine_progress_image(...) -> MaterialProgress {
    if uploaded → Uploaded
    if done → Done
    if scaled → Scaled
    Original
}
```

新逻辑（找最高阶段，再往回验证）：

```rust
/// 静帧进度判定（含断裂检测）
fn determine_progress_image(
    base_name: &str,
    scale_dir: &Path,
    done_dir: &Path,
    nextcloud_dir: &Option<std::path::PathBuf>,
) -> MaterialProgress {
    let in_nextcloud = nextcloud_dir
        .as_ref()
        .map(|nc| nc.exists() && find_file_in_dir(nc, base_name))
        .unwrap_or(false);
    let in_done = done_dir.exists() && find_file_in_subdirs(done_dir, base_name, "img");
    let in_scale = scale_dir.exists() && find_file_in_subdirs(scale_dir, base_name, "");

    if in_nextcloud {
        // 最高阶段 = uploaded，往回验证 done 必须存在
        if !in_done {
            return MaterialProgress::Broken;
        }
        return MaterialProgress::Uploaded;
    }
    if in_done {
        // 最高阶段 = done，往回验证 scale 必须存在
        if !in_scale {
            return MaterialProgress::Broken;
        }
        return MaterialProgress::Done;
    }
    if in_scale {
        return MaterialProgress::Scaled;
    }
    MaterialProgress::Original
}
```

**Step 2: 编译确认无报错**

```bash
cd src-tauri && cargo build 2>&1 | tail -5
```

---

## Task 3：Rust — 重写序列帧进度判定加入断裂检测

**Files:**
- Modify: `src-tauri/src/commands.rs`（`determine_progress_sequence` 函数，约第 864 行）

**Step 1: 替换 `determine_progress_sequence` 函数**

```rust
/// 序列帧进度判定（含断裂检测，跳过 01_scale）
fn determine_progress_sequence(
    base_name: &str,
    done_dir: &Path,
    nextcloud_dir: &Option<std::path::PathBuf>,
) -> MaterialProgress {
    let in_nextcloud = nextcloud_dir
        .as_ref()
        .map(|nc| nc.exists() && find_file_in_dir(nc, base_name))
        .unwrap_or(false);
    // 检查 02_done 中是否有 .webp（完整输出）
    let in_done_webp = done_dir.exists() && find_webp_in_subdirs(done_dir, base_name, "an");
    // 检查 02_done 中是否有任意文件（包括只有 .tps 的不完整情况）
    let in_done_any = done_dir.exists() && find_file_in_subdirs(done_dir, base_name, "an");

    if in_nextcloud {
        // 最高阶段 = uploaded，往回验证 done 必须有 .webp
        if !in_done_webp {
            return MaterialProgress::Broken;
        }
        return MaterialProgress::Uploaded;
    }
    if in_done_any {
        // 有 done 目录，但如果缺 .webp 说明三件套不完整
        if !in_done_webp {
            return MaterialProgress::Broken;
        }
        return MaterialProgress::Done;
    }
    MaterialProgress::Original
}
```

**Step 2: 同样更新 `determine_progress_prototype_seq`（Prototype 序列帧，约第 690 行）**

```rust
fn determine_progress_prototype_seq(
    base_name: &str,
    sub_name: &str,
    done_dir: &Path,
    nextcloud_dir: &Option<std::path::PathBuf>,
) -> MaterialProgress {
    let in_nextcloud = nextcloud_dir
        .as_ref()
        .map(|nc| {
            let nc_sub = nc.join(sub_name);
            nc_sub.exists() && find_file_in_dir(&nc_sub, base_name)
        })
        .unwrap_or(false);
    let in_done_webp = done_dir.exists()
        && find_webp_in_proto_subdirs(done_dir, base_name, sub_name, "an");
    let in_done_any = done_dir.exists()
        && find_file_in_proto_subdirs(done_dir, base_name, sub_name, "an");

    if in_nextcloud {
        if !in_done_webp {
            return MaterialProgress::Broken;
        }
        return MaterialProgress::Uploaded;
    }
    if in_done_any {
        if !in_done_webp {
            return MaterialProgress::Broken;
        }
        return MaterialProgress::Done;
    }
    MaterialProgress::Original
}
```

**Step 3: 编译确认**

```bash
cd src-tauri && cargo build 2>&1 | tail -5
```

---

## Task 4：前端 — useMaterials.ts 类型新增 broken

**Files:**
- Modify: `src/composables/useMaterials.ts`（第 5 行）

**Step 1: 在 MaterialProgress 类型中新增 `'broken'`**

当前：
```ts
export type MaterialProgress = 'none' | 'original' | 'scaled' | 'done' | 'uploaded'
```

改为：
```ts
export type MaterialProgress = 'none' | 'original' | 'scaled' | 'done' | 'uploaded' | 'broken'
```

---

## Task 5：前端 — MaterialCard.vue 新增 broken 样式和标签

**Files:**
- Modify: `src/components/MaterialCard.vue`

**Step 1: 在 `progressLabel` 映射中新增 broken**

当前（约第 27 行）：
```ts
const map: Record<string, string> = {
  none: '未开始',
  original: '原始文件',
  scaled: '已缩放',
  done: '已完成',
  uploaded: '已上传',
}
```

改为：
```ts
const map: Record<string, string> = {
  none: '未开始',
  original: '原始文件',
  scaled: '已缩放',
  done: '已完成',
  uploaded: '已上传',
  broken: '请检查',
}
```

**Step 2: 在 CSS 中新增 `.progress-broken` 样式**

在现有进度样式（约第 207 行）后追加：
```css
.progress-broken {
  background: var(--color-danger-500, #ef4444);
  color: white;
  animation: broken-pulse 2s ease-in-out infinite;
}

@keyframes broken-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.7; }
}
```

---

## Task 6：前端 — TaskPage.vue 侧边栏进度标签补充 broken

**Files:**
- Modify: `src/views/TaskPage.vue`（`progressLabel` 函数，约第 513 行）

**Step 1: 在映射中新增 broken**

当前：
```ts
const map: Record<string, string> = {
  none: '未开始', original: '原始文件', scaled: '已缩放', done: '已完成', uploaded: '已上传',
}
```

改为：
```ts
const map: Record<string, string> = {
  none: '未开始', original: '原始文件', scaled: '已缩放', done: '已完成', uploaded: '已上传', broken: '请检查',
}
```

---

## Task 7：前端 — ConvertPage / ScalePage 过滤逻辑补充 broken

转换页和缩放页目前过滤掉 `done` 和 `uploaded` 的素材，`broken` 的素材应该**显示**（需要重新转换），无需修改过滤逻辑。

但需确认 ConvertPage 的 `canStart` 校验不会意外排除 broken 素材。

**Files:**
- Read: `src/views/ConvertPage.vue`（确认 `pendingImages` / `pendingSequences` 过滤逻辑）

当前过滤：
```ts
m.progress !== 'done' && m.progress !== 'uploaded'
```

`broken` 不在排除列表，天然会显示。无需修改。

---

## Task 8：整体验收

**编译验证：**

```bash
cd D:/work/pgsoft/PGB1/src-tauri && cargo build 2>&1 | tail -5
cd D:/work/pgsoft/PGB1 && npx vue-tsc --noEmit 2>&1 | head -10
```

两者均无报错。

**手动验证（使用测试数据）：**

1. `fs_vfx_c_screen`：`[an-100-18]/` 里只有 `.tps` → 刷新后应显示红色「请检查」标签
2. 正常素材：进度标签显示不受影响
3. ConvertPage：`broken` 的序列帧出现在序列帧区，可以重新标注 FPS 并转换
