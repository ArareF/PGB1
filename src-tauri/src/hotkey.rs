use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};
use windows::Win32::Foundation::HWND;

const HOTKEY_ID_SHORTCUT: i32 = 1001;
const HOTKEY_ID_CALCULATOR: i32 = 1002;
const TRANSLATOR_WINDOW_LABEL: &str = "translator";

/// 在独立线程中运行全局快捷键消息循环
pub fn start_hotkey_listener(app_handle: AppHandle, shortcut: String, use_calculator_key: bool) {
    std::thread::spawn(move || {
        unsafe {
            let registered = if use_calculator_key {
                // VK_LAUNCH_APP2 = 0xB7（计算器键），无修饰键
                RegisterHotKey(
                    HWND(std::ptr::null_mut()),
                    HOTKEY_ID_CALCULATOR,
                    HOT_KEY_MODIFIERS(0),
                    0xB7,
                )
                .is_ok()
            } else if let Some((modifiers, vk)) = parse_shortcut(&shortcut) {
                RegisterHotKey(
                    HWND(std::ptr::null_mut()),
                    HOTKEY_ID_SHORTCUT,
                    modifiers,
                    vk as u32,
                )
                .is_ok()
            } else {
                false
            };

            if !registered {
                // 快捷键注册失败（被其他程序占用或格式不支持），静默退出
                return;
            }

            // Win32 消息循环：阻塞等待 WM_HOTKEY 消息
            let mut msg = MSG::default();
            loop {
                let result = GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0);
                // result.0 == 0 表示 WM_QUIT，result.0 == -1 表示错误
                if result.0 == 0 || result.0 == -1 {
                    break;
                }
                if msg.message == WM_HOTKEY {
                    do_toggle_window(&app_handle);
                }
            }

            // 清理注册的快捷键
            let _ = UnregisterHotKey(HWND(std::ptr::null_mut()), HOTKEY_ID_SHORTCUT);
            let _ = UnregisterHotKey(HWND(std::ptr::null_mut()), HOTKEY_ID_CALCULATOR);
        }
    });
}

/// 切换翻译窗口显示/隐藏，窗口不存在时动态创建
pub fn do_toggle_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(TRANSLATOR_WINDOW_LABEL) {
        let is_visible = win.is_visible().unwrap_or(false);
        if is_visible {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.set_focus();
        }
    } else {
        create_translator_window(app);
    }
}

/// 动态创建翻译悬浮窗
fn create_translator_window(app: &AppHandle) {
    let url = WebviewUrl::App("/translator".into());

    match WebviewWindowBuilder::new(app, TRANSLATOR_WINDOW_LABEL, url)
        .title("翻译")
        .inner_size(400.0, 500.0)
        .resizable(true)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .center()
        .build()
    {
        Ok(win) => {
            let _ = win.show();
            #[cfg(target_os = "windows")]
            {
                // 延迟应用 Acrylic，确保窗口 HWND 完全初始化后再调用
                let win_clone = win.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    use window_vibrancy::apply_acrylic;
                    let _ = apply_acrylic(&win_clone, Some((0, 0, 0, 1)));
                });
            }
        }
        Err(e) => {
            log::error!("[hotkey] 创建翻译窗口失败: {}", e);
        }
    }
}

/// 解析快捷键字符串，返回 (HOT_KEY_MODIFIERS, 虚拟键码)
/// 支持格式：Ctrl+Shift+T、Control+Shift+T、Ctrl+Alt+T
fn parse_shortcut(s: &str) -> Option<(HOT_KEY_MODIFIERS, u16)> {
    let parts: Vec<&str> = s.split('+').collect();
    if parts.len() < 2 {
        return None;
    }

    let key_char = parts.last()?.trim();
    let modifier_parts = &parts[..parts.len() - 1];

    let mut modifiers = HOT_KEY_MODIFIERS(0);
    for part in modifier_parts {
        match part.trim().to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "shift" => modifiers |= MOD_SHIFT,
            "alt" => modifiers |= MOD_ALT,
            _ => {}
        }
    }

    // 仅支持单个 A-Z 字母键
    let vk = if key_char.len() == 1 {
        let c = key_char.chars().next()?.to_ascii_uppercase();
        if c.is_ascii_alphabetic() {
            c as u16
        } else {
            return None;
        }
    } else {
        return None;
    };

    Some((modifiers, vk))
}
