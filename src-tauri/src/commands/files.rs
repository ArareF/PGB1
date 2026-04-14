use crate::models::ProjectConfig;
use super::helpers::matches_base_name;
use std::fs;
use std::path::Path;

/// 在系统文件管理器中打开指定路径
#[tauri::command]
pub fn open_in_explorer(path: String) -> Result<(), String> {
    let target = Path::new(&path);
    if !target.exists() {
        return Err(format!("路径不存在: {}", path));
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(target).spawn()
            .map_err(|e| format!("打开文件管理器失败: {}", e))?;
    }
    Ok(())
}

/// 用系统关联程序打开指定文件
#[tauri::command]
pub fn open_file(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() { return Err(format!("文件不存在: {}", path)); }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let path_wide: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let verb: Vec<u16> = "open\0".encode_utf16().collect();
        let result = unsafe { ShellExecuteW(None, PCWSTR(verb.as_ptr()), PCWSTR(path_wide.as_ptr()), None, None, SW_SHOWNORMAL) };
        if (result.0 as isize) <= 32 { return Err(format!("打开文件失败，错误码: {:?}", result.0)); }
    }
    Ok(())
}

/// 获取文件的修改时间戳（Unix 秒）
#[tauri::command]
pub fn get_file_mtime(path: String) -> Result<u64, String> {
    fs::metadata(&path).and_then(|m| m.modified())
        .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
        .map_err(|e| format!("获取文件信息失败: {}", e))
}

/// 读取文本文件内容（UTF-8）
#[tauri::command]
pub fn read_text_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| format!("读取文件失败: {}", e))
}

/// 重命名素材（所有工作流版本同步改名，包括 nextcloud）
#[tauri::command]
pub fn rename_material(task_path: String, base_name: String, new_base_name: String, material_type: String) -> Result<(), String> {
    let task_dir = Path::new(&task_path);
    let is_sequence = material_type == "sequence";
    let mut dirs_to_scan: Vec<std::path::PathBuf> = vec![task_dir.join("00_original")];
    let scale_dir = task_dir.join("01_scale");
    if scale_dir.exists() { if let Ok(entries) = fs::read_dir(&scale_dir) { for e in entries.flatten() { if e.path().is_dir() { dirs_to_scan.push(e.path()); } } } }
    let done_dir = task_dir.join("02_done");
    if done_dir.exists() { if let Ok(entries) = fs::read_dir(&done_dir) { for e in entries.flatten() { if e.path().is_dir() { dirs_to_scan.push(e.path()); } } } }
    let nc_dir = task_dir.parent().and_then(|p| p.parent()).map(|vfx| vfx.join("nextcloud").join(task_dir.file_name().unwrap_or_default()));
    if let Some(ref nc) = nc_dir { if nc.exists() { dirs_to_scan.push(nc.clone()); } }

    for dir in &dirs_to_scan {
        if !dir.exists() { continue; }
        let entries = match fs::read_dir(dir) { Ok(e) => e, Err(_) => continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|n| n.to_str()) { Some(n) => n.to_string(), None => continue };
            if !matches_base_name(&file_name, base_name.as_str()) { continue; }
            let suffix = &file_name[base_name.len()..];
            let new_name = format!("{}{}", new_base_name, suffix);
            let new_path = dir.join(&new_name);
            if is_sequence && path.is_dir() {
                if let Ok(frames) = fs::read_dir(&path) {
                    for frame_entry in frames.flatten() {
                        let fpath = frame_entry.path();
                        let fname = match fpath.file_name().and_then(|n| n.to_str()) { Some(n) => n.to_string(), None => continue };
                        // 序列帧帧文件命名为 {base_name}_{帧编号}.png（下划线分隔），
                        // matches_base_name 只认连字符后缀，这里单独放宽：允许 '_' 或 '-' 分隔帧号
                        let fstem = Path::new(&fname).file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        let is_frame = fstem == base_name.as_str()
                            || fstem.starts_with(&format!("{}_", base_name))
                            || fstem.starts_with(&format!("{}-", base_name));
                        if is_frame {
                            let fsuffix = &fname[base_name.len()..];
                            let new_fname = format!("{}{}", new_base_name, fsuffix);
                            let _ = fs::rename(&fpath, fpath.parent().expect("read_dir frame must have parent").join(&new_fname));
                        }
                    }
                }
                fs::rename(&path, &new_path).map_err(|e| format!("重命名目录 {} 失败: {}", file_name, e))?;
            } else if !path.is_dir() {
                fs::rename(&path, &new_path).map_err(|e| format!("重命名文件 {} 失败: {}", file_name, e))?;
            }
        }
    }
    Ok(())
}

/// 删除素材的所有工作流版本（包括 nextcloud）
#[tauri::command]
pub fn delete_material(task_path: String, base_name: String, material_type: String) -> Result<(), String> {
    let task_dir = Path::new(&task_path);
    let is_sequence = material_type == "sequence";
    let mut dirs_to_scan: Vec<std::path::PathBuf> = vec![task_dir.join("00_original")];
    let scale_dir = task_dir.join("01_scale");
    if scale_dir.exists() { if let Ok(entries) = fs::read_dir(&scale_dir) { for e in entries.flatten() { if e.path().is_dir() { dirs_to_scan.push(e.path()); } } } }
    let done_dir = task_dir.join("02_done");
    if done_dir.exists() { if let Ok(entries) = fs::read_dir(&done_dir) { for e in entries.flatten() { if e.path().is_dir() { dirs_to_scan.push(e.path()); } } } }
    let nc_dir = task_dir.parent().and_then(|p| p.parent()).map(|vfx| vfx.join("nextcloud").join(task_dir.file_name().unwrap_or_default()));
    if let Some(ref nc) = nc_dir { if nc.exists() { dirs_to_scan.push(nc.clone()); } }

    for dir in &dirs_to_scan {
        if !dir.exists() { continue; }
        let entries = match fs::read_dir(dir) { Ok(e) => e, Err(_) => continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|n| n.to_str()) { Some(n) => n.to_string(), None => continue };
            if !matches_base_name(&file_name, base_name.as_str()) { continue; }
            if is_sequence && path.is_dir() {
                fs::remove_dir_all(&path).map_err(|e| format!("删除目录 {} 失败: {}", file_name, e))?;
            } else if !path.is_dir() {
                fs::remove_file(&path).map_err(|e| format!("删除文件 {} 失败: {}", file_name, e))?;
            }
        }
    }
    Ok(())
}

/// 设置项目的默认 AE 工程文件
#[tauri::command]
pub fn set_default_ae_file(project_path: String, file_name: Option<String>) -> Result<(), String> {
    let config_path = Path::new(&project_path).join(".pgb1_project.json");
    let content = fs::read_to_string(&config_path).map_err(|e| format!("读取配置文件失败: {}", e))?;
    let mut config: ProjectConfig = serde_json::from_str(&content).map_err(|e| format!("解析配置文件失败: {}", e))?;
    config.default_ae_file = file_name;
    let json = serde_json::to_string_pretty(&config).map_err(|e| format!("序列化配置失败: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("写入配置文件失败: {}", e))?;
    Ok(())
}

/// 递归扫描目录，寻找游戏原型启动程序
#[tauri::command]
pub fn find_game_exe(root_dir: String) -> Result<Option<String>, String> {
    const UNITY_CRASH_HANDLER: &str = "unitycrashhandler64.exe";
    fn walk(dir: &Path) -> Option<std::path::PathBuf> {
        let entries: Vec<_> = match std::fs::read_dir(dir) { Ok(rd) => rd.filter_map(|e| e.ok()).collect(), Err(_) => return None };
        let names_lower: Vec<(String, &std::fs::DirEntry)> = entries.iter().map(|e| (e.file_name().to_string_lossy().to_lowercase(), e)).collect();
        let has_unity = names_lower.iter().any(|(n, _)| n == UNITY_CRASH_HANDLER);
        if has_unity { for (name, entry) in &names_lower { if name.ends_with(".exe") && name != UNITY_CRASH_HANDLER { return Some(entry.path()); } } }
        for (name, _) in &names_lower {
            if name.ends_with(".pck") {
                let stem = &name[..name.len() - 4];
                let target_exe = format!("{}.exe", stem);
                if let Some((_, exe_entry)) = names_lower.iter().find(|(n, _)| *n == target_exe) { return Some(exe_entry.path()); }
            }
        }
        for e in &entries { if let Ok(ft) = e.file_type() { if ft.is_dir() { if let Some(found) = walk(&e.path()) { return Some(found); } } } }
        None
    }
    let root = Path::new(&root_dir);
    if !root.exists() { return Ok(None); }
    Ok(walk(root).map(|p| p.to_string_lossy().to_string()))
}

/// 修改序列帧的帧率
#[tauri::command]
pub fn rename_sequence_fps(task_path: String, base_name: String, old_fps: u32, new_fps: u32) -> Result<(), String> {
    if old_fps == new_fps { return Ok(()); }
    let done_dir = Path::new(&task_path).join("02_done");
    if !done_dir.exists() { return Ok(()); }
    let old_suffix = format!("-{}]", old_fps);
    let entries = fs::read_dir(&done_dir).map_err(|e| format!("读取 02_done 失败: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }
        let dir_name = match path.file_name().and_then(|n| n.to_str()) { Some(n) => n.to_string(), None => continue };
        if !dir_name.starts_with("[an-") || !dir_name.ends_with(old_suffix.as_str()) { continue; }
        let has_match = fs::read_dir(&path).map(|rd| rd.flatten().any(|e| e.file_name().to_str().map(|n| matches_base_name(n, base_name.as_str())).unwrap_or(false))).unwrap_or(false);
        if !has_match { continue; }
        let prefix = &dir_name[..dir_name.len() - old_suffix.len()];
        let new_dir_name = format!("{}-{}]", prefix, new_fps);
        let new_path = done_dir.join(&new_dir_name);
        if new_path.exists() { return Err(format!("目标目录已存在: {}", new_dir_name)); }
        fs::rename(&path, &new_path).map_err(|e| format!("重命名 {} -> {} 失败: {}", dir_name, new_dir_name, e))?;
    }
    Ok(())
}

/// 重命名单个文件
#[tauri::command]
pub fn rename_file(path: String, new_name: String) -> Result<(), String> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() { return Err("文件名不能为空".to_string()); }
    const ILLEGAL_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    if trimmed.chars().any(|c| ILLEGAL_CHARS.contains(&c)) { return Err("文件名包含非法字符".to_string()); }
    let file_path = Path::new(&path);
    if !file_path.exists() { return Err(format!("文件不存在: {}", path)); }
    let parent = file_path.parent().ok_or_else(|| "无法获取父目录".to_string())?;
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let new_file_name = if ext.is_empty() { trimmed.to_string() } else { format!("{}.{}", trimmed, ext) };
    let new_path = parent.join(&new_file_name);
    if new_path.exists() { return Err(format!("文件名「{}」已存在", new_file_name)); }
    fs::rename(file_path, &new_path).map_err(|e| format!("重命名失败: {}", e))
}

/// 将单个文件移入 Windows 回收站
#[tauri::command]
pub fn delete_file(path: String) -> Result<(), String> {
    use windows::Win32::UI::Shell::{SHFileOperationW, SHFILEOPSTRUCTW, FO_DELETE};
    use windows::Win32::Foundation::HWND;
    use windows::core::PCWSTR;
    let file_path = Path::new(&path);
    if !file_path.exists() { return Err(format!("文件不存在: {}", path)); }
    let mut wide: Vec<u16> = path.encode_utf16().collect();
    wide.push(0); wide.push(0);
    let mut op = SHFILEOPSTRUCTW { hwnd: HWND(std::ptr::null_mut()), wFunc: FO_DELETE, pFrom: PCWSTR(wide.as_ptr()), pTo: PCWSTR::null(), fFlags: 0x0040, fAnyOperationsAborted: windows::Win32::Foundation::BOOL(0), hNameMappings: std::ptr::null_mut(), lpszProgressTitle: PCWSTR::null() };
    let result = unsafe { SHFileOperationW(&mut op) };
    if result != 0 { return Err(format!("移入回收站失败，错误码: {}", result)); }
    if op.fAnyOperationsAborted.as_bool() { return Err("操作被用户取消".to_string()); }
    Ok(())
}
