use crate::models::AttendanceConfig;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri::async_runtime::JoinHandle;

/// 考勤定时调度器
pub struct AttendanceScheduler {
    clock_in_handle: Option<JoinHandle<()>>,
    clock_out_handle: Option<JoinHandle<()>>,
    daily_report_handle: Option<JoinHandle<()>>,
    overtime_handle: Option<JoinHandle<()>>,
}

impl AttendanceScheduler {
    pub fn new() -> Self {
        Self {
            clock_in_handle: None,
            clock_out_handle: None,
            daily_report_handle: None,
            overtime_handle: None,
        }
    }

    /// 停止所有定时任务
    pub fn stop(&mut self) {
        if let Some(h) = self.clock_in_handle.take() {
            h.abort();
        }
        if let Some(h) = self.clock_out_handle.take() {
            h.abort();
        }
        if let Some(h) = self.daily_report_handle.take() {
            h.abort();
        }
        if let Some(h) = self.overtime_handle.take() {
            h.abort();
        }
    }

    /// 根据配置启动所有常驻定时任务
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

    /// 重置：stop + start
    pub fn reschedule(&mut self, app: AppHandle, config: &AttendanceConfig) {
        self.stop();
        self.start(app, config);
    }

    /// 创建一次性加班定时任务
    pub fn schedule_overtime(&mut self, app: AppHandle, minutes: u64) {
        // 取消之前的加班定时
        if let Some(h) = self.overtime_handle.take() {
            h.abort();
        }

        self.overtime_handle = Some(tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(minutes * 60)).await;
            let _ = create_reminder_window(&app, "overtime");
        }));
    }
}

/// 每日定时循环：分段 sleep + 墙钟校验，免疫系统休眠导致的单调时钟漂移。
///
/// 原理：`tokio::time::sleep` 基于单调时钟（Windows `QueryPerformanceCounter`），
/// 系统休眠/睡眠期间单调时钟暂停，导致长 sleep 实际等待远超目标墙钟时间。
/// 改为每段最多 30 秒，醒来后用 `chrono::Local::now()` 重新校验，
/// 系统唤醒后最多 30 秒即可检测到目标时间已过并触发提醒。
async fn daily_timer_loop(app: AppHandle, time_str: &str, reminder_type: &str) {
    loop {
        let duration = match calc_duration_until(time_str) {
            Some(d) => d,
            None => {
                // 解析失败，等待 1 小时后重试
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                continue;
            }
        };

        // 记录目标墙钟时间
        let wall_target = chrono::Local::now()
            + chrono::Duration::from_std(duration).unwrap_or(chrono::Duration::zero());

        // 分段 sleep：每段最多 30 秒，每段醒来用墙钟校验
        loop {
            let now = chrono::Local::now();
            if now >= wall_target {
                break;
            }
            let remaining = (wall_target - now)
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(30));
            let chunk = remaining.min(std::time::Duration::from_secs(30));
            tokio::time::sleep(chunk).await;
        }

        let _ = create_reminder_window(&app, reminder_type);

        // 等待 60 秒后进入下一轮循环（避免同一分钟内重复触发）
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}

/// 计算从现在到今天/明天 HH:mm 的 Duration
fn calc_duration_until(time_str: &str) -> Option<std::time::Duration> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let hour: u32 = parts[0].parse().ok()?;
    let minute: u32 = parts[1].parse().ok()?;

    let now = chrono::Local::now();
    let today = now.date_naive();

    let target_time = chrono::NaiveTime::from_hms_opt(hour, minute, 0)?;
    let mut target_dt = today.and_time(target_time);

    // 如果目标时间已过，设为明天
    if target_dt <= now.naive_local() {
        target_dt += chrono::Duration::days(1);
    }

    let target_local = target_dt
        .and_local_timezone(chrono::Local)
        .single()?;

    let duration = (target_local - now).to_std().ok()?;
    Some(duration)
}

/// 创建提醒弹窗（400×200 无装饰透明置顶小窗口）
pub fn create_reminder_window(app: &AppHandle, reminder_type: &str) -> Result<(), String> {
    let label = format!("reminder-{}", reminder_type);

    // 如果同名窗口已存在，显示并聚焦
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }

    let url = format!("/reminder/{}", reminder_type);

    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title("PG素材管理系统")
        .inner_size(400.0, 200.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .center()
        .visible(false)
        .build()
        .map_err(|e| format!("创建提醒窗口失败: {}", e))?;

    // 应用毛玻璃效果
    #[cfg(target_os = "windows")]
    {
        use window_vibrancy::apply_acrylic;
        let _ = apply_acrylic(&window, Some((0, 0, 0, 1)));
    }

    // 后备机制：Rust 侧延迟显示窗口
    // 前端 ReminderPage.vue onMounted 也会调 show()，这里做双保险
    // 防止 visible(false) 下 WebView2 未执行前端 JS 的情况
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if let Some(w) = app_clone.get_webview_window(&label) {
            let _ = w.show();
            let _ = w.set_focus();
        }
    });

    Ok(())
}

/// SchedulerState 类型别名，用于 Tauri State 管理
pub type SchedulerState = Arc<Mutex<AttendanceScheduler>>;
