use crate::models::{
    ApplyTaskResult, ArchivedVersion, GlobalTaskConfig,
    ProjectConfig, ProjectInfo,
};
use super::helpers::{
    default_global_tasks, find_app_icon,
    load_or_create_config, move_dir, scan_task_names, to_title_case,
    PROTOTYPE_SUBCATEGORIES, PSD_SUBCATEGORIES,
};
use std::fs;
use std::path::{Path, PathBuf};

#[tauri::command]
pub fn load_global_tasks(root_dir: String) -> Result<GlobalTaskConfig, String> {
    let root = Path::new(&root_dir);
    let config_path = root.join(".pgb1_global_tasks.json");

    if config_path.exists() {
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("读取全局任务清单失败: {}", e))?;
        let config: GlobalTaskConfig = serde_json::from_str(&content)
            .map_err(|e| format!("解析全局任务清单失败: {}", e))?;
        return Ok(config);
    }

    // 创建默认模板
    let config = default_global_tasks();
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化全局任务清单失败: {}", e))?;
    fs::write(&config_path, json)
        .map_err(|e| format!("写入全局任务清单失败: {}", e))?;

    Ok(config)
}

/// 保存全局任务清单
#[tauri::command]
pub fn save_global_tasks(root_dir: String, config: GlobalTaskConfig) -> Result<(), String> {
    let root = Path::new(&root_dir);
    let config_path = root.join(".pgb1_global_tasks.json");

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化全局任务清单失败: {}", e))?;
    fs::write(&config_path, json)
        .map_err(|e| format!("写入全局任务清单失败: {}", e))?;

    Ok(())
}

/// 应用任务变更：创建新任务文件夹 / 归档移除的任务文件夹
#[tauri::command]
pub fn apply_task_changes(
    project_path: String,
    enabled_tasks: Vec<String>,
) -> Result<ApplyTaskResult, String> {
    let project_dir = Path::new(&project_path);
    let config_path = project_dir.join(".pgb1_project.json");

    // 读取当前配置
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取项目配置失败: {}", e))?;
    let mut config: ProjectConfig = serde_json::from_str(&content)
        .map_err(|e| format!("解析项目配置失败: {}", e))?;

    let old_set: std::collections::HashSet<&str> =
        config.enabled_tasks.iter().map(|s| s.as_str()).collect();
    let new_set: std::collections::HashSet<&str> =
        enabled_tasks.iter().map(|s| s.as_str()).collect();

    // 只处理父任务（不含 / 的），子任务 key 仅存配置不操作文件夹
    let to_create: Vec<&str> = new_set.difference(&old_set).copied().filter(|s| !s.contains('/')).collect();
    let to_archive: Vec<&str> = old_set.difference(&new_set).copied().filter(|s| !s.contains('/')).collect();

    let vfx_dir = project_dir
        .join("03_Render_VFX")
        .join("VFX");
    let export_dir = vfx_dir.join("Export");
    let nextcloud_dir = vfx_dir.join("nextcloud");

    let mut created = Vec::new();
    let mut archived = Vec::new();
    let mut errors = Vec::new();

    // 创建新任务文件夹
    for task_name in &to_create {
        let folder_name = to_title_case(task_name);
        let is_prototype = task_name.to_lowercase() == "prototype";

        // Export/{Name}/00_original/, 01_scale/, 02_done/, 03_preview/
        let task_export = export_dir.join(&folder_name);
        let subdirs = ["00_original", "01_scale", "02_done", "03_preview"];

        for sub in &subdirs {
            let sub_path = task_export.join(sub);
            if is_prototype && (*sub == "00_original" || *sub == "02_done") {
                // Prototype: 00_original/02_done 下创建 7 个子分类；01_scale 只建空目录（缩放时按需创建 [XX]/subcat/）
                for cat in &PROTOTYPE_SUBCATEGORIES {
                    if let Err(e) = fs::create_dir_all(sub_path.join(cat)) {
                        errors.push(format!(
                            "创建 Export/{}/{}/{} 失败: {}",
                            folder_name, sub, cat, e
                        ));
                    }
                }
            } else if let Err(e) = fs::create_dir_all(&sub_path) {
                errors.push(format!(
                    "创建 Export/{}/{} 失败: {}",
                    folder_name, sub, e
                ));
            }
        }

        // nextcloud/{Name}/
        let task_nc = nextcloud_dir.join(&folder_name);
        if is_prototype {
            for cat in &PROTOTYPE_SUBCATEGORIES {
                if let Err(e) = fs::create_dir_all(task_nc.join(cat)) {
                    errors.push(format!(
                        "创建 nextcloud/{}/{} 失败: {}",
                        folder_name, cat, e
                    ));
                }
            }
        } else if let Err(e) = fs::create_dir_all(&task_nc) {
            errors.push(format!("创建 nextcloud/{} 失败: {}", folder_name, e));
        }

        created.push(folder_name);
    }

    // 归档移除的任务文件夹
    for task_name in &to_archive {
        let folder_name = to_title_case(task_name);
        let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M").to_string();
        let archive_base = project_dir
            .join(".archived_tasks")
            .join(&folder_name)
            .join(format!("timestamp_{}", timestamp));

        // 归档 Export/{Name}/
        let src_export = export_dir.join(&folder_name);
        if src_export.exists() {
            let dest = archive_base.join("Export").join(&folder_name);
            if let Err(e) = move_dir(&src_export, &dest) {
                errors.push(format!("归档 Export/{} 失败: {}", folder_name, e));
            }
        }

        // 归档 nextcloud/{Name}/
        let src_nc = nextcloud_dir.join(&folder_name);
        if src_nc.exists() {
            let dest = archive_base.join("nextcloud").join(&folder_name);
            if let Err(e) = move_dir(&src_nc, &dest) {
                errors.push(format!("归档 nextcloud/{} 失败: {}", folder_name, e));
            }
        }

        archived.push(folder_name);
    }

    // 更新配置
    config.enabled_tasks = enabled_tasks;
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化项目配置失败: {}", e))?;
    fs::write(&config_path, json)
        .map_err(|e| format!("写入项目配置失败: {}", e))?;

    Ok(ApplyTaskResult {
        created,
        archived,
        errors,
    })
}

/// 移动目录（先复制再删除原目录）
// ─── 时光机（归档恢复） ─────────────────────────────────────────

/// 列出所有归档版本（同时清理超过 60 天的过期归档）
#[tauri::command]
pub fn list_archived_tasks(project_path: String) -> Result<Vec<ArchivedVersion>, String> {
    let archive_root = Path::new(&project_path).join(".archived_tasks");
    if !archive_root.exists() {
        return Ok(Vec::new());
    }

    let now = chrono::Local::now();
    let cutoff = now - chrono::Duration::days(60);
    let mut versions = Vec::new();

    // 遍历 .archived_tasks/{TaskName}/
    let task_dirs =
        fs::read_dir(&archive_root).map_err(|e| format!("无法读取归档目录: {}", e))?;

    for task_entry in task_dirs.flatten() {
        let task_path = task_entry.path();
        if !task_path.is_dir() {
            continue;
        }
        let task_name = task_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if task_name.starts_with('.') {
            continue;
        }

        // 遍历 timestamp_{YYYY-MM-DD_HH-MM}/
        let ts_dirs = match fs::read_dir(&task_path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for ts_entry in ts_dirs.flatten() {
            let ts_path = ts_entry.path();
            if !ts_path.is_dir() {
                continue;
            }
            let dir_name = ts_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // 解析 "timestamp_YYYY-MM-DD_HH-MM"
            let timestamp = match dir_name.strip_prefix("timestamp_") {
                Some(ts) => ts.to_string(),
                None => continue,
            };

            // 60 天清理
            if let Ok(parsed) =
                chrono::NaiveDateTime::parse_from_str(&timestamp, "%Y-%m-%d_%H-%M")
            {
                let local_time = parsed
                    .and_local_timezone(chrono::Local)
                    .single()
                    .unwrap_or_else(|| {
                        chrono::Local::now() // 回退：不删除
                    });
                if local_time < cutoff {
                    let _ = fs::remove_dir_all(&ts_path);
                    continue;
                }
            }

            // timestamp = "2026-02-13_14-30" → display_time = "2026-02-13 14:30"
            let display_time = format!(
                "{} {}",
                &timestamp[..10],                          // "2026-02-13"
                &timestamp[11..].replace('-', ":")         // "14:30"
            );

            versions.push(ArchivedVersion {
                task_name: task_name.clone(),
                timestamp,
                display_time,
                path: ts_path.to_string_lossy().to_string(),
            });
        }

        // 如果任务名目录已空，清理
        if fs::read_dir(&task_path)
            .map(|mut d| d.next().is_none())
            .unwrap_or(false)
        {
            let _ = fs::remove_dir(&task_path);
        }
    }

    // 按任务名升序，同名内按时间倒序
    versions.sort_by(|a, b| {
        a.task_name
            .cmp(&b.task_name)
            .then_with(|| b.timestamp.cmp(&a.timestamp))
    });

    Ok(versions)
}

/// 恢复归档任务
#[tauri::command]
pub fn restore_archived_task(
    project_path: String,
    task_name: String,
    timestamp: String,
) -> Result<(), String> {
    let project_dir = Path::new(&project_path);

    // 读取项目配置
    let config_path = project_dir.join(".pgb1_project.json");
    let content =
        fs::read_to_string(&config_path).map_err(|e| format!("读取项目配置失败: {}", e))?;
    let mut config: ProjectConfig =
        serde_json::from_str(&content).map_err(|e| format!("解析项目配置失败: {}", e))?;

    // 检查同名任务是否已启用（小写比较）
    let task_lower = task_name.to_lowercase();
    if config.enabled_tasks.iter().any(|t| t.to_lowercase() == task_lower) {
        return Err(format!(
            "任务「{}」已在启用列表中，请先在「任务启用」中关闭该任务再恢复",
            task_name
        ));
    }

    // 构建归档路径
    let archive_path = project_dir
        .join(".archived_tasks")
        .join(&task_name)
        .join(format!("timestamp_{}", timestamp));

    if !archive_path.exists() {
        return Err(format!("归档版本不存在: {}", archive_path.display()));
    }

    let vfx_dir = project_dir.join("03_Render_VFX").join("VFX");

    // 恢复 Export/{TaskName}/
    let archived_export = archive_path.join("Export").join(&task_name);
    if archived_export.exists() {
        let dest = vfx_dir.join("Export").join(&task_name);
        move_dir(&archived_export, &dest)?;
    }

    // 恢复 nextcloud/{TaskName}/
    let archived_nc = archive_path.join("nextcloud").join(&task_name);
    if archived_nc.exists() {
        let dest = vfx_dir.join("nextcloud").join(&task_name);
        move_dir(&archived_nc, &dest)?;
    }

    // 删除该归档版本目录
    let _ = fs::remove_dir_all(&archive_path);

    // 如果任务名目录已空，清理
    let task_archive_dir = project_dir.join(".archived_tasks").join(&task_name);
    if fs::read_dir(&task_archive_dir)
        .map(|mut d| d.next().is_none())
        .unwrap_or(false)
    {
        let _ = fs::remove_dir(&task_archive_dir);
    }

    // 更新 enabled_tasks
    config.enabled_tasks.push(task_lower);
    let json =
        serde_json::to_string_pretty(&config).map_err(|e| format!("序列化配置失败: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("写入配置失败: {}", e))?;

    Ok(())
}

/// 删除指定的归档版本
#[tauri::command]
pub fn delete_archived_version(
    project_path: String,
    task_name: String,
    timestamp: String,
) -> Result<(), String> {
    let project_dir = Path::new(&project_path);
    let archive_path = project_dir
        .join(".archived_tasks")
        .join(&task_name)
        .join(format!("timestamp_{}", timestamp));

    if !archive_path.exists() {
        return Err(format!("归档版本不存在: {}", archive_path.display()));
    }

    fs::remove_dir_all(&archive_path).map_err(|e| format!("删除归档版本失败: {}", e))?;

    // 如果任务名目录已空，清理
    let task_archive_dir = project_dir.join(".archived_tasks").join(&task_name);
    if fs::read_dir(&task_archive_dir)
        .map(|mut d| d.next().is_none())
        .unwrap_or(false)
    {
        let _ = fs::remove_dir(&task_archive_dir);
    }

    Ok(())
}

/// 新建项目：创建标准 6 目录骨架 + 配置文件
#[tauri::command]
pub fn create_project(
    root_dir: String,
    project_name: String,
    deadline: Option<String>,
) -> Result<ProjectInfo, String> {
    // 校验项目名不为空
    let trimmed_name = project_name.trim();
    if trimmed_name.is_empty() {
        return Err("项目名称不能为空".to_string());
    }

    // 校验项目名不含非法字符（Windows 文件名限制）
    const ILLEGAL_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    if trimmed_name.chars().any(|c| ILLEGAL_CHARS.contains(&c)) {
        return Err(format!(
            "项目名称包含非法字符，不能使用: {}",
            ILLEGAL_CHARS.iter().collect::<String>()
        ));
    }

    let root = Path::new(&root_dir);
    if !root.exists() {
        return Err(format!("项目根目录不存在: {}", root_dir));
    }

    let project_dir = root.join(trimmed_name);

    // 校验同名项目不存在
    if project_dir.exists() {
        return Err(format!("同名项目已存在: {}", trimmed_name));
    }

    // 创建标准目录骨架
    let vfx_base = project_dir.join("03_Render_VFX").join("VFX");
    let mut dirs_to_create: Vec<PathBuf> = vec![
        project_dir.join("00_Game Design & Doc"),
        project_dir.join("01_Preproduction"),
        project_dir.join("02_Production"),
        vfx_base.join("Export"),
        vfx_base.join("nextcloud"),
        vfx_base.join("nextcloud").join("preview"),
        vfx_base.join("nextcloud").join("preview").join("breakdown"),
        vfx_base.join("AE"),
        project_dir.join("04_Trailer"),
        project_dir.join("05_Outside"),
    ];
    // PSD/ 下 8 个固定子目录（与任务列表无关）
    let psd_base = vfx_base.join("PSD");
    for cat in &PSD_SUBCATEGORIES {
        dirs_to_create.push(psd_base.join(cat));
    }

    for dir in &dirs_to_create {
        fs::create_dir_all(dir).map_err(|e| format!("创建目录失败 {}: {}", dir.display(), e))?;
    }

    // 写入配置文件
    let config = ProjectConfig {
        project_name: trimmed_name.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        imported: false,
        deadline: deadline.clone(),
        enabled_tasks: Vec::new(),
        archived_tasks: Vec::new(),
        completed_subtasks: Vec::new(),
        upload_prompted_tasks: Vec::new(),
        default_ae_file: None,
        priority: None,
        task_priorities: std::collections::HashMap::new(),
    };

    let config_path = project_dir.join(".pgb1_project.json");
    let json =
        serde_json::to_string_pretty(&config).map_err(|e| format!("序列化配置失败: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("写入配置文件失败: {}", e))?;

    // 返回 ProjectInfo
    Ok(ProjectInfo {
        name: trimmed_name.to_string(),
        path: project_dir.to_string_lossy().to_string(),
        deadline,
        tasks: Vec::new(),
        task_count: 0,
        enabled_tasks: Vec::new(),
        completed_subtasks: Vec::new(),
        upload_prompted_tasks: Vec::new(),
        completed_tasks: Vec::new(),
        default_ae_file: None,
        app_icon: None,
        priority: None,
        note: None,
    })
}

/// 切换子任务完成状态
#[tauri::command]
pub fn toggle_subtask_completion(
    project_path: String,
    subtask_key: String,
) -> Result<Vec<String>, String> {
    let project_dir = Path::new(&project_path);
    let config_path = project_dir.join(".pgb1_project.json");

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取项目配置失败: {}", e))?;
    let mut config: ProjectConfig = serde_json::from_str(&content)
        .map_err(|e| format!("解析项目配置失败: {}", e))?;

    // 切换：有则移除，无则添加
    if let Some(pos) = config.completed_subtasks.iter().position(|s| s == &subtask_key) {
        config.completed_subtasks.remove(pos);
    } else {
        config.completed_subtasks.push(subtask_key);
    }

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化项目配置失败: {}", e))?;
    fs::write(&config_path, json)
        .map_err(|e| format!("写入项目配置失败: {}", e))?;

    Ok(config.completed_subtasks)
}

/// 标记/取消任务的上传提醒状态
#[tauri::command]
pub fn mark_upload_prompted(
    project_path: String,
    task_name: String,
    prompted: bool,
) -> Result<(), String> {
    let project_dir = Path::new(&project_path);
    let config_path = project_dir.join(".pgb1_project.json");

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取项目配置失败: {}", e))?;
    let mut config: ProjectConfig = serde_json::from_str(&content)
        .map_err(|e| format!("解析项目配置失败: {}", e))?;

    let key = task_name.to_lowercase();
    if prompted {
        if !config.upload_prompted_tasks.contains(&key) {
            config.upload_prompted_tasks.push(key);
        }
    } else {
        config.upload_prompted_tasks.retain(|s| s != &key);
    }

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化项目配置失败: {}", e))?;
    fs::write(&config_path, json)
        .map_err(|e| format!("写入项目配置失败: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn update_project_deadline(
    project_path: String,
    deadline: Option<String>,
) -> Result<(), String> {
    let config_path = Path::new(&project_path).join(".pgb1_project.json");
    let raw = fs::read_to_string(&config_path).map_err(|e| format!("读取配置失败: {}", e))?;
    let mut config: ProjectConfig = serde_json::from_str(&raw).map_err(|e| format!("解析配置失败: {}", e))?;
    config.deadline = deadline;
    let json = serde_json::to_string_pretty(&config).map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("写入配置失败: {}", e))?;
    Ok(())
}

/// 将项目目录移入回收站（Windows Shell API）
#[tauri::command]
pub fn delete_project(project_path: String) -> Result<(), String> {
    use windows::Win32::UI::Shell::{SHFileOperationW, SHFILEOPSTRUCTW, FO_DELETE};
    use windows::Win32::Foundation::HWND;
    use windows::core::PCWSTR;

    let path = Path::new(&project_path);
    if !path.exists() {
        return Err(format!("项目目录不存在: {}", project_path));
    }
    // 安全检查：必须包含 .pgb1_project.json，防止误操作非项目目录
    if !path.join(".pgb1_project.json").exists() {
        return Err("目标目录不是有效的 PGB1 项目（缺少 .pgb1_project.json）".to_string());
    }

    // SHFileOperationW 要求路径以双 null 结尾的宽字符串
    let mut wide: Vec<u16> = project_path.encode_utf16().collect();
    wide.push(0); // 第一个 null
    wide.push(0); // 双 null 结尾

    let mut op = SHFILEOPSTRUCTW {
        hwnd: HWND(std::ptr::null_mut()),
        wFunc: FO_DELETE,
        pFrom: PCWSTR(wide.as_ptr()),
        pTo: PCWSTR::null(),
        fFlags: 0x0040, // FOF_ALLOWUNDO — 移入回收站而非永久删除
        fAnyOperationsAborted: windows::Win32::Foundation::BOOL(0),
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: PCWSTR::null(),
    };

    let result = unsafe { SHFileOperationW(&mut op) };
    if result != 0 {
        return Err(format!("移入回收站失败，错误码: {}", result));
    }
    if op.fAnyOperationsAborted.as_bool() {
        return Err("操作被用户取消".to_string());
    }
    Ok(())
}

/// 重命名项目（改目录名 + 更新 config 中的 project_name）
#[tauri::command]
pub fn rename_project(project_path: String, new_name: String) -> Result<ProjectInfo, String> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err("项目名称不能为空".to_string());
    }

    // 校验非法字符（与 create_project 一致）
    const ILLEGAL_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    if trimmed.chars().any(|c| ILLEGAL_CHARS.contains(&c)) {
        return Err(format!(
            "项目名称包含非法字符，不能使用: {}",
            ILLEGAL_CHARS.iter().collect::<String>()
        ));
    }

    let old_path = Path::new(&project_path);
    if !old_path.exists() {
        return Err(format!("项目目录不存在: {}", project_path));
    }

    let parent = old_path
        .parent()
        .ok_or("无法获取父目录")?;
    let new_path = parent.join(trimmed);

    if new_path.exists() {
        return Err(format!("同名项目已存在: {}", trimmed));
    }

    // 重命名目录
    fs::rename(old_path, &new_path)
        .map_err(|e| format!("重命名目录失败: {}", e))?;

    // 更新 .pgb1_project.json 中的 project_name
    let config_path = new_path.join(".pgb1_project.json");
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置文件失败: {}", e))?;
    let mut config: ProjectConfig = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置失败: {}", e))?;
    config.project_name = trimmed.to_string();
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化配置失败: {}", e))?;
    fs::write(&config_path, json)
        .map_err(|e| format!("写入配置失败: {}", e))?;

    // 返回新的 ProjectInfo（重新扫描单个项目）
    let config = load_or_create_config(&new_path)?;
    let export_path = new_path.join("03_Render_VFX").join("VFX").join("Export");
    let tasks = if export_path.exists() {
        scan_task_names(&export_path)?
    } else {
        Vec::new()
    };
    let task_count = tasks.len();
    let app_icon = find_app_icon(&new_path.join("01_Preproduction"));

    Ok(ProjectInfo {
        name: trimmed.to_string(),
        path: new_path.to_string_lossy().to_string(),
        deadline: config.deadline,
        tasks,
        task_count,
        enabled_tasks: config.enabled_tasks,
        completed_subtasks: config.completed_subtasks,
        upload_prompted_tasks: config.upload_prompted_tasks,
        completed_tasks: Vec::new(),
        default_ae_file: config.default_ae_file,
        app_icon,
        priority: config.priority,
        note: None,
    })
}

/// 重命名单个文件（保留扩展名，仅改基础名）
#[tauri::command]
pub fn set_project_priority(project_path: String, priority: Option<String>) -> Result<(), String> {
    let config_path = std::path::Path::new(&project_path).join(".pgb1_project.json");
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let mut config: crate::models::ProjectConfig = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置失败: {}", e))?;
    config.priority = priority;
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}

/// 设置任务优先度（"high"/"medium"/"low" 或 null 清除）
#[tauri::command]
pub fn set_task_priority(project_path: String, task_name: String, priority: Option<String>) -> Result<(), String> {
    let config_path = std::path::Path::new(&project_path).join(".pgb1_project.json");
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let mut config: crate::models::ProjectConfig = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置失败: {}", e))?;
    let key = task_name.to_lowercase();
    match priority {
        Some(p) => { config.task_priorities.insert(key, p); }
        None    => { config.task_priorities.remove(&key); }
    }
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}
