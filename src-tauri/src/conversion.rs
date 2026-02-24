use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::path::{Path, PathBuf};
use std::fs;
use tauri::{AppHandle, Emitter, Runtime};
use notify::{RecommendedWatcher, Event};
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// 转换会话状态
pub struct ConversionSession {
    pub imagine_pid: Option<u32>,
    pub done_path: PathBuf,
    /// 序列帧名称 -> 帧率
    pub sequence_fps_map: HashMap<String, u32>,
    /// 监控器句柄（必须存活，drop 即停止监控）
    pub _watcher: RecommendedWatcher,
    /// 外部工具路径
    pub texture_packer_cli: PathBuf,
    pub texture_packer_gui: PathBuf,
}

pub type ConversionState = Arc<Mutex<Option<ConversionSession>>>;

/// 处理文件监控事件的逻辑
/// scale_dir: 01_scale/ 目录路径
/// done_path: 02_done/ 目录路径
pub fn handle_file_event<R: Runtime>(
    event: Event,
    scale_dir: &Path,
    done_path: &Path,
    app_handle: &AppHandle<R>,
) {
    if !event.kind.is_create() {
        return;
    }

    for path in event.paths {
        if !path.is_file() { continue; }

        // 只处理 .webp 文件（imagine 输出格式）
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext != "webp" { continue; }

        // 父目录必须是 scale_dir/[XX]/ 形式（普通）
        // 或 scale_dir/[XX]/{subcat}/ 形式（Prototype）
        let parent = match path.parent() {
            Some(p) => p,
            None => continue,
        };

        // 确定 [XX] 目录和可选的子分类名（Prototype 专用）
        let (scale_parent, subcat) = if parent.parent() == Some(scale_dir) {
            // 普通任务: scale_dir/[XX]/file.webp
            (parent, None)
        } else {
            // Prototype: scale_dir/[XX]/{subcat}/file.webp
            let grandparent = match parent.parent() {
                Some(gp) => gp,
                None => continue,
            };
            if grandparent.parent() != Some(scale_dir) { continue; }
            let subcat_name = parent
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());
            (grandparent, subcat_name)
        };

        // 从 [XX] 目录名解析比例数字
        let dir_name = scale_parent.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !dir_name.starts_with('[') || !dir_name.ends_with(']') { continue; }
        let scale_str = &dir_name[1..dir_name.len() - 1];
        let scale: u32 = match scale_str.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        // 普通: 02_done/[img-XX]/file.webp
        // Prototype: 02_done/[img-XX]/{subcat}/file.webp
        let target_dir = match &subcat {
            Some(subcat_name) => done_path.join(format!("[img-{}]", scale)).join(subcat_name),
            None => done_path.join(format!("[img-{}]", scale)),
        };
        let app_handle_clone = app_handle.clone();
        let source = path.clone();
        let dest = target_dir.join(&file_name);

        tauri::async_runtime::spawn(async move {
            // 等待文件写入完成（防抖）
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            // 文件已不存在：同一文件触发了多次事件，前一次已处理，静默忽略
            if !source.exists() { return; }
            if !target_dir.exists() {
                let _ = fs::create_dir_all(&target_dir);
            }
            if let Err(e) = fs::rename(&source, &dest) {
                log::warn!("自动整理失败 ({}): {}", stem, e);
            } else {
                let _ = app_handle_clone.emit("conversion-organized", stem);
            }
        });
    }
}

pub fn bring_window_to_front(target_pid: u32) {
    unsafe {
        let _ = EnumWindows(Some(enum_windows_callback), LPARAM(target_pid as isize));
    }
}

unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let target_pid = lparam.0 as u32;
    let mut process_id = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    
    if process_id == target_pid {
        if IsWindowVisible(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            let _ = SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
            let _ = SetForegroundWindow(hwnd);
            return FALSE;
        }
    }
    TRUE
}
