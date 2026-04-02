# 打卡模式选择 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为打卡功能增加三档模式（关闭/开启/仅记录）和日报提醒独立开关。

**Architecture:**
- `AttendanceConfig.mode: String`（`"off"` / `"auto"` / `"record_only"`）控制出勤退勤打卡行为
- `DailyReportSettings.enabled: bool` 控制日报提醒定时任务
- 后端 `execute_clock_action_inner` 在 config 加载后立即 early-return 走 record_only 路径
- 前端 SettingsPage 出勤分组增加三段按钮，日报分组增加 toggle，mode/enabled 随其他字段一起 save

**Tech Stack:** Rust (serde_json, chrono), Vue 3 + TypeScript, vue-i18n

---

## Task 1：Rust 数据模型 — 新增 mode 字段和 daily_report.enabled

**Files:**
- Modify: `src-tauri/src/models.rs:226-278`

**Step 1: 在 `models.rs` 中添加 serde 默认值 helper 函数**

在 `// ─── 日报打卡系统 ─────────────────────────────────────────────` 注释行（L226）之前，
紧接着在该注释之后、`pub struct AttendanceConfig` 之前插入两个 helper fn：

```rust
fn default_attendance_mode() -> String {
    "auto".to_string()
}

fn default_true() -> bool {
    true
}
```

**Step 2: 给 `AttendanceConfig` 加 `mode` 字段**

修改 `AttendanceConfig` struct（L230-235）：

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AttendanceConfig {
    /// 打卡模式："off" 关闭 / "auto" 自动打卡 / "record_only" 仅记录时间
    #[serde(default = "default_attendance_mode")]
    pub mode: String,
    pub attendance: AttendanceSettings,
    pub daily_report: DailyReportSettings,
    /// 账号（明文存储，密码不在这里）
    pub username: String,
}
```

**Step 3: 给 `DailyReportSettings` 加 `enabled` 字段**

修改 `DailyReportSettings` struct（L253-259）：

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DailyReportSettings {
    /// 日报提醒开关（默认开启）
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 日报提醒时间 "HH:mm"
    pub time: String,
    /// 日报网站 URL
    pub url: String,
}
```

**Step 4: 更新 `Default` 实现**

修改 `impl Default for AttendanceConfig`（L261-278），加入新字段：

```rust
impl Default for AttendanceConfig {
    fn default() -> Self {
        Self {
            mode: "auto".to_string(),
            attendance: AttendanceSettings {
                clock_in_time: "09:50".to_string(),
                clock_out_time: "19:00".to_string(),
                url: String::new(),
                lunch_start_time: None,
                lunch_end_time: None,
            },
            daily_report: DailyReportSettings {
                enabled: true,
                time: "18:30".to_string(),
                url: String::new(),
            },
            username: String::new(),
        }
    }
}
```

**Step 5: 验证编译**

```bash
cd "C:\work\PG Butler\PGB1\src-tauri" && cargo check 2>&1
```

期望：无 error（可能有 warning，无妨）

**Step 6: Commit**

```bash
git add src-tauri/src/models.rs
git commit -m "feat(attendance): 数据模型新增 mode 字段和 daily_report.enabled"
```

---

## Task 2：Rust 调度器 — 尊重 mode 和 enabled

**Files:**
- Modify: `src-tauri/src/scheduler.rs:41-68`

**Step 1: 修改 `start()` 方法**

当前代码在 L41：`pub fn start(&mut self, app: AppHandle, config: &AttendanceConfig)`

将 `start()` 方法体改为：

```rust
pub fn start(&mut self, app: AppHandle, config: &AttendanceConfig) {
    // 关闭模式：不启动任何定时任务
    if config.mode == "off" {
        return;
    }

    // 出勤提醒
    if !config.attendance.clock_in_time.is_empty() {
        let app_clone = app.clone();
        let time_str = config.attendance.clock_in_time.clone();
        self.clock_in_handle = Some(tauri::async_runtime::spawn(async move {
            daily_timer_loop(app_clone, &time_str, "clock-in").await;
        }));
    }

    // 退勤提醒
    if !config.attendance.clock_out_time.is_empty() {
        let app_clone = app.clone();
        let time_str = config.attendance.clock_out_time.clone();
        self.clock_out_handle = Some(tauri::async_runtime::spawn(async move {
            daily_timer_loop(app_clone, &time_str, "clock-out").await;
        }));
    }

    // 日报提醒（独立开关）
    if config.daily_report.enabled && !config.daily_report.time.is_empty() {
        let app_clone = app.clone();
        let time_str = config.daily_report.time.clone();
        self.daily_report_handle = Some(tauri::async_runtime::spawn(async move {
            daily_timer_loop(app_clone, &time_str, "daily-report").await;
        }));
    }
}
```

**Step 2: 验证编译**

```bash
cd "C:\work\PG Butler\PGB1\src-tauri" && cargo check 2>&1
```

**Step 3: Commit**

```bash
git add src-tauri/src/scheduler.rs
git commit -m "feat(attendance): 调度器尊重 mode=off 和 daily_report.enabled"
```

---

## Task 3：Rust 命令 — execute_clock_action_inner 的 record_only 路径

**Files:**
- Modify: `src-tauri/src/commands.rs:3399-3437`（`execute_clock_action_inner` 开头部分）

**Step 1: 在 config 加载之后、url/username 校验之前插入 mode 分支**

定位到 `execute_clock_action_inner` 函数（L3399）。
找到 config 加载完毕后的 L3420（`} else { return Err(...) }`），
在 L3421（`if config.attendance.url.is_empty()`）之前插入：

```rust
    // ── 模式分发 ──────────────────────────────────────────────
    if config.mode == "off" {
        return Err("打卡功能已关闭".to_string());
    }

    if config.mode == "record_only" {
        let now = chrono::Local::now();
        let today = now.format("%Y-%m-%d").to_string();
        let actual_time = now.format("%H:%M").to_string();
        let record_path = config_dir.join("attendance_record.json");
        let mut record = load_attendance_record_internal(&record_path);
        if action == "clock_in" {
            record.last_clock_in = Some(today);
            record.actual_clock_in_time = Some(actual_time.clone());
        } else {
            record.last_clock_out = Some(today);
            record.actual_clock_out_time = Some(actual_time.clone());
        }
        save_attendance_record_internal(&record_path, &record);
        emit_progress(&app_handle, "success", &format!("已记录 {}", actual_time));
        return Ok(format!("已记录 {}", actual_time));
    }
    // ── 以下为 mode == "auto" 的 WebView 自动化逻辑 ─────────────
```

注意：`config_dir` 在 L3408 已经定义，可以直接使用。

**Step 2: 验证编译**

```bash
cd "C:\work\PG Butler\PGB1\src-tauri" && cargo check 2>&1
```

**Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat(attendance): record_only 模式直接写时间记录，跳过 WebView 自动化"
```

---

## Task 4：i18n — 新增文案

**Files:**
- Modify: `src/locales/zh-CN.ts:243-263`（settings.attendance 区块内）
- Modify: `src/locales/en.ts`（对应位置）

**Step 1: zh-CN.ts — 在 `attendanceGroup` 行之后插入新 key**

定位到 `attendanceGroup: '考勤设置',`（L244），在其之前插入：

```ts
    // 打卡模式
    clockMode: '打卡模式',
    clockModeOff: '关闭',
    clockModeAuto: '开启',
    clockModeRecordOnly: '仅记录',
    clockModeRecordOnlyHint: '提醒时直接记录时间，不执行网页打卡',
    // 日报开关
    dailyReportEnabled: '日报提醒',
```

**Step 2: en.ts — 对应位置插入**

定位到英文 `attendanceGroup` key，在其之前插入：

```ts
    // Clock mode
    clockMode: 'Clock Mode',
    clockModeOff: 'Off',
    clockModeAuto: 'On',
    clockModeRecordOnly: 'Record Only',
    clockModeRecordOnlyHint: 'Logs the time on reminder click, no web automation',
    // Daily report toggle
    dailyReportEnabled: 'Daily Report Reminder',
```

**Step 3: Commit**

```bash
git add src/locales/zh-CN.ts src/locales/en.ts
git commit -m "feat(i18n): 新增打卡模式和日报开关文案"
```

---

## Task 5：前端 SettingsPage — script 部分

**Files:**
- Modify: `src/views/SettingsPage.vue`

**Step 1: 新增响应式状态**

在 `// ─── 日报打卡状态 ─────────────────────────────────────────────` 区块（L42）的末尾（`attendanceDirty` ref 之后），加入：

```ts
const attendanceMode = ref<'off' | 'auto' | 'record_only'>('auto')
const dailyReportEnabled = ref(true)
```

**Step 2: `init()` 中读取新字段**

定位 `init()` 函数里加载打卡配置的 `try` 块（L76-97），修改 `invoke` 调用的类型标注，
在 `clockInTime.value = config.attendance.clock_in_time` 之前加：

```ts
    attendanceMode.value = (config.mode ?? 'auto') as 'off' | 'auto' | 'record_only'
    dailyReportEnabled.value = config.daily_report.enabled ?? true
```

**Step 3: watch 中加入新 ref**

定位 `watch([clockInTime, clockOutTime, ...]` 那行（L152），加入两个新 ref：

```ts
watch([clockInTime, clockOutTime, attendanceUrl, lunchStartTime, lunchEndTime,
       dailyReportTime, dailyReportUrl, attendanceUsername, attendancePassword,
       attendanceMode, dailyReportEnabled], () => {
  attendanceDirty.value = true
  attendanceSaved.value = false
})
```

（完整替换原来的 watch 调用）

**Step 4: `handleAttendanceSave()` 中加入新字段**

定位 `save_attendance_config` invoke（L188-203），在 config 对象里加 mode 和 enabled：

```ts
    await invoke('save_attendance_config', {
      config: {
        mode: attendanceMode.value,
        attendance: {
          clock_in_time: clockInTime.value,
          clock_out_time: clockOutTime.value,
          url: attendanceUrl.value.trim(),
          lunch_start_time: lunchStartTime.value || null,
          lunch_end_time: lunchEndTime.value || null,
        },
        daily_report: {
          enabled: dailyReportEnabled.value,
          time: dailyReportTime.value,
          url: dailyReportUrl.value.trim(),
        },
        username: attendanceUsername.value,
      },
    })
```

**Step 5: Commit（script 部分）**

```bash
git add src/views/SettingsPage.vue
git commit -m "feat(settings): 打卡模式/日报开关 script 逻辑"
```

---

## Task 6：前端 SettingsPage — 出勤分组 UI

**Files:**
- Modify: `src/views/SettingsPage.vue`（template 部分，attendance tab）

**Step 1: 出勤分组顶部加模式选择**

找到 `<div class="attendance-group">` 内 `<p class="attendance-group-title">{{ $t('settings.attendanceGroup') }}</p>` 这行（L394）。

在该 `<p>` 标签之后、第一个 `<div class="form-group">` 之前插入：

```html
            <div class="form-group">
              <label class="form-label">{{ $t('settings.clockMode') }}</label>
              <div class="mode-btn-group">
                <button
                  class="mode-btn"
                  :class="{ active: attendanceMode === 'off' }"
                  @click="attendanceMode = 'off'"
                >{{ $t('settings.clockModeOff') }}</button>
                <button
                  class="mode-btn"
                  :class="{ active: attendanceMode === 'auto' }"
                  @click="attendanceMode = 'auto'"
                >{{ $t('settings.clockModeAuto') }}</button>
                <button
                  class="mode-btn"
                  :class="{ active: attendanceMode === 'record_only' }"
                  @click="attendanceMode = 'record_only'"
                >{{ $t('settings.clockModeRecordOnly') }}</button>
              </div>
              <p v-if="attendanceMode === 'record_only'" class="form-hint">
                {{ $t('settings.clockModeRecordOnlyHint') }}
              </p>
            </div>
```

**Step 2: 出勤分组其余字段按 mode 灰化**

给出勤分组的 URL 输入行（`attendanceUrl`）和账号密码整个 `attendance-group` div 加 `:class`，
在不需要 URL 的 mode 下灰化：

在 `attendanceUrl` 所在 `form-group`（L411-414），加：

```html
            <div class="form-group" :class="{ 'form-group-disabled': attendanceMode !== 'auto' }">
```

同理，账号设置 `attendance-group` 整块（L429）：

```html
          <div class="attendance-group" :class="{ 'form-group-disabled': attendanceMode !== 'auto' }">
```

测试连接整块（L446）：

```html
          <div class="attendance-group" :class="{ 'form-group-disabled': attendanceMode !== 'auto' }">
```

**Step 3: 日报分组顶部加 enabled toggle**

找到日报分组（L417-427）的 `<p class="attendance-group-title">` 行，
将整个 `attendance-group` div 的首行改为：

```html
          <div class="attendance-group">
            <div class="group-title-row">
              <p class="attendance-group-title">{{ $t('settings.dailyReportGroup') }}</p>
              <label class="toggle-label">
                <input type="checkbox" v-model="dailyReportEnabled" />
                {{ $t('settings.dailyReportEnabled') }}
              </label>
            </div>
```

并给日报分组的内容字段加灰化：

```html
            <div :class="{ 'form-group-disabled': !dailyReportEnabled }">
              <!-- 原 dailyReportTime 和 dailyReportUrl form-group 移入这个 div -->
            </div>
```

**Step 4: Commit（template 部分）**

```bash
git add src/views/SettingsPage.vue
git commit -m "feat(settings): 出勤 mode 三段按钮 + 日报 enabled toggle UI"
```

---

## Task 7：前端 SettingsPage — 样式

**Files:**
- Modify: `src/views/SettingsPage.vue`（`<style scoped>` 末尾）

**Step 1: 加入所需 CSS**

在 `<style scoped>` 末尾追加：

```css
/* 打卡模式三段按钮 */
.mode-btn-group {
  display: flex;
  gap: var(--spacing-1);
}

.mode-btn {
  flex: 1;
  height: var(--button-height);
  font-size: var(--text-sm);
  font-weight: var(--font-weight-heading);
  color: var(--text-secondary);
  background: var(--bg-hover);
  border: 1px solid var(--border-light);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.mode-btn:hover {
  color: var(--text-primary);
  border-color: var(--border-medium);
}

.mode-btn.active {
  color: var(--color-primary-300);
  background: color-mix(in srgb, var(--color-primary-500) 15%, transparent);
  border-color: color-mix(in srgb, var(--color-primary-500) 40%, transparent);
}

/* 日报 group 标题行（标题 + toggle 同行） */
.group-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--spacing-3);
}

.group-title-row .attendance-group-title {
  margin-bottom: 0;
}

.toggle-label {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  font-size: var(--text-sm);
  color: var(--text-secondary);
  cursor: pointer;
  user-select: none;
}

/* 灰化状态（mode 不适用时） */
.form-group-disabled {
  opacity: 0.35;
  pointer-events: none;
}
```

**Step 2: Commit（样式）**

```bash
git add src/views/SettingsPage.vue
git commit -m "feat(settings): 打卡模式按钮 + 灰化 + toggle 样式"
```

---

## Task 8：端对端验证

**Step 1: 启动开发服务器**

```bash
cd "C:\work\PG Butler\PGB1" && npm run tauri dev
```

**Step 2: 验证「关闭」模式**
1. 设置页 → 日报打卡 → 打卡模式选「关闭」→ 保存
2. 重启程序（关闭托盘→重新打开），确认出勤/退勤/日报提醒窗口均不弹出
3. 状态栏工作时间胶囊：因无打卡记录，正常不显示

**Step 3: 验证「仅记录」模式**
1. 打卡模式选「仅记录」→ 保存
2. URL / 账号密码区灰化（不可点击）
3. 等出勤提醒弹窗 或 在设置页找到手动触发方式（或直接用提醒窗口路由 `/reminder/clock-in` 开发者模式访问）
4. 点击「出勤打卡」→ 弹窗出现"已记录 XX:XX"（而非 WebView 流程）
5. 状态栏显示已工作胶囊（有 actual_clock_in_time 记录）

**Step 4: 验证「开启」模式**
1. 模式改回「开启」→ 保存
2. URL/账号密码可用
3. 测试连接按钮可点击，流程正常

**Step 5: 验证日报提醒开关**
1. 日报提醒关闭 → 保存 → 重启 → 日报时间到达时不弹窗
2. 打开 → 保存 → 正常弹窗

**Step 6: 验证向前兼容**
- 手动删除 `%APPDATA%\com.pgb1\config\attendance_config.json`（或找到实际路径），重启程序 → 进设置页 → 打卡模式应默认显示「开启」，日报提醒开关应默认开启

---

## 注意事项

- `record_only` 模式下 `test_clock_action_inner`（测试连接）不受影响，测试连接仍走 WebView 流程，但测试按钮已被 `attendanceMode !== 'auto'` 灰化，用户无法点击，无需修改后端
- `mode == "off"` 时调度器不弹提醒窗口，但用户在 ReminderPage 路由直接访问（开发模式）仍能触发 execute_clock_action → 后端返回 Err("打卡功能已关闭") → 弹窗显示错误信息，行为合理
- `#[serde(default = "...")]` 保证老的 `attendance_config.json` 文件（无 mode / enabled 字段）反序列化时自动填入默认值，无破坏性
