use crate::models::{AttendanceConfig, AttendanceRecord};
use crate::scheduler::SchedulerState;
use std::fs;
use std::path::Path;
use tauri::Emitter;

// ─── 日报打卡命令 ─────────────────────────────────────────────

/// 加载日报打卡配置
#[tauri::command]
pub fn load_attendance_config(app_handle: tauri::AppHandle) -> Result<AttendanceConfig, String> {
    use tauri::Manager;
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("获取应用配置目录失败: {}", e))?;
    let config_path = config_dir.join("attendance_config.json");

    if !config_path.exists() {
        return Ok(AttendanceConfig::default());
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取日报打卡配置失败: {}", e))?;
    let config: AttendanceConfig = serde_json::from_str(&content)
        .map_err(|e| format!("解析日报打卡配置失败: {}", e))?;

    Ok(config)
}

/// 保存日报打卡配置（不含密码）
#[tauri::command]
pub fn save_attendance_config(
    app_handle: tauri::AppHandle,
    config: AttendanceConfig,
) -> Result<(), String> {
    use tauri::Manager;
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("获取应用配置目录失败: {}", e))?;

    // 确保目录存在
    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("创建配置目录失败: {}", e))?;

    let config_path = config_dir.join("attendance_config.json");
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化日报打卡配置失败: {}", e))?;
    fs::write(&config_path, json)
        .map_err(|e| format!("写入日报打卡配置失败: {}", e))?;

    Ok(())
}

/// 保存打卡密码到 Windows Credential Manager
#[tauri::command]
pub fn save_attendance_password(username: String, password: String) -> Result<(), String> {
    if username.is_empty() {
        return Err("请先填写账号".to_string());
    }
    let entry = keyring::Entry::new("pgb1-attendance", &username)
        .map_err(|e| format!("创建凭据条目失败: {}", e))?;
    entry
        .set_password(&password)
        .map_err(|e| format!("保存密码失败: {}", e))?;
    Ok(())
}

/// 读取打卡密码
#[tauri::command]
pub fn load_attendance_password(username: String) -> Result<String, String> {
    if username.is_empty() {
        return Ok(String::new());
    }
    let entry = keyring::Entry::new("pgb1-attendance", &username)
        .map_err(|e| format!("创建凭据条目失败: {}", e))?;
    match entry.get_password() {
        Ok(pwd) => Ok(pwd),
        Err(keyring::Error::NoEntry) => Ok(String::new()),
        Err(e) => Err(format!("读取密码失败: {}", e)),
    }
}

// ─── 打卡自动化辅助函数 ─────────────────────────────────────────

/// 内部加载打卡记录（不依赖 app_handle）
fn load_attendance_record_internal(path: &Path) -> AttendanceRecord {
    if path.exists() {
        fs::read_to_string(path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        AttendanceRecord::default()
    }
}

/// 内部保存打卡记录（不依赖 app_handle）
fn save_attendance_record_internal(path: &Path, record: &AttendanceRecord) {
    if let Ok(json) = serde_json::to_string_pretty(record) {
        let _ = fs::write(path, json);
    }
}

// ─── 打卡自动化命令 ─────────────────────────────────────────────

/// 执行打卡自动化（出勤或退勤）— 后台 spawn，立刻返回
#[tauri::command]
pub fn execute_clock_action(
    app_handle: tauri::AppHandle,
    action: String,
) -> Result<String, String> {
    let app = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = execute_clock_action_inner(app.clone(), action).await {
            let _ = app.emit("clock-progress", serde_json::json!({
                "step": "error",
                "message": e.to_string(),
            }));
        }
    });
    Ok("打卡任务已启动".to_string())
}

/// 向前端推送打卡进度
fn emit_progress(app: &tauri::AppHandle, step: &str, message: &str) {
    let _ = app.emit("clock-progress", serde_json::json!({
        "step": step,
        "message": message,
    }));
}

/// 获取 WebView 当前 URL（Tauri 原生 API，不依赖 document.title）
fn get_webview_url(webview: &tauri::WebviewWindow) -> String {
    webview.url().map(|u| u.to_string()).unwrap_or_default()
}

/// 从 WebView URL hash 读取状态（格式：#__pgb1_STATE）
fn read_webview_hash_state(webview: &tauri::WebviewWindow) -> String {
    webview
        .url()
        .ok()
        .and_then(|u| u.fragment().map(String::from))
        .and_then(|f| f.strip_prefix("__pgb1_").map(String::from))
        .unwrap_or_default()
}

/// 共享的 WebView 登录流程：创建窗口 → 打开打卡网站 → 填写凭据 → 登录
/// 返回 (WebviewWindow, 配置URL, 应用配置目录)
async fn webview_login_flow(
    app_handle: &tauri::AppHandle,
    config: &AttendanceConfig,
    password: &str,
    label: &str,
    visible: bool,
    emit_fn: fn(&tauri::AppHandle, &str, &str),
) -> Result<(tauri::WebviewWindow, String), String> {
    use tauri::Manager;

    emit_fn(app_handle, "opening-page", "正在打开打卡网站...");

    // 1. 创建 WebView 窗口（关闭已有同名窗口）
    if let Some(existing) = app_handle.get_webview_window(label) {
        let _ = existing.close();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let url = config.attendance.url.clone();
    let mut builder = tauri::WebviewWindowBuilder::new(
        app_handle,
        label,
        tauri::WebviewUrl::External(url.parse().map_err(|e| format!("URL 无效: {}", e))?),
    )
    .title(if visible { "打卡测试" } else { "PGB1 打卡" })
    .inner_size(1024.0, 768.0)
    .visible(visible);
    if visible {
        builder = builder.center();
    }
    let webview_window = builder
        .build()
        .map_err(|e| format!("创建打卡 WebView 失败: {}", e))?;

    // 2. 等待页面加载
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // 如果已在打刻页（可能 URL 直指打刻页或已登录），跳过登录
    let current_url = get_webview_url(&webview_window);
    if current_url.contains("record/register") {
        return Ok((webview_window, url));
    }

    emit_fn(app_handle, "logging-in", "正在登录...");

    // 3. 填写账号
    let fill_username_js = format!(
        r#"(function() {{
            var el = document.querySelector('input[type="email"], input[name="username"], input[name="email"], input[type="text"]');
            if (el) {{ el.value = '{}'; el.dispatchEvent(new Event('input', {{bubbles: true}})); return 'ok'; }}
            return 'not_found';
        }})()"#,
        config.username.replace('\'', "\\'").replace('"', "\\\"")
    );
    let _ = webview_window.eval(&fill_username_js);

    // 4. 填写密码
    let fill_password_js = format!(
        r#"(function() {{
            var el = document.querySelector('input[type="password"]');
            if (el) {{ el.value = '{}'; el.dispatchEvent(new Event('input', {{bubbles: true}})); return 'ok'; }}
            return 'not_found';
        }})()"#,
        password.replace('\'', "\\'").replace('"', "\\\"")
    );
    let _ = webview_window.eval(&fill_password_js);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // 5. 点击登录按钮（模拟完整鼠标事件链）
    let login_js = r#"(function() {
        function simulateClick(el) {
            var rect = el.getBoundingClientRect();
            var cx = rect.left + rect.width / 2;
            var cy = rect.top + rect.height / 2;
            var opts = {bubbles: true, cancelable: true, view: window, clientX: cx, clientY: cy};
            el.dispatchEvent(new MouseEvent('mousedown', opts));
            el.dispatchEvent(new MouseEvent('mouseup', opts));
            el.dispatchEvent(new MouseEvent('click', opts));
        }
        var buttons = document.querySelectorAll('button, input[type="submit"]');
        for (var i = 0; i < buttons.length; i++) {
            var text = buttons[i].textContent || buttons[i].value || '';
            if (text.indexOf('ログイン') >= 0 || text.indexOf('Login') >= 0 || text.indexOf('login') >= 0) {
                if (!buttons[i].disabled) { simulateClick(buttons[i]); return 'clicked'; }
            }
        }
        return 'not_found';
    })()"#;
    let _ = webview_window.eval(login_js);

    // 6. 轮询等待登录跳转（URL 离开 login 页即成功，最多 10 秒）
    let mut login_ok = false;
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let u = get_webview_url(&webview_window);
        if !u.is_empty() && !u.contains("login") {
            login_ok = true;
            break;
        }
    }
    if !login_ok {
        return Err("登录失败：账号或密码错误，请前往「程序设置 → 日报打卡」检查账号和密码".to_string());
    }

    Ok((webview_window, url))
}

/// 加载打卡配置和密码（execute/test 共用前置校验）
fn load_clock_config_and_password(
    app_handle: &tauri::AppHandle,
) -> Result<(AttendanceConfig, String, std::path::PathBuf), String> {
    use tauri::Manager;

    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {}", e))?;
    let config_path = config_dir.join("attendance_config.json");

    let config: AttendanceConfig = if config_path.exists() {
        let content =
            fs::read_to_string(&config_path).map_err(|e| format!("读取配置失败: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("解析配置失败: {}", e))?
    } else {
        return Err("请先配置日报打卡设置".to_string());
    };

    if config.attendance.url.is_empty() {
        return Err("打卡网站 URL 未配置".to_string());
    }
    if config.username.is_empty() {
        return Err("账号未配置".to_string());
    }

    let password = {
        let entry = keyring::Entry::new("pgb1-attendance", &config.username)
            .map_err(|e| format!("读取凭据失败: {}", e))?;
        match entry.get_password() {
            Ok(pwd) => pwd,
            Err(keyring::Error::NoEntry) => return Err("密码未配置".to_string()),
            Err(e) => return Err(format!("读取密码失败: {}", e)),
        }
    };

    Ok((config, password, config_dir))
}

/// 打卡自动化内部实现
async fn execute_clock_action_inner(
    app_handle: tauri::AppHandle,
    action: String,
) -> Result<String, String> {
    emit_progress(&app_handle, "loading-config", "正在加载配置...");

    // 1. 加载配置和密码
    let (config, password, config_dir) = load_clock_config_and_password(&app_handle)?;

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

    // ── WebView 自动化登录 ─────────────
    let (webview_window, url) = webview_login_flow(
        &app_handle, &config, &password, "webview-clock", false, emit_progress,
    ).await?;

    emit_progress(&app_handle, "navigating", "正在进入打刻页面...");

    // 8. 导航到打刻页面
    //    直接从 URL origin 构造打刻页地址并跳转，比「找链接 → 模拟点击」更可靠
    let current_url = get_webview_url(&webview_window);
    if !current_url.contains("record/register") {
        // 从配置 URL 提取 origin（如 https://timecard.yenbo.jp）
        let origin = if let Some(scheme_end) = url.find("://") {
            let after_scheme = &url[scheme_end + 3..];
            if let Some(slash) = after_scheme.find('/') {
                &url[..scheme_end + 3 + slash]
            } else {
                url.as_str()
            }
        } else {
            url.as_str()
        };
        let register_url = format!("{}/record/register.html", origin);
        let nav_js = format!(r#"window.location.href = '{}'"#, register_url);
        let _ = webview_window.eval(&nav_js);

        // 轮询等待打刻页加载（最多 10 秒）
        let mut reached = false;
        for _ in 0..20 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if get_webview_url(&webview_window).contains("record/register") {
                reached = true;
                break;
            }
        }
        if !reached {
            let u = get_webview_url(&webview_window);
            return Err(format!(
                "未能进入打刻页面（当前页：{}），请检查打卡网站是否正常",
                u
            ));
        }
    }

    // 打刻页已就绪，等待页面 JS 初始化完成
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    emit_progress(&app_handle, "finding-button", "正在查找打卡按钮...");

    // 9. 轮询等待目标按钮变为可点击（resetStatus 完成后按钮才会启用）
    //    Tauri eval() 无返回值，用 URL hash (#__pgb1_STATE) 做状态通信
    let button_text = if action == "clock_in" { "出勤" } else { "退勤" };

    let mut button_state = String::from("not_found");
    let mut disabled_count = 0u32; // 连续 disabled 计数，防止页面初始化期间误判
    const DISABLED_CONFIRM_THRESHOLD: u32 = 8; // 连续 8 次 disabled（≈2 秒）才认定已打卡
    for _ in 0..60 {
        // 最多等 15 秒，每 250ms 检查一次
        let hash_js = format!(
            r#"(function() {{
                var elements = document.querySelectorAll('button, input[type="button"], input[type="submit"]');
                for (var i = 0; i < elements.length; i++) {{
                    var text = elements[i].textContent || elements[i].value || '';
                    if (text.indexOf('{}') >= 0) {{
                        var s = elements[i].disabled ? 'disabled' : 'ready';
                        try {{ history.replaceState(null, '', location.pathname + location.search + '#__pgb1_' + s); }} catch(e) {{}}
                        return;
                    }}
                }}
                try {{ history.replaceState(null, '', location.pathname + location.search + '#__pgb1_not_found'); }} catch(e) {{}}
            }})()"#,
            button_text
        );
        let _ = webview_window.eval(&hash_js);
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;

        let state = read_webview_hash_state(&webview_window);
        if !state.is_empty() {
            button_state = state.clone();
            if state == "ready" {
                break;
            }
            if state == "disabled" {
                disabled_count += 1;
                // 连续多次确认 disabled 才认定已打卡（过滤页面初始化的瞬态 disabled）
                if disabled_count >= DISABLED_CONFIRM_THRESHOLD {
                    break;
                }
            } else {
                disabled_count = 0;
            }
        }
    }

    // 判断按钮状态
    if button_state == "disabled" && disabled_count >= DISABLED_CONFIRM_THRESHOLD {
        // 按钮持续禁用 = 已经打过卡了（不覆盖已有的 actual 时间）
        let now = chrono::Local::now();
        let today = now.format("%Y-%m-%d").to_string();
        let record_path = config_dir.join("attendance_record.json");
        let mut record = load_attendance_record_internal(&record_path);
        if action == "clock_in" {
            record.last_clock_in = Some(today);
            // 仅在未记录时补写（避免覆盖已有的实际时间）
            if record.actual_clock_in_time.is_none() {
                record.actual_clock_in_time = Some(now.format("%H:%M").to_string());
            }
        } else {
            record.last_clock_out = Some(today);
            if record.actual_clock_out_time.is_none() {
                record.actual_clock_out_time = Some(now.format("%H:%M").to_string());
            }
        }
        save_attendance_record_internal(&record_path, &record);

        let _ = webview_window.hide();
        emit_progress(&app_handle, "already-done", "今天已经打过卡了");
        return Ok("已打卡（按钮已禁用）".to_string());
    }

    if button_state != "ready" {
        let reason = if button_state == "not_found" {
            "打刻页面结构可能已变化，未找到对应按钮"
        } else if button_state == "disabled" {
            "按钮短暂禁用后未恢复，页面可能未完全加载，请稍后重试"
        } else {
            "按钮未在 15 秒内变为可用状态，请稍后重试"
        };
        let _ = webview_window.close();
        return Err(format!("无法点击「{}」：{}", button_text, reason));
    }

    emit_progress(&app_handle, "clicking", &format!("正在点击「{}」...", button_text));

    // 10. 按钮可用，执行点击（模拟完整鼠标事件链）
    let clock_js = format!(
        r#"(function() {{
            function simulateClick(el) {{
                var rect = el.getBoundingClientRect();
                var cx = rect.left + rect.width / 2;
                var cy = rect.top + rect.height / 2;
                var opts = {{bubbles: true, cancelable: true, view: window, clientX: cx, clientY: cy}};
                el.dispatchEvent(new MouseEvent('mousedown', opts));
                el.dispatchEvent(new MouseEvent('mouseup', opts));
                el.dispatchEvent(new MouseEvent('click', opts));
            }}
            var elements = document.querySelectorAll('button, input[type="button"], input[type="submit"]');
            for (var i = 0; i < elements.length; i++) {{
                var text = elements[i].textContent || elements[i].value || '';
                if (text.indexOf('{}') >= 0) {{
                    simulateClick(elements[i]);
                    return 'clicked';
                }}
            }}
            return 'not_found';
        }})()"#,
        button_text
    );
    let _ = webview_window.eval(&clock_js);

    emit_progress(&app_handle, "verifying", "正在验证打卡结果...");

    // 11. 轮询验证：等待按钮变为 disabled（服务端确认打卡成功）
    let mut confirmed = false;
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let verify_js = format!(
            r#"(function() {{
                var elements = document.querySelectorAll('button, input[type="button"], input[type="submit"]');
                for (var i = 0; i < elements.length; i++) {{
                    var text = elements[i].textContent || elements[i].value || '';
                    if (text.indexOf('{}') >= 0) {{
                        var s = elements[i].disabled ? 'confirmed' : 'pending';
                        try {{ history.replaceState(null, '', location.pathname + location.search + '#__pgb1_' + s); }} catch(e) {{}}
                        return;
                    }}
                }}
            }})()"#,
            button_text
        );
        let _ = webview_window.eval(&verify_js);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        if read_webview_hash_state(&webview_window) == "confirmed" {
            confirmed = true;
            break;
        }
    }

    // 12. 根据验证结果更新记录
    let now = chrono::Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let actual_time = now.format("%H:%M").to_string();
    let record_path = config_dir.join("attendance_record.json");

    if confirmed {
        let mut record = load_attendance_record_internal(&record_path);
        if action == "clock_in" {
            record.last_clock_in = Some(today);
            record.actual_clock_in_time = Some(actual_time);
        } else {
            record.last_clock_out = Some(today);
            record.actual_clock_out_time = Some(actual_time);
        }
        save_attendance_record_internal(&record_path, &record);

        let _ = webview_window.hide();
        emit_progress(&app_handle, "success", "打卡成功");
        Ok("打卡成功".to_string())
    } else {
        // 未确认：不写入记录，不关闭 WebView（保留现场供用户检查）
        emit_progress(&app_handle, "unconfirmed", "打卡操作已执行但未确认，请点击「查看结果」手动检查");
        Ok("打卡操作已执行但未确认".to_string())
    }
}

/// 测试打卡连接（可见 WebView）— 走完登录→打刻页流程，但不点击打卡按钮
#[tauri::command]
pub fn test_clock_action(
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let app = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = test_clock_action_inner(app.clone()).await {
            let _ = app.emit(
                "clock-test-progress",
                serde_json::json!({ "step": "error", "message": e.to_string() }),
            );
        }
    });
    Ok("测试已启动".to_string())
}

fn emit_test_progress(app: &tauri::AppHandle, step: &str, message: &str) {
    let _ = app.emit(
        "clock-test-progress",
        serde_json::json!({ "step": step, "message": message }),
    );
}

async fn test_clock_action_inner(app_handle: tauri::AppHandle) -> Result<(), String> {
    emit_test_progress(&app_handle, "loading-config", "正在加载配置...");

    // 1. 加载配置和密码
    let (config, password, _config_dir) = load_clock_config_and_password(&app_handle)?;

    // 2. WebView 登录流程（可见窗口）
    let (webview_window, url) = webview_login_flow(
        &app_handle, &config, &password, "webview-clock-test", true, emit_test_progress,
    ).await?;

    let current_url = get_webview_url(&webview_window);

    // 如果登录后已经在打刻页
    if current_url.contains("record/register") {
        emit_test_progress(&app_handle, "success", "登录成功，已到达打刻页面，测试通过！");
        return Ok(());
    }

    emit_test_progress(&app_handle, "navigating", "登录成功，正在导航到打刻页...");

    // 8. 直接导航到打刻页
    let origin = if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];
        if let Some(slash) = after_scheme.find('/') {
            &url[..scheme_end + 3 + slash]
        } else {
            url.as_str()
        }
    } else {
        url.as_str()
    };
    let register_url = format!("{}/record/register.html", origin);
    let nav_js = format!(r#"window.location.href = '{}'"#, register_url);
    let _ = webview_window.eval(&nav_js);

    // 轮询等待（最多 10 秒）
    let mut reached = false;
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let u = get_webview_url(&webview_window);
        if u.contains("record/register") {
            reached = true;
            break;
        }
    }

    let final_url = get_webview_url(&webview_window);
    if !reached {
        emit_test_progress(
            &app_handle,
            "error",
            &format!("未能到达打刻页面，当前停留在：{}", final_url),
        );
        return Ok(());
    }

    emit_test_progress(
        &app_handle,
        "finding-button",
        &format!("已到达打刻页：{}，正在查找「休憩入」按钮...", final_url),
    );

    // 等待页面 JS 初始化
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // 9. 轮询查找「出勤」按钮（用 URL hash 通信）
    let mut button_state = String::from("not_found");
    for _ in 0..60 {
        let hash_js = r#"(function() {
            var el = document.getElementById('work-start');
            if (el) {
                var s = el.disabled ? 'disabled' : 'ready';
                try { history.replaceState(null, '', location.pathname + location.search + '#__pgb1_' + s); } catch(e) {}
                return;
            }
            var elements = document.querySelectorAll('button, input[type="button"]');
            for (var i = 0; i < elements.length; i++) {
                var text = elements[i].textContent || elements[i].value || '';
                if (text.indexOf('出勤') >= 0) {
                    var s = elements[i].disabled ? 'disabled' : 'ready';
                    try { history.replaceState(null, '', location.pathname + location.search + '#__pgb1_' + s); } catch(e) {}
                    return;
                }
            }
            try { history.replaceState(null, '', location.pathname + location.search + '#__pgb1_not_found'); } catch(e) {}
        })()"#;
        let _ = webview_window.eval(hash_js);
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;

        let state = read_webview_hash_state(&webview_window);
        if !state.is_empty() {
            button_state = state.clone();
            if button_state == "ready" || button_state == "disabled" {
                break;
            }
        }
    }

    if button_state == "not_found" {
        emit_test_progress(
            &app_handle,
            "error",
            "未找到「出勤」按钮，页面结构可能已变化",
        );
        return Ok(());
    }

    // 10. 高亮「出勤」按钮（绿色呼吸边框），不点击
    let highlight_js = r#"(function() {
        var style = document.createElement('style');
        style.textContent = '@keyframes __pgb1_pulse{0%,100%{box-shadow:0 0 8px 2px rgba(34,197,94,.6)}50%{box-shadow:0 0 20px 6px rgba(34,197,94,.9)}}';
        document.head.appendChild(style);
        var el = document.getElementById('work-start');
        if (!el) {
            var elements = document.querySelectorAll('button, input[type="button"]');
            for (var i = 0; i < elements.length; i++) {
                if ((elements[i].textContent || '').indexOf('出勤') >= 0) { el = elements[i]; break; }
            }
        }
        if (el) {
            el.style.outline = '3px solid #22c55e';
            el.style.outlineOffset = '3px';
            el.style.animation = '__pgb1_pulse 1.5s ease-in-out infinite';
            el.scrollIntoView({behavior:'smooth',block:'center'});
        }
    })()"#;
    let _ = webview_window.eval(highlight_js);

    let status_text = if button_state == "disabled" { "已禁用（今天已打卡）" } else { "可点击" };
    emit_test_progress(
        &app_handle,
        "success",
        &format!("测试通过！已找到「出勤」按钮（{}），请查看浏览器窗口", status_text),
    );

    Ok(())
}

/// 显示打卡 WebView 窗口（让用户查看结果）
#[tauri::command]
pub fn show_clock_webview(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    if let Some(window) = app_handle.get_webview_window("webview-clock") {
        window
            .show()
            .map_err(|e| format!("显示窗口失败: {}", e))?;
        window
            .set_focus()
            .map_err(|e| format!("聚焦窗口失败: {}", e))?;
        Ok(())
    } else {
        Err("打卡 WebView 窗口不存在".to_string())
    }
}

/// 关闭打卡 WebView 窗口
#[tauri::command]
pub fn close_clock_webview(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    if let Some(window) = app_handle.get_webview_window("webview-clock") {
        window
            .close()
            .map_err(|e| format!("关闭窗口失败: {}", e))?;
    }
    Ok(())
}

/// 日报 WebView 初始化脚本（SSOT）
/// scheduler 预热 + open_daily_report 共用此脚本
/// 通过 initialization_script 注入，在页面 JS 运行前执行
/// 抢先保存原生 replaceState 引用，绕过 Google CSP
/// 两条件：readyState=complete + .kix-cursor-caret（编辑器可交互的最终信号）
pub(crate) const DAILY_REPORT_INIT_JS: &str = r#"(function(){
    var _rs=history.replaceState.bind(history);
    var _t=setInterval(function(){
        if(location.hash==='#pgb-ready'){clearInterval(_t);return}
        var r=document.readyState==='complete';
        var c=!!document.querySelector('.kix-cursor-caret');
        var tag='#pgb-'+(+r)+(+c);
        if(r&&c)tag='#pgb-ready';
        if(location.hash!==tag){
            try{_rs(null,'',location.href.split('#')[0]+tag)}catch(e){}
        }
    },500);
})();"#;

/// 后台轮询日报 WebView 就绪状态，就绪后聚焦并滚到文档末尾
/// 预热命中时（hash 已为 #pgb-ready），跳过轮询直接执行滚动
/// force_immediate=true：跳过 hash 轮询，直接执行聚焦+滚动。
/// 适用于"窗口已可见（用户之前打开过）"的场景——页面肯定已加载，
/// 但 Google Docs 自行修改了 URL hash，导致 #pgb-ready 已不存在。
fn spawn_daily_report_scroll(window: tauri::WebviewWindow, force_immediate: bool) {
    tauri::async_runtime::spawn(async move {
        // 提前提取 HWND raw（isize 满足 Send），不跨 await 点持有 HWND(*mut c_void)
        #[cfg(target_os = "windows")]
        let hwnd_raw: Option<isize> = window.hwnd().ok().map(|h| {
            unsafe { *(&h as *const _ as *const isize) }
        });

        // force_immediate：窗口已可见/已加载，直接跳过轮询
        // 否则：检查是否已就绪（预热场景：init_script 已跑完，hash 为 #pgb-ready）
        let already_ready = force_immediate || window
            .url()
            .ok()
            .and_then(|u| u.fragment().map(|f| f == "pgb-ready"))
            .unwrap_or(false);

        let detected = if already_ready {
            if force_immediate {
                eprintln!("[daily-report] 窗口已可见，跳过轮询直接滚动");
            } else {
                eprintln!("[daily-report] 预热命中，页面已就绪");
            }
            true
        } else {
            eprintln!("[daily-report] 页面未就绪，开始轮询...");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            let mut iteration: u32 = 0;
            let mut result = false;

            // 无超时：Google Docs 加载时间不可预测，退出条件仅 #pgb-ready 或窗口关闭
            loop {
                iteration += 1;
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                match window.url() {
                    Err(e) => {
                        eprintln!("[daily-report] 第{}次: url() 失败: {:?}，窗口已关闭", iteration, e);
                        break;
                    }
                    Ok(url) => {
                        let fragment = url.fragment().unwrap_or("(none)");
                        eprintln!("[daily-report] 第{}次 hash: {}", iteration, fragment);

                        if fragment == "pgb-ready" {
                            eprintln!("[daily-report] 编辑器就绪！");
                            result = true;
                            break;
                        }
                    }
                }
            }
            result
        };

        // 就绪后的动作序列：JS focus → OS 聚焦 → Ctrl+End
        if detected {
            let _ = window.eval("var e=document.querySelector('.kix-appview-editor');if(e)e.focus()");
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            #[cfg(target_os = "windows")]
            {
                use windows::Win32::Foundation::HWND;
                use windows::Win32::UI::WindowsAndMessaging::{
                    ShowWindow, SW_RESTORE, SetWindowPos,
                    HWND_TOPMOST, HWND_NOTOPMOST,
                    SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetForegroundWindow,
                };
                if let Some(raw) = hwnd_raw {
                    unsafe {
                        let hwnd = HWND(raw as *mut core::ffi::c_void);
                        let _ = ShowWindow(hwnd, SW_RESTORE);
                        let _ = SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
                        let _ = SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
                        let _ = SetForegroundWindow(hwnd);
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;

            #[cfg(target_os = "windows")]
            {
                send_ctrl_end();
                eprintln!("[daily-report] send_ctrl_end() 已发送");
            }
        }

        eprintln!("[daily-report] 后台检测任务结束");
    });
}

/// 打开日报 WebView 窗口
#[tauri::command]
pub async fn open_daily_report(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    // 加载配置
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {}", e))?;
    let config_path = config_dir.join("attendance_config.json");

    let config: AttendanceConfig = if config_path.exists() {
        let content =
            fs::read_to_string(&config_path).map_err(|e| format!("读取配置失败: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("解析配置失败: {}", e))?
    } else {
        return Err("请先配置日报打卡设置".to_string());
    };

    if config.daily_report.url.is_empty() {
        return Err("日报网站 URL 未配置".to_string());
    }

    let label = "daily-report";

    // 如果已存在（包含预热创建的隐藏窗口），显示 + 聚焦 + 滚动
    if let Some(existing) = app_handle.get_webview_window(label) {
        // 窗口已可见：说明用户之前就打开过，页面肯定已加载完成（但 Google Docs 可能已修改
        // 了 URL hash，导致 #pgb-ready 已不存在）。跳过 hash 轮询直接滚动。
        // 窗口不可见：预热创建的隐藏窗口，页面可能还在加载，走正常 hash 轮询。
        let force_immediate = existing.is_visible().unwrap_or(false);
        let _ = existing.show();
        let _ = existing.set_focus();
        spawn_daily_report_scroll(existing, force_immediate);
        return Ok(());
    }

    let url = config.daily_report.url.clone();

    let window = tauri::WebviewWindowBuilder::new(
        &app_handle,
        label,
        tauri::WebviewUrl::External(url.parse().map_err(|e| format!("URL 无效: {}", e))?),
    )
    .title("PGB1 日报")
    .inner_size(1200.0, 800.0)
    .center()
    .initialization_script(DAILY_REPORT_INIT_JS)
    .build()
    .map_err(|e| format!("创建日报窗口失败: {}", e))?;

    // 后台轮询就绪状态 + 聚焦 + 滚到底部（新窗口，不跳过轮询）
    spawn_daily_report_scroll(window, false);

    Ok(())
}

/// 测试提醒弹窗（设置页用）
/// 必须 spawn 到 async runtime：sync 命令在线程池调 WebviewWindowBuilder::build()
/// 会 run_on_main_thread 等主线程，而主线程正阻塞等命令返回 → 死锁
#[tauri::command]
pub fn test_reminder(app_handle: tauri::AppHandle, reminder_type: String) -> Result<(), String> {
    let app = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::scheduler::create_reminder_window(&app, &reminder_type) {
            log::error!("创建提醒窗口失败: {}", e);
        }
    });
    Ok(())
}

/// 加载本地打卡记录
#[tauri::command]
pub fn load_attendance_record(app_handle: tauri::AppHandle) -> Result<AttendanceRecord, String> {
    use tauri::Manager;

    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {}", e))?;
    let record_path = config_dir.join("attendance_record.json");

    if !record_path.exists() {
        return Ok(AttendanceRecord::default());
    }

    let content =
        fs::read_to_string(&record_path).map_err(|e| format!("读取打卡记录失败: {}", e))?;
    let record: AttendanceRecord =
        serde_json::from_str(&content).map_err(|e| format!("解析打卡记录失败: {}", e))?;

    Ok(record)
}

/// 保存本地打卡记录
#[tauri::command]
pub fn save_attendance_record(
    app_handle: tauri::AppHandle,
    record: AttendanceRecord,
) -> Result<(), String> {
    use tauri::Manager;

    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {}", e))?;

    fs::create_dir_all(&config_dir).map_err(|e| format!("创建配置目录失败: {}", e))?;

    let record_path = config_dir.join("attendance_record.json");
    let json =
        serde_json::to_string_pretty(&record).map_err(|e| format!("序列化打卡记录失败: {}", e))?;
    fs::write(&record_path, json).map_err(|e| format!("写入打卡记录失败: {}", e))?;

    Ok(())
}

/// 记录用户已关闭今日出勤提醒（防止重启后补打检测再次弹出）
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

/// 创建加班定时提醒
#[tauri::command]
pub fn schedule_overtime_reminder(
    app_handle: tauri::AppHandle,
    scheduler: tauri::State<'_, SchedulerState>,
    minutes: u64,
) -> Result<(), String> {
    let mut sched = scheduler
        .lock()
        .map_err(|e| format!("获取调度器锁失败: {}", e))?;
    sched.schedule_overtime(app_handle, minutes);
    Ok(())
}

/// 显示加班设置弹窗
/// 必须 spawn 到 async runtime（同 test_reminder，防止 build() 主线程死锁）
#[tauri::command]
pub fn show_overtime_dialog(app_handle: tauri::AppHandle) -> Result<(), String> {
    let app = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        use tauri::Manager;

        let label = "reminder-overtime-setting";

        // 如果已存在，聚焦
        if let Some(existing) = app.get_webview_window(label) {
            let _ = existing.show();
            let _ = existing.set_focus();
            return;
        }

        let window = match tauri::WebviewWindowBuilder::new(
            &app,
            label,
            tauri::WebviewUrl::App("/overtime".into()),
        )
        .title("PGB1")
        .inner_size(400.0, 200.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .center()
        .build()
        {
            Ok(w) => w,
            Err(e) => {
                log::error!("创建加班设置窗口失败: {}", e);
                return;
            }
        };

        #[cfg(target_os = "windows")]
        {
            use window_vibrancy::apply_acrylic;
            let _ = apply_acrylic(&window, Some((0, 0, 0, 1)));
        }
    });
    Ok(())
}

/// 重置所有定时任务（设置保存后调用）
#[tauri::command]
pub fn reschedule_attendance(
    app_handle: tauri::AppHandle,
    scheduler: tauri::State<'_, SchedulerState>,
) -> Result<(), String> {
    use tauri::Manager;

    // 重新加载配置
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {}", e))?;
    let config_path = config_dir.join("attendance_config.json");

    let config: AttendanceConfig = if config_path.exists() {
        let content =
            fs::read_to_string(&config_path).map_err(|e| format!("读取配置失败: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("解析配置失败: {}", e))?
    } else {
        AttendanceConfig::default()
    };

    let mut sched = scheduler
        .lock()
        .map_err(|e| format!("获取调度器锁失败: {}", e))?;
    sched.reschedule(app_handle, &config);

    Ok(())
}

/// 通过 Win32 SendInput 发送真实 Ctrl+End 按键
/// Google Docs canvas 编辑器只响应真实系统按键，合成 DOM KeyboardEvent 无效
#[cfg(target_os = "windows")]
/// 将光标移到 hwnd 窗口中央，发送大量鼠标滚轮向下事件滚到底部
/// MOUSEEVENTF_WHEEL 事件送往光标下方的窗口，不需要键盘焦点，
/// 比 Ctrl+End 键盘方案更可靠（绕过 WebView2 焦点链难题）
#[cfg(target_os = "windows")]
#[allow(dead_code)]
unsafe fn scroll_to_bottom_via_wheel(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, SetCursorPos};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEINPUT, MOUSEEVENTF_WHEEL,
    };

    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect).is_err() {
        return;
    }
    // 移到窗口中央（文档内容区），SetCursorPos 用屏幕像素坐标，多显示器也正确
    let cx = (rect.left + rect.right) / 2;
    let cy = (rect.top + rect.bottom) / 2;
    let _ = SetCursorPos(cx, cy);

    // WHEEL_DELTA = 120 per notch；负值 = 向下滚动
    // 发送 500 次 × -120 = 总计 -60000，足以滚过数百页文档
    let wheel_event = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0, dy: 0,
                mouseData: (-120i32) as u32, // u32 存负数，Windows 按 i32 解释为向下
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0, dwExtraInfo: 0,
            },
        },
    };
    let batch = vec![wheel_event; 500];
    SendInput(&batch, std::mem::size_of::<INPUT>() as i32);
}

fn send_ctrl_end() {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;

    const VK_CONTROL_U16: u16 = 0x11; // VK_CONTROL
    const VK_END_U16: u16 = 0x23;     // VK_END

    let inputs = [
        // Ctrl down
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(VK_CONTROL_U16),
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // End down
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(VK_END_U16),
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // End up
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(VK_END_U16),
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // Ctrl up
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(VK_CONTROL_U16),
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];

    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}
