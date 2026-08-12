mod commands;
mod hotkey;
mod migrations;
mod models;
mod scheduler;
mod conversion;

use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

/// 主窗口 / 贴图板 Acrylic 毛玻璃底色：纯净深灰黑 #0C0D10，A=225 ≈ 88% 不透明。
/// 与 design-system.css `--canvas-bg: #0C0D10` 保持一致。
pub const MAIN_ACRYLIC: (u8, u8, u8, u8) = (12, 13, 16, 225);

/// 浮动窗口（提醒弹窗 / 翻译悬浮窗）Acrylic 底色：几乎全透明，让玻璃效果主要由 CSS 提供。
pub const FLOATING_ACRYLIC: (u8, u8, u8, u8) = (0, 0, 0, 1);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 已有实例在运行：把主窗口带到前台
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }))
        // tauri-plugin-log：同时输出到 stdout（dev）和 app_config_dir/logs/（release）
        // 取代 env_logger：R-4 终态方案，对齐 Claude N-20
        // - LogDir 目标：用户无感，用户报 bug 时 dev 可以直接要 log 文件
        // - Webview 目标：前端 console 也能看到 Rust 日志（dev 调试友好）
        // - 默认 level = Info，关键错误靠 log::error! 主动打到最高优先级
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: None }),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
                ])
                .max_file_size(2_000_000)  // 2MB per file
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepAll)
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_drag::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            commands::scan_projects,
            commands::scan_tasks,
            commands::scan_directory,
            commands::scan_materials,
            commands::list_sequence_frames,
            commands::scan_material_versions,
            commands::open_in_explorer,
            commands::load_settings,
            commands::save_settings,
            commands::scan_normalize_items,
            commands::execute_normalize_v2,
            commands::restore_normalize_backup,
            commands::execute_scaling,
            commands::start_conversion,
            commands::stop_conversion,
            commands::execute_sequence_conversion,
            commands::collect_drag_files,
            commands::copy_to_nextcloud,
            commands::import_files,
            commands::load_global_tasks,
            commands::save_global_tasks,
            commands::apply_task_changes,
            commands::list_archived_tasks,
            commands::restore_archived_task,
            commands::delete_archived_version,
            commands::create_project,
            commands::toggle_subtask_completion,
            commands::mark_upload_prompted,
            commands::load_attendance_config,
            commands::save_attendance_config,
            commands::save_attendance_password,
            commands::load_attendance_password,
            commands::execute_clock_action,
            commands::test_clock_action,
            commands::show_clock_webview,
            commands::close_clock_webview,
            commands::open_daily_report,
            commands::test_reminder,
            commands::load_attendance_record,
            commands::save_attendance_record,
            commands::dismiss_clock_in_reminder,
            commands::reschedule_attendance,
            commands::open_reminder_window,
            commands::translate_text_stream,
            commands::toggle_translator_window,
            commands::open_pinboard_window,
            commands::load_shortcuts,
            commands::save_shortcuts,
            commands::launch_shortcut,
            commands::scan_app_shortcuts,
            commands::extract_exe_icon,
            commands::fetch_favicon,
            commands::copy_icon_to_cache,
            commands::rename_material,
            commands::delete_material,
            commands::reset_material_versions,
            commands::mark_not_sequence,
            commands::list_archived_materials,
            commands::restore_archived_material,
            commands::delete_archived_material_version,
            commands::read_text_file,
            commands::find_game_exe,
            commands::open_file,
            commands::edit_sequence_tps,
            commands::rename_sequence_fps,
            commands::set_default_ae_file,
            commands::update_project_deadline,
            commands::delete_project,
            commands::rename_project,
            commands::rename_file,
            commands::delete_file,
            commands::scan_preview_videos,
            commands::copy_preview_to_nextcloud,
            commands::extract_psd_thumbnail,
            commands::set_project_priority,
            commands::set_task_priority,
            commands::get_file_mtime,
            commands::get_pinboard,
            commands::save_pinboard,
            commands::save_pin_image,
            commands::delete_pin_image,
            commands::get_notes,
            commands::set_note,
            commands::extract_pdf_pages_text,
            commands::translate_text_once,
            commands::build_translated_pdf,
            commands::check_translated_pdf_exists,
            commands::fetch_ip_country,
            commands::fetch_cn_holiday_type,
            commands::fetch_nager_holidays,
        ])
        .setup(|app| {
            // 清理 v2.8.10 前的中文 productName 旧安装目录（静默，仅 Windows）
            // 详见 src-tauri/src/migrations.rs
            #[cfg(target_os = "windows")]
            tauri::async_runtime::spawn(async {
                migrations::migrate_legacy_install();
            });

            let window = app.get_webview_window("main").expect("main 窗口必须在 tauri.conf.json 中声明");

            // ─── 系统托盘 ────────────────────────────────────────
            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().expect("tauri.conf.json 必须配置 windows.icon").clone())
                .tooltip("PG素材管理系统")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                })
                .build(app)?;

            // 关闭按钮 → 最小化到托盘（不退出）
            let win = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = win.hide();
                }
            });

            // Windows: 应用 Acrylic 半透明毛玻璃效果
            #[cfg(target_os = "windows")]
            {
                use window_vibrancy::apply_acrylic;
                let _ = apply_acrylic(&window, Some(MAIN_ACRYLIC));
            }

            // 初始化考勤调度器
            let scheduler_state: scheduler::SchedulerState =
                Arc::new(Mutex::new(scheduler::AttendanceScheduler::new()));
            app.manage(scheduler_state.clone());

            // 初始化转换状态
            let conversion_state: conversion::ConversionState = Arc::new(Mutex::new(None));
            app.manage(conversion_state);

            // 加载配置并启动定时任务 + 补打检测
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // 加载考勤配置
                let config_dir = match app_handle.path().app_config_dir() {
                    Ok(dir) => dir,
                    Err(e) => {
                        log::error!("[startup] 获取 app_config_dir 失败: {}", e);
                        return;
                    }
                };

                let config_path = config_dir.join("attendance_config.json");
                let config: models::AttendanceConfig = if config_path.exists() {
                    match std::fs::read_to_string(&config_path) {
                        Ok(content) => match serde_json::from_str(&content) {
                            Ok(c) => c,
                            Err(e) => {
                                log::error!("[startup] 解析 attendance_config.json 失败: {}", e);
                                return;
                            }
                        },
                        Err(e) => {
                            log::error!("[startup] 读取 attendance_config.json 失败: {}", e);
                            return;
                        }
                    }
                } else {
                    return; // 无配置，不启动（首次运行属正常）
                };

                // 有有效配置才启动定时器
                let has_valid_config = !config.attendance.url.is_empty()
                    && !config.username.is_empty();

                if has_valid_config || !config.daily_report.url.is_empty() {
                    if let Ok(mut sched) = scheduler_state.lock() {
                        sched.start(app_handle.clone(), &config);
                    }
                }

                // 补打检测：如果今天还没出勤打卡且已过出勤时间
                if has_valid_config {
                    let record_path = config_dir.join("attendance_record.json");
                    let record: models::AttendanceRecord = if record_path.exists() {
                        std::fs::read_to_string(&record_path)
                            .ok()
                            .and_then(|c| serde_json::from_str(&c).ok())
                            .unwrap_or_default()
                    } else {
                        models::AttendanceRecord::default()
                    };

                    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
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
                        let now = chrono::Local::now();
                        let parts: Vec<&str> =
                            config.attendance.clock_in_time.split(':').collect();
                        if parts.len() == 2 {
                            if let (Ok(h), Ok(m)) =
                                (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                            {
                                if let Some(target) =
                                    chrono::NaiveTime::from_hms_opt(h, m, 0)
                                {
                                    if now.time() > target {
                                        let _ = scheduler::create_reminder_window(
                                            &app_handle,
                                            "clock-in",
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            });

            // 同步开机自启状态（以配置文件为准，修复重装后注册表丢失的情况）
            // 仅 release 构建执行：dev 构建注册的是 debug 二进制路径，
            // 会导致开机启动一个连不上 Vite 的幽灵实例
            #[cfg(not(debug_assertions))]
            {
                use tauri_plugin_autostart::ManagerExt;
                let settings_path = app.path().app_config_dir()
                    .map(|dir| dir.join("app_settings.json"));
                if let Ok(path) = settings_path {
                    if path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(s) = serde_json::from_str::<models::AppSettings>(&content) {
                                let autolaunch = app.autolaunch();
                                if s.general.auto_start {
                                    if let Err(e) = autolaunch.enable() {
                                        log::error!("[autolaunch] enable 失败（注册表权限 / 杀软拦截可能）: {}", e);
                                    }
                                } else if let Err(e) = autolaunch.disable() {
                                    log::error!("[autolaunch] disable 失败: {}", e);
                                }
                            }
                        }
                    }
                }
            }

            // 初始化全局快捷键（翻译窗口）
            let hotkey_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let settings_path = match hotkey_app.path().app_config_dir() {
                    Ok(dir) => dir.join("app_settings.json"),
                    Err(e) => {
                        log::error!("[hotkey] 获取 app_config_dir 失败: {}", e);
                        return;
                    }
                };
                let settings: models::AppSettings = if settings_path.exists() {
                    match std::fs::read_to_string(&settings_path) {
                        Ok(content) => match serde_json::from_str(&content) {
                            Ok(s) => s,
                            Err(e) => {
                                log::error!("[hotkey] 解析 app_settings.json 失败，使用默认配置: {}", e);
                                models::AppSettings::default()
                            }
                        },
                        Err(e) => {
                            log::error!("[hotkey] 读取 app_settings.json 失败，使用默认配置: {}", e);
                            models::AppSettings::default()
                        }
                    }
                } else {
                    models::AppSettings::default()
                };
                hotkey::start_hotkey_listener(
                    hotkey_app,
                    settings.translation.shortcut.clone(),
                    settings.translation.use_calculator_key,
                );
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
