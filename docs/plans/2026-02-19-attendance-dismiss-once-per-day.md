# 出勤补打检测"每天只弹一次"实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 用户每天首次关闭出勤提醒弹窗后，当天重启软件不再重复弹出补打提醒。

**Architecture:** 在 `AttendanceRecord` 加 `dismissed_clock_in_date` 字段，用户点「关闭」时通过新 Tauri 命令写入当日日期，`lib.rs` 启动补打检测时同时判断该字段。定时任务（`daily_timer_loop`）不受影响，仍按时弹出。

**Tech Stack:** Rust（Tauri 2.x），Vue 3 + TypeScript，`invoke` IPC 调用

---

## 涉及文件

| 文件 | 操作 |
|------|------|
| `src-tauri/src/models.rs` | 修改 `AttendanceRecord`，加 `dismissed_clock_in_date` 字段 |
| `src-tauri/src/commands.rs` | 新增 `dismiss_clock_in_reminder` 命令 |
| `src-tauri/src/lib.rs` | 修改补打检测逻辑，加 dismissed 判断 |
| `src/views/ReminderPage.vue` | `closeWindow()` 时若 type=clock-in 调用新命令 |

---

### Task 1: 给 AttendanceRecord 加 dismissed 字段

**Files:**
- Modify: `src-tauri/src/models.rs`（找 `AttendanceRecord` 结构体，约第 261 行）

**Step 1: 修改结构体**

找到：
```rust
pub struct AttendanceRecord {
    /// 最后出勤打卡日期 "YYYY-MM-DD"
    pub last_clock_in: Option<String>,
    /// 最后退勤打卡日期 "YYYY-MM-DD"
    pub last_clock_out: Option<String>,
}
```

替换为：
```rust
pub struct AttendanceRecord {
    /// 最后出勤打卡日期 "YYYY-MM-DD"
    pub last_clock_in: Option<String>,
    /// 最后退勤打卡日期 "YYYY-MM-DD"
    pub last_clock_out: Option<String>,
    /// 用户主动关闭出勤提醒的日期 "YYYY-MM-DD"（每天只弹一次用）
    pub dismissed_clock_in_date: Option<String>,
}
```

**Step 2: 编译确认无误**

```bash
cd D:/work/pgsoft/PGB1
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5
```

预期：无 error（可能有 warning，无妨）

---

### Task 2: 新增 dismiss_clock_in_reminder 命令

**Files:**
- Modify: `src-tauri/src/commands.rs`（在 `save_attendance_record` 命令附近末尾追加）

**Step 1: 追加命令函数**

在 `save_attendance_record` 函数结束后追加：

```rust
/// 记录用户已关闭今日出勤提醒（防止重启后重复弹出）
#[tauri::command]
pub fn dismiss_clock_in_reminder(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {}", e))?;

    let record_path = config_dir.join("attendance_record.json");

    let mut record: AttendanceRecord = if record_path.exists() {
        fs::read_to_string(&record_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        AttendanceRecord::default()
    };

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    record.dismissed_clock_in_date = Some(today);

    save_attendance_record_internal(&record_path, &record);
    Ok(())
}
```

**Step 2: 在 lib.rs 注册命令**

找到 `lib.rs` 中的 `.invoke_handler(tauri::generate_handler![` 列表，加入：

```rust
commands::dismiss_clock_in_reminder,
```

**Step 3: 编译确认**

```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5
```

预期：无 error

---

### Task 3: 修改 lib.rs 补打检测逻辑

**Files:**
- Modify: `src-tauri/src/lib.rs`（第 121~148 行，`already_clocked_in` 判断块）

**Step 1: 找到现有逻辑**

```rust
let already_clocked_in = record
    .last_clock_in
    .as_ref()
    .map(|d| d == &today)
    .unwrap_or(false);

if !already_clocked_in {
    // 检查当前时间是否已过出勤提醒时间
    ...
```

**Step 2: 加入 dismissed 判断**

替换为：

```rust
let already_clocked_in = record
    .last_clock_in
    .as_ref()
    .map(|d| d == &today)
    .unwrap_or(false);

let already_dismissed = record
    .dismissed_clock_in_date
    .as_ref()
    .map(|d| d == &today)
    .unwrap_or(false);

if !already_clocked_in && !already_dismissed {
    // 检查当前时间是否已过出勤提醒时间
    ...
```

**Step 3: 编译确认**

```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5
```

预期：无 error

---

### Task 4: 前端 ReminderPage.vue 关闭时写入 dismissed

**Files:**
- Modify: `src/views/ReminderPage.vue`（`closeWindow` 函数，第 84~90 行）

**Step 1: 修改 closeWindow 函数**

现有：
```typescript
async function closeWindow() {
  try {
    await getCurrentWindow().close()
  } catch {
    // 忽略关闭错误
  }
}
```

替换为：
```typescript
async function closeWindow() {
  try {
    // 如果是出勤提醒，记录今日已关闭（防止重启后补打检测再次弹出）
    if (reminderType.value === 'clock-in') {
      await invoke('dismiss_clock_in_reminder').catch(() => {})
    }
    await getCurrentWindow().close()
  } catch {
    // 忽略关闭错误
  }
}
```

> 注意：`invoke` 已在文件顶部 import，无需额外引入。

**Step 2: 验证 import 存在**

检查文件第 4 行是否有：
```typescript
import { invoke } from '@tauri-apps/api/core'
```

若已存在则无需修改。

---

### Task 5: 完整构建验证

**Step 1: 运行完整构建**

```bash
cd D:/work/pgsoft/PGB1
npm run tauri build 2>&1 | tail -20
```

或开发模式验证：
```bash
npm run tauri dev
```

**Step 2: 手动验证流程**

1. 启动软件，确认已过出勤时间且今日未打卡 → 应弹出提醒
2. 点击「×」关闭弹窗
3. 关闭并重启软件 → **不应再弹出**补打提醒
4. 次日重启 → 应再次弹出（新的一天）

**Step 3: 验证定时任务不受影响**

定时任务走 `daily_timer_loop`（scheduler.rs），与 `dismissed_clock_in_date` 完全无关，无需额外测试。

---

## 副作用分析

- `AttendanceRecord` 新增可选字段，JSON 反序列化向后兼容（旧记录文件中无此字段时默认 `None`）
- 定时触发的弹窗路径（`daily_timer_loop`）完全不涉及此逻辑，行为不变
- 用户打卡成功后 `last_clock_in` 会被写入，即使有 dismissed 记录也不影响正确性（两个条件是 OR 关系：满足任一就不弹）
