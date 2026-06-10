use super::helpers::{matches_base_name, material_type_from_ext, move_dir, mutate_project_config, validate_file_name};
use super::workflow_paths::{
    nextcloud_task_dir, stage_dir_prefix, vfx_dir,
    DIR_DONE, DIR_EXPORT, DIR_NC_ORIGINAL, DIR_NEXTCLOUD, DIR_ORIGINAL, DIR_SCALE,
    STAGE_PREFIX_ANIM,
};
use crate::models::{ArchivedMaterialVersion, MaterialType};
use std::fs;
use std::path::{Path, PathBuf};

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
    let mut dirs_to_scan: Vec<std::path::PathBuf> = vec![task_dir.join(DIR_ORIGINAL)];
    let scale_dir = task_dir.join(DIR_SCALE);
    if scale_dir.exists() { if let Ok(entries) = fs::read_dir(&scale_dir) { for e in entries.flatten() { if e.path().is_dir() { dirs_to_scan.push(e.path()); } } } }
    let done_dir = task_dir.join(DIR_DONE);
    if done_dir.exists() { if let Ok(entries) = fs::read_dir(&done_dir) { for e in entries.flatten() { if e.path().is_dir() { dirs_to_scan.push(e.path()); } } } }
    let nc_dir = nextcloud_task_dir(task_dir);
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
                // 先收集所有需要改名的帧文件，检测目标冲突，然后原子性批量重命名
                // 任何一帧失败立即返回错误，防止目录改名后留下半成功的帧集
                let mut frame_renames: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
                let frames = fs::read_dir(&path)
                    .map_err(|e| format!("读取序列帧目录 {} 失败: {}", file_name, e))?;
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
                        let new_fpath = fpath.parent()
                            .ok_or_else(|| format!("帧文件 {} 无父目录", fname))?
                            .join(&new_fname);
                        if new_fpath.exists() && new_fpath != fpath {
                            return Err(format!("帧文件目标已存在: {}", new_fname));
                        }
                        frame_renames.push((fpath, new_fpath));
                    }
                }
                // 两阶段提交：任一帧 rename 失败则回滚已成功的前序帧，
                // 保证帧集与目录名的一致性（数据完整性 P0，对齐 R9-R）
                let mut committed: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
                for (src, dst) in &frame_renames {
                    if let Err(e) = fs::rename(src, dst) {
                        let fname = src.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                        // 回滚：逆序把已改名的帧恢复到原名
                        for (orig_src, done_dst) in committed.iter().rev() {
                            if let Err(re) = fs::rename(done_dst, orig_src) {
                                log::error!(
                                    "[rename_material] 回滚帧失败 {} -> {}: {}",
                                    done_dst.display(), orig_src.display(), re
                                );
                            }
                        }
                        return Err(format!("重命名帧文件 {} 失败（已回滚前序帧）: {}", fname, e));
                    }
                    committed.push((src.clone(), dst.clone()));
                }
                // 外层目录 rename 失败也需要回滚所有帧，否则目录名与帧名不一致
                if let Err(e) = fs::rename(&path, &new_path) {
                    for (orig_src, done_dst) in committed.iter().rev() {
                        if let Err(re) = fs::rename(done_dst, orig_src) {
                            log::error!(
                                "[rename_material] 目录回滚时帧恢复失败 {} -> {}: {}",
                                done_dst.display(), orig_src.display(), re
                            );
                        }
                    }
                    return Err(format!("重命名目录 {} 失败（已回滚所有帧）: {}", file_name, e));
                }
            } else if !path.is_dir() {
                fs::rename(&path, &new_path).map_err(|e| format!("重命名文件 {} 失败: {}", file_name, e))?;
            }
        }
    }
    Ok(())
}

/// 归档素材的工作流版本到 `.archived_materials/<Task>/<BaseName>/timestamp_<TS>/`（move 保留子目录结构），
/// nextcloud 副本（含 original/ 子目录）直接删除（仅本地上传标记，非云端本体，不进归档）。
///   - `include_original = true`：连 `00_original` 一起归档 → 删除素材
///   - `include_original = false`：保留 `00_original`，只清派生版本（01_scale/02_done/nextcloud）→ 「更新」重做
fn archive_material_internal(
    task_path: String,
    base_name: String,
    material_type: String,
    include_original: bool,
) -> Result<(), String> {
    let task_dir = Path::new(&task_path);
    let is_sequence = material_type == "sequence";

    // 定位项目根：task_path = <project>/03_Render_VFX/VFX/Export/<TaskName>/
    let vfx_root = task_dir
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| "无法定位 VFX 目录".to_string())?;
    let project_dir = vfx_root
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| "无法定位项目根目录".to_string())?;
    let task_name = task_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "无法获取任务名".to_string())?
        .to_string();

    // 归档目录
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M").to_string();
    let archive_base = project_dir
        .join(".archived_materials")
        .join(&task_name)
        .join(&base_name)
        .join(format!("timestamp_{}", timestamp));

    // 收集归档源：(stage_label, src_dir)。include_original 决定是否归档 00_original。
    let mut archive_sources: Vec<(String, PathBuf)> = Vec::new();
    if include_original {
        archive_sources.push((DIR_ORIGINAL.to_string(), task_dir.join(DIR_ORIGINAL)));
    }
    let scale_dir = task_dir.join(DIR_SCALE);
    if scale_dir.exists() {
        if let Ok(entries) = fs::read_dir(&scale_dir) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    let sub = e.file_name().to_string_lossy().to_string();
                    archive_sources.push((format!("{}/{}", DIR_SCALE, sub), e.path()));
                }
            }
        }
    }
    let done_dir = task_dir.join(DIR_DONE);
    if done_dir.exists() {
        if let Ok(entries) = fs::read_dir(&done_dir) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    let sub = e.file_name().to_string_lossy().to_string();
                    archive_sources.push((format!("{}/{}", DIR_DONE, sub), e.path()));
                }
            }
        }
    }

    // 归档（move 到 .archived_materials）
    for (stage, src_dir) in &archive_sources {
        if !src_dir.exists() { continue; }
        let entries = match fs::read_dir(src_dir) { Ok(e) => e, Err(_) => continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|n| n.to_str()) { Some(n) => n.to_string(), None => continue };
            if !matches_base_name(&file_name, base_name.as_str()) { continue; }
            let dest = archive_base.join(stage).join(&file_name);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("创建归档目录 {} 失败: {}", parent.display(), e))?;
            }
            if is_sequence && path.is_dir() {
                move_dir(&path, &dest)?;
            } else if !path.is_dir() {
                fs::rename(&path, &dest).map_err(|e| format!("归档文件 {} 失败: {}", file_name, e))?;
            }
        }
    }

    // nextcloud 副本：本地上传标记，直接删（决策对齐）
    let nc_dir = vfx_root.join(DIR_NEXTCLOUD).join(&task_name);
    if nc_dir.exists() {
        if let Ok(entries) = fs::read_dir(&nc_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = match path.file_name().and_then(|n| n.to_str()) { Some(n) => n.to_string(), None => continue };
                if !matches_base_name(&file_name, base_name.as_str()) { continue; }
                if path.is_dir() {
                    fs::remove_dir_all(&path).map_err(|e| format!("删除 nextcloud 目录 {} 失败: {}", file_name, e))?;
                } else {
                    fs::remove_file(&path).map_err(|e| format!("删除 nextcloud 文件 {} 失败: {}", file_name, e))?;
                }
            }
        }
        // original/ 子目录（原件直传副本）：删除匹配文件，避免遗留孤儿（方案 B 配套）
        let nc_original = nc_dir.join(DIR_NC_ORIGINAL);
        if nc_original.exists() {
            if let Ok(entries) = fs::read_dir(&nc_original) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let file_name = match path.file_name().and_then(|n| n.to_str()) { Some(n) => n.to_string(), None => continue };
                    if path.is_dir() || !matches_base_name(&file_name, base_name.as_str()) { continue; }
                    fs::remove_file(&path).map_err(|e| format!("删除 nextcloud/original 文件 {} 失败: {}", file_name, e))?;
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub fn delete_material(task_path: String, base_name: String, material_type: String) -> Result<(), String> {
    archive_material_internal(task_path, base_name, material_type, true)
}

/// 「更新」：清除素材的派生版本（01_scale + 02_done 归档时光机，nextcloud 标记直删），保留 00_original，
/// 便于替换原件后重新制作。制作参数（scale/帧率）由前端记入笔记。
#[tauri::command]
pub fn reset_material_versions(task_path: String, base_name: String, material_type: String) -> Result<(), String> {
    archive_material_internal(task_path, base_name, material_type, false)
}

/// 列出项目下所有素材归档版本（顺带清理超过 60 天的归档，对齐 `list_archived_tasks`）
#[tauri::command]
pub fn list_archived_materials(project_path: String) -> Result<Vec<ArchivedMaterialVersion>, String> {
    let archive_root = Path::new(&project_path).join(".archived_materials");
    if !archive_root.exists() {
        return Ok(Vec::new());
    }

    let now = chrono::Local::now();
    let cutoff = now - chrono::Duration::days(60);
    let mut versions: Vec<ArchivedMaterialVersion> = Vec::new();

    let task_dirs = fs::read_dir(&archive_root)
        .map_err(|e| format!("无法读取素材归档目录: {}", e))?;
    for task_entry in task_dirs.flatten() {
        let task_path = task_entry.path();
        if !task_path.is_dir() { continue; }
        let task_name = task_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if task_name.starts_with('.') { continue; }

        let base_dirs = match fs::read_dir(&task_path) { Ok(d) => d, Err(_) => continue };
        for base_entry in base_dirs.flatten() {
            let base_path = base_entry.path();
            if !base_path.is_dir() { continue; }
            let base_name = base_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();

            let ts_dirs = match fs::read_dir(&base_path) { Ok(d) => d, Err(_) => continue };
            for ts_entry in ts_dirs.flatten() {
                let ts_path = ts_entry.path();
                if !ts_path.is_dir() { continue; }
                let dir_name = ts_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                let timestamp = match dir_name.strip_prefix("timestamp_") { Some(s) => s.to_string(), None => continue };

                // 60 天懒 GC
                if let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(&timestamp, "%Y-%m-%d_%H-%M") {
                    let local_time = parsed
                        .and_local_timezone(chrono::Local)
                        .single()
                        .unwrap_or_else(chrono::Local::now);
                    if local_time < cutoff {
                        if let Err(e) = fs::remove_dir_all(&ts_path) {
                            log::warn!("[archive-gc-material] 清理过期素材归档失败 {}: {}", ts_path.display(), e);
                        }
                        continue;
                    }
                }

                let display_time = if timestamp.len() >= 16 {
                    format!("{} {}", &timestamp[..10], &timestamp[11..].replace('-', ":"))
                } else {
                    timestamp.clone()
                };

                let material_type = infer_archived_material_type(&ts_path);
                let (size_bytes, stages) = scan_archive_content(&ts_path);

                versions.push(ArchivedMaterialVersion {
                    task_name: task_name.clone(),
                    base_name: base_name.clone(),
                    material_type,
                    timestamp,
                    display_time,
                    path: ts_path.to_string_lossy().to_string(),
                    size_bytes,
                    stages,
                });
            }

            // base_name 目录空了就清掉
            if fs::read_dir(&base_path).map(|mut d| d.next().is_none()).unwrap_or(false) {
                if let Err(e) = fs::remove_dir(&base_path) {
                    log::warn!("[archive-gc-material] 清理空素材归档目录失败 {}: {}", base_path.display(), e);
                }
            }
        }

        // task_name 目录空了就清掉
        if fs::read_dir(&task_path).map(|mut d| d.next().is_none()).unwrap_or(false) {
            if let Err(e) = fs::remove_dir(&task_path) {
                log::warn!("[archive-gc-material] 清理空任务素材归档目录失败 {}: {}", task_path.display(), e);
            }
        }
    }

    // 按 task_name → base_name → timestamp 倒序
    versions.sort_by(|a, b| {
        a.task_name
            .cmp(&b.task_name)
            .then_with(|| a.base_name.cmp(&b.base_name))
            .then_with(|| b.timestamp.cmp(&a.timestamp))
    });

    Ok(versions)
}

/// 恢复素材归档版本（拒绝式冲突：目标位置有同名文件直接报错，让用户先删再恢复）
#[tauri::command]
pub fn restore_archived_material(
    project_path: String,
    task_name: String,
    base_name: String,
    timestamp: String,
) -> Result<(), String> {
    let project_dir = Path::new(&project_path);
    let archive_path = project_dir
        .join(".archived_materials")
        .join(&task_name)
        .join(&base_name)
        .join(format!("timestamp_{}", timestamp));

    if !archive_path.exists() {
        return Err(format!("归档版本不存在: {}", archive_path.display()));
    }

    let task_dir = vfx_dir(project_dir).join(DIR_EXPORT).join(&task_name);

    if !task_dir.exists() {
        return Err(format!(
            "任务目录不存在，请先在「任务归档」中恢复「{}」，再恢复素材",
            task_name
        ));
    }

    // 冲突预检：拒绝式
    let mut conflicts: Vec<String> = Vec::new();
    collect_restore_conflicts(&archive_path.join(DIR_ORIGINAL), &task_dir.join(DIR_ORIGINAL), DIR_ORIGINAL, &mut conflicts);

    let archived_scale = archive_path.join(DIR_SCALE);
    if archived_scale.exists() {
        if let Ok(entries) = fs::read_dir(&archived_scale) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    let sub = e.file_name().to_string_lossy().to_string();
                    collect_restore_conflicts(
                        &archived_scale.join(&sub),
                        &task_dir.join(DIR_SCALE).join(&sub),
                        &format!("{}/{}", DIR_SCALE, sub),
                        &mut conflicts,
                    );
                }
            }
        }
    }

    let archived_done = archive_path.join(DIR_DONE);
    if archived_done.exists() {
        if let Ok(entries) = fs::read_dir(&archived_done) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    let sub = e.file_name().to_string_lossy().to_string();
                    collect_restore_conflicts(
                        &archived_done.join(&sub),
                        &task_dir.join(DIR_DONE).join(&sub),
                        &format!("{}/{}", DIR_DONE, sub),
                        &mut conflicts,
                    );
                }
            }
        }
    }

    if !conflicts.is_empty() {
        return Err(format!(
            "恢复冲突：目标位置已存在同名文件，请先在素材列表中删除对应版本再恢复。\n冲突清单:\n{}",
            conflicts.join("\n")
        ));
    }

    // 执行恢复
    restore_stage_dir(&archive_path.join(DIR_ORIGINAL), &task_dir.join(DIR_ORIGINAL))?;

    if archived_scale.exists() {
        if let Ok(entries) = fs::read_dir(&archived_scale) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    let sub = e.file_name().to_string_lossy().to_string();
                    let dest = task_dir.join(DIR_SCALE).join(&sub);
                    restore_stage_dir(&archived_scale.join(&sub), &dest)?;
                }
            }
        }
    }
    if archived_done.exists() {
        if let Ok(entries) = fs::read_dir(&archived_done) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    let sub = e.file_name().to_string_lossy().to_string();
                    let dest = task_dir.join(DIR_DONE).join(&sub);
                    restore_stage_dir(&archived_done.join(&sub), &dest)?;
                }
            }
        }
    }

    // 清理已恢复的归档目录
    if let Err(e) = fs::remove_dir_all(&archive_path) {
        log::warn!("[restore-archive-material] 清理已恢复归档失败 {}: {}", archive_path.display(), e);
    }
    let base_dir = project_dir.join(".archived_materials").join(&task_name).join(&base_name);
    if fs::read_dir(&base_dir).map(|mut d| d.next().is_none()).unwrap_or(false) {
        let _ = fs::remove_dir(&base_dir);
    }
    let task_archive_dir = project_dir.join(".archived_materials").join(&task_name);
    if fs::read_dir(&task_archive_dir).map(|mut d| d.next().is_none()).unwrap_or(false) {
        let _ = fs::remove_dir(&task_archive_dir);
    }

    Ok(())
}

/// 手动删除单个素材归档版本（物理删除，不可恢复）
#[tauri::command]
pub fn delete_archived_material_version(
    project_path: String,
    task_name: String,
    base_name: String,
    timestamp: String,
) -> Result<(), String> {
    let project_dir = Path::new(&project_path);
    let archive_path = project_dir
        .join(".archived_materials")
        .join(&task_name)
        .join(&base_name)
        .join(format!("timestamp_{}", timestamp));

    if !archive_path.exists() {
        return Err(format!("归档版本不存在: {}", archive_path.display()));
    }

    fs::remove_dir_all(&archive_path).map_err(|e| format!("删除归档版本失败: {}", e))?;

    let base_dir = project_dir.join(".archived_materials").join(&task_name).join(&base_name);
    if fs::read_dir(&base_dir).map(|mut d| d.next().is_none()).unwrap_or(false) {
        if let Err(e) = fs::remove_dir(&base_dir) {
            log::warn!("[delete-archive-material] 清理空素材归档目录失败 {}: {}", base_dir.display(), e);
        }
    }
    let task_archive_dir = project_dir.join(".archived_materials").join(&task_name);
    if fs::read_dir(&task_archive_dir).map(|mut d| d.next().is_none()).unwrap_or(false) {
        if let Err(e) = fs::remove_dir(&task_archive_dir) {
            log::warn!("[delete-archive-material] 清理空任务素材归档目录失败 {}: {}", task_archive_dir.display(), e);
        }
    }

    Ok(())
}

// ─── 内部 helpers（仅素材归档使用） ────────────────────────────

/// 从归档目录推断素材类型：看 `00_original` 下的首个条目
fn infer_archived_material_type(ts_path: &Path) -> String {
    let original = ts_path.join(DIR_ORIGINAL);
    if original.exists() {
        if let Ok(entries) = fs::read_dir(&original) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() { return "sequence".to_string(); }
                let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                return match material_type_from_ext(&ext) {
                    MaterialType::Video => "video".to_string(),
                    MaterialType::Image => "image".to_string(),
                    _ => "other".to_string(),
                };
            }
        }
    }
    // fallback：看 02_done 子目录命名
    let done = ts_path.join(DIR_DONE);
    if done.exists() {
        if let Ok(entries) = fs::read_dir(&done) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    let name = e.file_name().to_string_lossy().to_lowercase();
                    if name.starts_with(&stage_dir_prefix(STAGE_PREFIX_ANIM)) { return "sequence".to_string(); }
                    return "image".to_string();
                }
            }
        }
    }
    "other".to_string()
}

/// 扫描归档目录，返回 (总字节数, 阶段列表)
fn scan_archive_content(ts_path: &Path) -> (u64, Vec<String>) {
    let mut total_size: u64 = 0;
    let mut stages: Vec<String> = Vec::new();

    let original_path = ts_path.join(DIR_ORIGINAL);
    if original_path.exists() {
        if let Ok(entries) = fs::read_dir(&original_path) {
            let mut has_content = false;
            for e in entries.flatten() {
                has_content = true;
                total_size += compute_path_size(&e.path());
            }
            if has_content { stages.push(DIR_ORIGINAL.to_string()); }
        }
    }

    for parent_stage in [DIR_SCALE, DIR_DONE] {
        let stage_path = ts_path.join(parent_stage);
        if !stage_path.exists() { continue; }
        if let Ok(entries) = fs::read_dir(&stage_path) {
            for e in entries.flatten() {
                let sub_path = e.path();
                if !sub_path.is_dir() { continue; }
                let sub = e.file_name().to_string_lossy().to_string();
                if let Ok(inner) = fs::read_dir(&sub_path) {
                    let mut has = false;
                    for ie in inner.flatten() {
                        has = true;
                        total_size += compute_path_size(&ie.path());
                    }
                    if has { stages.push(format!("{}/{}", parent_stage, sub)); }
                }
            }
        }
    }

    (total_size, stages)
}

/// 递归计算文件/目录字节数
fn compute_path_size(p: &Path) -> u64 {
    if let Ok(meta) = fs::metadata(p) {
        if meta.is_file() {
            return meta.len();
        }
    }
    if p.is_dir() {
        let mut total: u64 = 0;
        if let Ok(entries) = fs::read_dir(p) {
            for e in entries.flatten() {
                total += compute_path_size(&e.path());
            }
        }
        return total;
    }
    0
}

/// 冲突预检：列出目标位置已存在的同名文件/目录，带阶段前缀
fn collect_restore_conflicts(archive_dir: &Path, target_dir: &Path, stage_label: &str, conflicts: &mut Vec<String>) {
    if !archive_dir.exists() { return; }
    let Ok(entries) = fs::read_dir(archive_dir) else { return; };
    for e in entries.flatten() {
        let name = e.file_name();
        let target = target_dir.join(&name);
        if target.exists() {
            conflicts.push(format!("{}/{}", stage_label, name.to_string_lossy()));
        }
    }
}

/// 把归档阶段目录的内容 move 回目标目录（目标不存在则创建）
fn restore_stage_dir(archive_dir: &Path, target_dir: &Path) -> Result<(), String> {
    if !archive_dir.exists() { return Ok(()); }
    fs::create_dir_all(target_dir).map_err(|e| format!("创建目标目录 {} 失败: {}", target_dir.display(), e))?;
    if let Ok(entries) = fs::read_dir(archive_dir) {
        for e in entries.flatten() {
            let src = e.path();
            let name = e.file_name();
            let dst = target_dir.join(&name);
            if src.is_dir() {
                move_dir(&src, &dst)?;
            } else {
                fs::rename(&src, &dst).map_err(|e| format!("恢复文件 {} 失败: {}", name.to_string_lossy(), e))?;
            }
        }
    }
    Ok(())
}

/// 设置项目的默认 AE 工程文件
#[tauri::command]
pub fn set_default_ae_file(project_path: String, file_name: Option<String>) -> Result<(), String> {
    mutate_project_config(Path::new(&project_path), |cfg| cfg.default_ae_file = file_name)
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
    let done_dir = Path::new(&task_path).join(DIR_DONE);
    if !done_dir.exists() { return Ok(()); }
    let old_suffix = format!("-{}]", old_fps);
    let an_prefix = stage_dir_prefix(STAGE_PREFIX_ANIM);
    let entries = fs::read_dir(&done_dir).map_err(|e| format!("读取 {} 失败: {}", DIR_DONE, e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }
        let dir_name = match path.file_name().and_then(|n| n.to_str()) { Some(n) => n.to_string(), None => continue };
        if !dir_name.starts_with(&an_prefix) || !dir_name.ends_with(old_suffix.as_str()) { continue; }
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
    // 统一走 validate_file_name：空 + 非法字符 + 控制字符 + 末尾点空格 + Windows 保留字
    let trimmed = validate_file_name(&new_name, "文件名")?;
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
