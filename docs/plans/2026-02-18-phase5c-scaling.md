# Phase 5c: 缩放 (Scaling) 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现 "缩放" 功能，允许用户为选中的静帧素材指定缩放比例（100/70/50/40 或自定义），并自动生成缩放后的文件到 `01_scale/[XX]/`。

**Architecture:** 
- Rust 后端新增 `execute_scaling` 命令，使用 `image` crate 进行高质量缩放（Lanczos3 算法）。
- 前端新增 `ScalingDialog.vue` 弹窗，支持选择素材、设定比例（预设或手动输入）。
- 集成到 `TaskPage.vue` 的导航栏按钮。

**Tech Stack:** Tauri 2.x, Rust (`image` crate), Vue 3.

---

## Task 1: Rust 后端 — 添加 image 依赖与命令框架

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 添加 Rust 依赖**

在 `src-tauri/Cargo.toml` 中添加：
```toml
image = "0.25"
```

**Step 2: 在 models.rs 中添加请求结构**

```rust
#[derive(Debug, Deserialize, Clone)]
pub struct ScaleRequest {
    pub original_path: String,
    pub target_dir: String, // 01_scale/[XX]/
    pub scale_percent: u32,
    pub base_name: String,
}
```

**Step 3: 实现 execute_scaling 命令**

逻辑：
1. 读取原图。
2. 计算目标尺寸。
3. 执行 `resize`（FilterType::Lanczos3）。
4. 保存到目标路径。

**Step 4: 注册命令**

在 `lib.rs` 中注册 `commands::execute_scaling`。

---

## Task 2: 前端 — 创建 ScalingDialog 组件

**Files:**
- Create: `src/components/ScalingDialog.vue`

**功能说明：** 
- 素材选择：列出当前选中的或全部静帧。
- 比例设置：提供 100, 70, 50, 40 的快捷按钮，以及一个自定义数字输入框。
- 预览列表：显示即将处理的任务列表。
- 进度条：显示处理进度。

**UI 规范：**
- 遵循 `glass-strong` 样式。
- 按钮变体遵循设计系统。

---

## Task 3: 前端 — 集成 ScalingDialog 到 TaskPage

**Files:**
- Modify: `src/views/TaskPage.vue`

**Step 1: 导入并添加弹窗状态**

```typescript
const showScalingDialog = ref(false)
const materialsToScale = ref<MaterialInfo[]>([])
```

**Step 2: 实现 startScaling 函数**

逻辑：
- 如果有多选，处理多选素材中的静帧。
- 如果没多选，处理当前显示的全部静帧（或者仅打开弹窗由用户选）。

**Step 3: 更新导航栏按钮**

将 `scale` 按钮关联到 `startScaling`。

---

## Task 4: 联调与测试

**验证点：**
1. 选择一个 1920x1080 的静帧，选择 50% 缩放。
2. 执行后，`01_scale/[50]/` 下生成了 960x540 的文件。
3. 文件质量清晰，无明显锯齿。
4. 页面刷新后，素材进度显示为 "已缩放"。
