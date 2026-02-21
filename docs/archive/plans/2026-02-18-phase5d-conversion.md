# Phase 5d: 格式转换 (Conversion) 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现 "转换" 功能，调用 imagine 处理静帧转换，调用 TexturePacker 处理序列帧打包，并自动整理到 `02_done/` 的 [img-XX] 或 [an-XX-YY] 目录。

**Architecture:** 
- **后端 (Rust)**：实现 `start_conversion_session`（启动外部工具并建立目录监控）和 `ConversionManager`（状态管理 + Windows 窗口控制 API）。
- **前端 (Vue)**：新增 `ConversionDialog.vue`（素材选择与帧率输入）和 `ConversionFloatWindow.vue`（置顶进度条）。
- **流程控制**：素材选择 -> 后端启动监控 -> 外部工具输出 -> 后端识别并自动归位 -> 前端展示实时进度。

**Tech Stack:** Tauri 2.x, Rust (notify crate, Windows API), Vue 3.

---

## Task 1: Rust 后端 — 转换管理与窗口控制骨架

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/conversion.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 添加 Windows 依赖**

在 `Cargo.toml` 中添加：
```toml
windows = { version = "0.58", features = ["Win32_Foundation", "Win32_UI_WindowsAndMessaging", "Win32_System_Threading"] }
notify = "6.1"
```

**Step 2: 实现窗口置前逻辑**

在 `conversion.rs` 中实现 `bring_window_to_front(pid)` 函数，使用 `EnumWindows` 和 `SetForegroundWindow`。

**Step 3: 定义 ConversionSession 状态**

用于存储映射表（文件名 -> 比例/帧率）和监控器句柄。

---

## Task 2: 前端 — 创建 ConversionDialog 组件

**Files:**
- Create: `src/components/ConversionDialog.vue`

**功能说明：** 
- 分区展示：静帧列表、序列帧列表。
- 静帧：默认全选，排除已转换项。
- 序列帧：**强制填写帧率**（1-120），填写完整后才允许“开始制作”。
- 提交：发送静帧映射表和序列帧列表给后端。

---

## Task 3: Rust 后端 — 实现静帧转换逻辑 (imagine)

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: 实现 start_image_conversion**

1. 启动 `imagine.exe`。
2. 开启 `02_done` 目录监控。
3. 当新文件产生时：识别文件名 -> 移动到 `[img-XX]` 文件夹 -> 发送事件到前端。

---

## Task 4: Rust 后端 — 实现序列帧转换逻辑 (TexturePacker)

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: 实现 start_sequence_conversion**

1. 逐个循环序列帧：
   - 调用 TexturePacker CLI 生成 `.tps`。
   - 启动 TexturePacker GUI 并置前。
   - 等待用户关闭 GUI。
   - 解析 `.tps` 获取最终 scale。
   - 整理三件套到 `[an-XX-YY]`。
   - 通知前端更新。

---

## Task 5: 前端 — 转换进度悬浮窗与 TaskPage 集成

**Files:**
- Create: `src/components/ConversionFloatWindow.vue`
- Modify: `src/views/TaskPage.vue`

**Step 1: 创建悬浮窗**

- 使用独立的 Tauri WebviewWindow 或在主窗口内实现（计划建议：主窗口内实现的 Fixed 遮罩层/小浮窗更简单且风格统一）。
- 实时监听后端事件，更新 `X / Y` 计数。

**Step 2: 集成按钮逻辑**

关联导航栏 `convert` 按钮到 `ConversionDialog`。

---

## Task 6: 联调与测试

**验证点：**
1. 点击转换，弹窗正确排除已完成项。
2. imagine 启动后，转换出的 webp 能自动归位到 `[img-XX]`。
3. TexturePacker 流程中，关闭 GUI 后，三件套能自动按解析出的 scale 归位。
4. 转换完成后自动刷新素材列表。
