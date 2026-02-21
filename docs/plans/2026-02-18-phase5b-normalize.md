# Phase 5b: 规范化 (Normalization) 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现 "规范化" 功能，自动整理 00_original 目录下的 AE 导出文件（静帧去后缀，序列帧归类到文件夹）。

**Architecture:** 
- Rust 后端新增 `preview_normalize` 命令（扫描并返回计划执行的操作列表）和 `execute_normalize` 命令（执行实际的文件操作）。
- 前端新增 `NormalizationDialog.vue` 弹窗，展示对比预览（原始 -> 规范后），并在用户确认后执行操作。

**Tech Stack:** Tauri 2.x, Rust, Vue 3, Composition API.

---

## Task 1: Rust 后端 — 实现预览规范化命令

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 在 models.rs 中添加模型**

```rust
/// 规范化操作类型
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum NormalizeActionType {
    /// 重命名文件（用于静帧去 _01）
    Rename,
    /// 创建文件夹并移动（用于序列帧）
    MoveToFolder,
}

/// 规范化单项操作预览
#[derive(Debug, Serialize, Clone)]
pub struct NormalizePreviewItem {
    pub original_path: String,
    pub original_name: String,
    pub target_name: String,
    pub action_type: NormalizeActionType,
    pub is_sequence: bool,
}
```

**Step 2: 在 commands.rs 中实现 preview_normalize**

逻辑：
1. 扫描 `00_original/`。
2. 提取所有文件的 base_name（去掉 _NN 后缀）。
3. 统计每个 base_name 出现的文件数。
4. 数量 == 1 且以 _01 结尾 -> Rename (去掉 _01)。
5. 数量 > 1 -> MoveToFolder (创建同名文件夹)。

**Step 3: 注册命令**

在 `lib.rs` 中注册 `commands::preview_normalize`。

---

## Task 2: Rust 后端 — 实现执行规范化命令

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 实现 execute_normalize 命令**

```rust
#[tauri::command]
pub fn execute_normalize(
    items: Vec<NormalizePreviewItem>
) -> Result<(), String> {
    for item in items {
        match item.action_type {
            NormalizeActionType::Rename => {
                let old_path = Path::new(&item.original_path);
                let new_path = old_path.parent().unwrap().join(&item.target_name);
                fs::rename(old_path, new_path).map_err(|e| e.to_string())?;
            },
            NormalizeActionType::MoveToFolder => {
                let old_path = Path::new(&item.original_path);
                let parent = old_path.parent().unwrap();
                let target_dir = parent.join(Path::new(&item.target_name).file_stem().unwrap());
                if !target_dir.exists() {
                    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
                }
                let dest_path = target_dir.join(old_path.file_name().unwrap());
                fs::rename(old_path, dest_path).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
```

**Step 2: 注册命令**

在 `lib.rs` 中注册 `commands::execute_normalize`。

---

## Task 3: 前端 — 创建 NormalizationDialog 组件

**Files:**
- Create: `src/components/NormalizationDialog.vue`

**功能说明：** 展示预览列表，左侧是原文件名，中间是箭头，右侧是目标文件名或文件夹。支持一键全选。

**UI 规范：**
- 使用 `glass-strong` 样式。
- 列表支持滚动。
- 底部 [执行规范化] 按钮（确认类，绿色）和 [取消] 按钮。

---

## Task 4: 前端 — 集成 NormalizationDialog 到 TaskPage

**Files:**
- Modify: `src/views/TaskPage.vue`

**Step 1: 导入并添加弹窗状态**

```typescript
const showNormalizeDialog = ref(false)
```

**Step 2: 实现进入规范化流程函数**

```typescript
async function startNormalize() {
  showNormalizeDialog.value = true
}
```

**Step 3: 更新导航栏按钮**

将 `normalize` 按钮的 `handler` 关联到 `startNormalize`。

---

## Task 5: 联调与测试

**验证点：**
1. 准备一个包含 `xxx_01.jpg` (静帧) 和 `yyy_01.png`, `yyy_02.png` (序列帧) 的 `00_original` 目录。
2. 点击 [规范化]，预览弹窗正确识别：
   - `xxx_01.jpg` -> `xxx.jpg` (Rename)
   - `yyy_01.png` -> `yyy/yyy_01.png` (Move)
3. 执行后，文件系统变化正确。
4. 页面自动刷新，素材列表显示正常。
