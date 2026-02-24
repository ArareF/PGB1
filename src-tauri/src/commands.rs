use crate::models::{
    AppSettings, ApplyTaskResult, ArchivedVersion, AttendanceConfig, AttendanceRecord,
    ConversionSequenceRequest, CopyMaterialRequest, CopyResult, DragMaterialRequest, FileEntry,
    GlobalTask, GlobalTaskChild, GlobalTaskConfig, ImportResult, MaterialInfo, MaterialProgress,
    MaterialType, MaterialVersion, PreviewVideoEntry, ProjectConfig, ProjectInfo, ScaleRequest,
    AppShortcut, Shortcut, ShortcutsConfig, StartConversionRequest, TaskInfo,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use crate::scheduler::SchedulerState;
use crate::conversion::{ConversionState, ConversionSession, handle_file_event, bring_window_to_front};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Runtime, State, Manager};
use notify::{Watcher, RecursiveMode, Event};

/// 扫描项目根目录，返回所有有效项目
#[tauri::command]
pub fn scan_projects(root_dir: String) -> Result<Vec<ProjectInfo>, String> {
    let root = Path::new(&root_dir);
    if !root.exists() {
        return Err(format!("项目根目录不存在: {}", root_dir));
    }

    let mut projects = Vec::new();

    let entries = fs::read_dir(root).map_err(|e| format!("无法读取目录: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();

        // 跳过非目录和隐藏目录
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                continue;
            }
        }

        // 检查是否为有效项目：必须存在 03_Render_VFX/VFX/Export/
        let export_path = path.join("03_Render_VFX").join("VFX").join("Export");
        if !export_path.exists() {
            continue;
        }

        // 读取或创建配置文件
        let config = load_or_create_config(&path)?;

        // 扫描 Export 下的任务列表
        let tasks = scan_task_names(&export_path)?;
        let task_count = tasks.len();

        let project_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let enabled_tasks = config.enabled_tasks.clone();
        let nextcloud_dir = path.join("03_Render_VFX").join("VFX").join("nextcloud");

        // 统计所有父任务中素材+视频全部上传的任务（含有子任务的父任务）
        let completed_tasks: Vec<String> = enabled_tasks.iter()
            .filter(|t| !t.contains('/'))
            .filter(|parent| {
                let original_dir = export_path.join(parent).join("00_original");
                let nc_task_dir = nextcloud_dir.join(parent);
                let is_prototype = parent.to_lowercase() == "prototype";
                let (total, uploaded) = count_upload_progress(&original_dir, &nc_task_dir, is_prototype);
                if total == 0 || uploaded < total { return false; }
                // 同时要求预览视频也全部上传
                let preview_dir = export_path.join(parent).join("03_preview");
                let nc_preview_dir = nextcloud_dir.join("preview");
                let (video_total, video_uploaded) = count_preview_progress(&preview_dir, &nc_preview_dir);
                video_total == 0 || video_uploaded >= video_total
            })
            .cloned()
            .collect();

        // 查找 01_Preproduction/ 下名字含 appicon 的文件（大小写不敏感）
        let app_icon = find_app_icon(&path.join("01_Preproduction"));

        projects.push(ProjectInfo {
            name: project_name,
            path: path.to_string_lossy().to_string(),
            deadline: config.deadline,
            default_ae_file: config.default_ae_file,
            app_icon,
            tasks,
            task_count,
            enabled_tasks,
            completed_subtasks: config.completed_subtasks,
            upload_prompted_tasks: config.upload_prompted_tasks,
            completed_tasks,
        });
    }

    // 按项目名排序
    projects.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(projects)
}

/// 扫描指定项目的任务列表
#[tauri::command]
pub fn scan_tasks(project_path: String) -> Result<Vec<TaskInfo>, String> {
    let project_dir = Path::new(&project_path);
    let vfx_dir = project_dir.join("03_Render_VFX").join("VFX");
    let export_path = vfx_dir.join("Export");
    let nextcloud_dir = vfx_dir.join("nextcloud");

    if !export_path.exists() {
        return Err(format!("Export 目录不存在: {}", export_path.display()));
    }

    let mut tasks = Vec::new();

    let entries =
        fs::read_dir(&export_path).map_err(|e| format!("无法读取 Export 目录: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();

        // 跳过非目录和隐藏项
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                continue;
            }
        }

        let task_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let has_subtasks = task_name.to_lowercase() == "prototype";

        // 统计素材上传进度
        let original_dir = path.join("00_original");
        let nc_task_dir = nextcloud_dir.join(&task_name);
        let is_prototype = task_name.to_lowercase() == "prototype";
        let (material_total, material_uploaded) = count_upload_progress(&original_dir, &nc_task_dir, is_prototype);

        // 统计预览视频上传进度
        let preview_dir = path.join("03_preview");
        let nc_preview_dir = nextcloud_dir.join("preview");
        let (video_total, video_uploaded) = count_preview_progress(&preview_dir, &nc_preview_dir);

        // 任务卡片大小：显示已上传到 nextcloud 的文件大小
        let size_bytes = calc_dir_size(&nc_task_dir);

        tasks.push(TaskInfo {
            name: task_name,
            path: path.to_string_lossy().to_string(),
            size_bytes,
            has_subtasks,
            material_total,
            material_uploaded,
            video_total,
            video_uploaded,
        });
    }

    // 按任务名排序
    tasks.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(tasks)
}

/// 统计素材上传进度：(总数, 已上传数)
/// 普通任务：总数 = 00_original 第一层文件/目录数
/// Prototype：总数 = 00_original 递归所有文件数（子分类下的素材）
fn count_upload_progress(original_dir: &Path, nc_dir: &Path, is_prototype: bool) -> (u32, u32) {
    if !original_dir.exists() {
        return (0, 0);
    }

    let original_names = if is_prototype {
        // Prototype: 00_original 下是子分类目录，递归收集所有素材文件名
        collect_base_names(original_dir)
    } else {
        // 普通任务: 第一层即素材
        collect_first_level_names(original_dir)
    };
    let total = original_names.len() as u32;
    if total == 0 || !nc_dir.exists() {
        return (total, 0);
    }

    // 收集 nextcloud 中所有文件的基础名（递归）
    let nc_names = collect_base_names(nc_dir);

    let uploaded = original_names.iter().filter(|name| nc_names.contains(*name)).count() as u32;
    (total, uploaded)
}

/// 统计预览视频上传进度：(总数, 已上传数)
/// 扫描 03_preview/ 中的视频文件，对比 nextcloud/preview/（及其 breakdown 子目录）
fn count_preview_progress(preview_dir: &Path, nc_preview_dir: &Path) -> (u32, u32) {
    if !preview_dir.exists() {
        return (0, 0);
    }

    let video_exts: &[&str] = &["mp4", "mov", "avi", "mkv", "webm", "flv"];

    // 收集 03_preview/ 第一层视频文件名（小写，含扩展名），并标记是否为 breakdown
    let mut video_names: Vec<(String, bool)> = Vec::new();
    if let Ok(entries) = fs::read_dir(preview_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() { continue; }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if !video_exts.contains(&ext.as_str()) { continue; }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let name_lower = name.to_lowercase();
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                let is_breakdown = stem.contains("_breakdown");
                video_names.push((name_lower, is_breakdown));
            }
        }
    }

    if video_names.is_empty() { return (0, 0); }

    // 按 baseName 分组（去掉版本号后缀），每组只保留最新版本（按版本号数字比较）
    // 与前端 groupPreviewVideos 的 localeCompare 对齐：避免字符串比较导致 _9 > _10 的问题
    let mut groups: std::collections::HashMap<String, (String, bool)> = std::collections::HashMap::new();
    for (name, is_bd) in &video_names {
        let stem = name.rsplitn(2, '.').nth(1).unwrap_or(name);
        let base_name = regex_strip_version(stem).to_string();
        let entry = groups.entry(base_name).or_insert_with(|| (name.clone(), *is_bd));
        if extract_version_number(name) > extract_version_number(&entry.0) {
            *entry = (name.clone(), *is_bd);
        }
    }

    let total = groups.len() as u32;

    // 收集 nextcloud/preview/ 中的文件名（小写）
    let nc_files: std::collections::HashSet<String> = if nc_preview_dir.exists() {
        fs::read_dir(nc_preview_dir)
            .map(|entries| entries.flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if p.is_file() { p.file_name()?.to_str().map(|n| n.to_lowercase()) } else { None }
                })
                .collect())
            .unwrap_or_default()
    } else {
        std::collections::HashSet::new()
    };

    // 收集 nextcloud/preview/breakdown/ 中的文件名（小写）
    let nc_breakdown = nc_preview_dir.join("breakdown");
    let nc_breakdown_files: std::collections::HashSet<String> = if nc_breakdown.exists() {
        fs::read_dir(&nc_breakdown)
            .map(|entries| entries.flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if p.is_file() { p.file_name()?.to_str().map(|n| n.to_lowercase()) } else { None }
                })
                .collect())
            .unwrap_or_default()
    } else {
        std::collections::HashSet::new()
    };

    // 每组只检查最新版本是否已上传
    let uploaded = groups.values().filter(|(name, is_bd)| {
        if *is_bd { nc_breakdown_files.contains(name) } else { nc_files.contains(name) }
    }).count() as u32;

    (total, uploaded)
}

/// 收集目录第一层的名称（小写，文件去扩展名，目录保留名称）
fn collect_first_level_names(dir: &Path) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
                if path.is_dir() {
                    // 序列帧目录：用目录名
                    names.insert(name.to_lowercase());
                } else if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.insert(stem.to_lowercase());
                }
            }
        }
    }
    names
}

/// 递归收集目录中所有文件的基础名（小写，去扩展名）
fn collect_base_names(dir: &Path) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                names.extend(collect_base_names(&path));
            } else if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.insert(stem.to_lowercase());
            }
        }
    }
    names
}

/// 读取或自动创建 .pgb1_project.json
fn load_or_create_config(project_path: &Path) -> Result<ProjectConfig, String> {
    let config_path = project_path.join(".pgb1_project.json");

    if config_path.exists() {
        let content =
            fs::read_to_string(&config_path).map_err(|e| format!("读取配置文件失败: {}", e))?;
        let config: ProjectConfig =
            serde_json::from_str(&content).map_err(|e| format!("解析配置文件失败: {}", e))?;
        return Ok(config);
    }

    // 自动创建配置文件（旧项目导入）
    let project_name = project_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let export_path = project_path.join("03_Render_VFX").join("VFX").join("Export");
    let enabled_tasks = if export_path.exists() {
        scan_task_names(&export_path)?
            .into_iter()
            .map(|name| name.to_lowercase())
            .collect()
    } else {
        Vec::new()
    };

    let config = ProjectConfig {
        project_name,
        created_at: chrono::Utc::now().to_rfc3339(),
        imported: true,
        deadline: None,
        enabled_tasks,
        archived_tasks: Vec::new(),
        completed_subtasks: Vec::new(),
        upload_prompted_tasks: Vec::new(),
        default_ae_file: None,
    };

    let json =
        serde_json::to_string_pretty(&config).map_err(|e| format!("序列化配置失败: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("写入配置文件失败: {}", e))?;

    Ok(config)
}

/// 扫描 Export 目录下的任务名称列表
fn scan_task_names(export_path: &Path) -> Result<Vec<String>, String> {
    let mut names = Vec::new();

    let entries =
        fs::read_dir(export_path).map_err(|e| format!("无法读取 Export 目录: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if !name.starts_with('.') {
                names.push(name.to_string());
            }
        }
    }

    names.sort();
    Ok(names)
}

/// 扫描指定目录，返回文件和子目录列表（非递归，只扫一层）
#[tauri::command]
pub fn scan_directory(dir_path: String) -> Result<Vec<FileEntry>, String> {
    let dir = Path::new(&dir_path);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let dir_entries = fs::read_dir(dir).map_err(|e| format!("无法读取目录: {}", e))?;

    for entry in dir_entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // 跳过隐藏文件/目录
        if name.starts_with('.') {
            continue;
        }

        let is_dir = path.is_dir();
        let size_bytes = if is_dir {
            0
        } else {
            path.metadata().map(|m| m.len()).unwrap_or(0)
        };
        let extension = if is_dir {
            String::new()
        } else {
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase()
        };

        entries.push(FileEntry {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir,
            size_bytes,
            extension,
        });
    }

    // 目录在前，文件在后；各自按名称排序
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));

    Ok(entries)
}

/// 在系统文件管理器中打开指定路径
#[tauri::command]
pub fn open_in_explorer(path: String) -> Result<(), String> {
    let target = Path::new(&path);
    if !target.exists() {
        return Err(format!("路径不存在: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(target)
            .spawn()
            .map_err(|e| format!("打开文件管理器失败: {}", e))?;
    }

    Ok(())
}

/// 递归计算目录大小（字节）
fn calc_dir_size(path: &Path) -> u64 {
    let mut size = 0u64;

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file() {
                size += entry_path.metadata().map(|m| m.len()).unwrap_or(0);
            } else if entry_path.is_dir() {
                size += calc_dir_size(&entry_path);
            }
        }
    }

    size
}

/// 扫描任务的素材列表（从 00_original 读取，关联各目录判定进度）
#[tauri::command]
pub fn scan_materials(task_path: String) -> Result<Vec<MaterialInfo>, String> {
    let task_dir = Path::new(&task_path);
    let original_dir = task_dir.join("00_original");

    if !original_dir.exists() {
        return Ok(Vec::new());
    }

    // 判断是否为 Prototype 任务
    let task_name = task_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if task_name.to_lowercase() == "prototype" {
        return scan_materials_prototype(task_dir);
    }

    let scale_dir = task_dir.join("01_scale");
    let done_dir = task_dir.join("02_done");

    // 获取 nextcloud 路径：从 task_path 向上推导
    // task_path = .../Export/{TaskName}
    // nextcloud = .../nextcloud/{TaskName}
    let nextcloud_dir = task_dir
        .parent() // Export/
        .and_then(|p| p.parent()) // VFX/
        .map(|vfx| vfx.join("nextcloud").join(task_dir.file_name().unwrap_or_default()));

    let mut materials = Vec::new();

    let entries =
        fs::read_dir(&original_dir).map_err(|e| format!("无法读取 00_original: {}", e))?;

    // ── Phase 1: 分类收集所有条目 ──
    // 目录 → 已规范化的序列帧
    // 文件（stem 末尾 _NN 纯数字后缀）→ 散落的序列帧候选
    // 文件（其他）→ 独立文件（静帧/视频/其他）
    let mut dir_entries: Vec<(std::path::PathBuf, String)> = Vec::new();
    let mut seq_candidates: std::collections::HashMap<String, Vec<std::path::PathBuf>> =
        std::collections::HashMap::new();
    let mut standalone_files: Vec<(std::path::PathBuf, String)> = Vec::new();
    let mut dir_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取失败: {}", e))?;
        let path = entry.path();

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // 跳过隐藏文件
        if file_name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            dir_names.insert(file_name.clone());
            dir_entries.push((path, file_name));
        } else {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            // 检查 stem 末尾是否有 _NN 纯数字后缀（序列帧特征）
            let seq_base = if let Some(pos) = stem.rfind('_') {
                let suffix = &stem[pos + 1..];
                if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                    Some(stem[..pos].to_string())
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(base) = seq_base {
                seq_candidates.entry(base).or_default().push(path);
            } else {
                standalone_files.push((path, file_name));
            }
        }
    }

    // ── Phase 2: 处理目录（已规范化的序列帧） ──
    for (path, file_name) in &dir_entries {
        let frame_count = count_frames(path);
        let first_frame = find_first_frame(path);
        let base_name = file_name.clone();

        let progress = determine_progress_sequence(&base_name, &done_dir, &nextcloud_dir);
        let scales = collect_scales_for_sequence(&base_name, &done_dir);
        let fps = collect_fps_for_sequence(&base_name, &done_dir);

        // 优先取 02_done 中精灵图三件套大小，回退到 00_original 目录大小
        let size_bytes = done_size_sequence(&done_dir, &base_name)
            .unwrap_or_else(|| calc_dir_size(path));

        materials.push(MaterialInfo {
            name: base_name,
            file_name: file_name.clone(),
            path: path.to_string_lossy().to_string(),
            material_type: MaterialType::Sequence,
            progress,
            size_bytes,
            frame_count,
            extension: "seq".to_string(),
            preview_path: first_frame,
            scales,
            fps,
        });
    }

    // ── Phase 3: 处理散落的序列帧候选组 ──
    for (base_name, mut files) in seq_candidates {
        // 同名目录已存在 → 散落文件是残留，跳过
        if dir_names.contains(&base_name) {
            continue;
        }

        if files.len() > 1 {
            // 多文件同基础名 → 未规范化的序列帧，合并显示
            files.sort();
            let frame_count = files.len() as u32;
            let first_frame = files.first().map(|p| p.to_string_lossy().to_string());
            let progress = determine_progress_sequence(&base_name, &done_dir, &nextcloud_dir);
            let scales = collect_scales_for_sequence(&base_name, &done_dir);
            let fps = collect_fps_for_sequence(&base_name, &done_dir);
            let size_bytes = done_size_sequence(&done_dir, &base_name)
                .unwrap_or_else(|| {
                    files
                        .iter()
                        .map(|f| f.metadata().map(|m| m.len()).unwrap_or(0))
                        .sum()
                });

            materials.push(MaterialInfo {
                name: base_name.clone(),
                file_name: base_name.clone(),
                path: original_dir.to_string_lossy().to_string(),
                material_type: MaterialType::Sequence,
                progress,
                size_bytes,
                frame_count,
                extension: "seq".to_string(),
                preview_path: first_frame,
                scales,
                fps,
            });
        } else if let Some(path) = files.into_iter().next() {
            // 单文件（如 _01 的静帧）→ 移入独立文件列表
            let fname = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            standalone_files.push((path, fname));
        }
    }

    // ── Phase 4: 处理独立文件（静帧/视频/其他） ──
    for (path, file_name) in standalone_files {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // 提取基础名（去掉扩展名，如有 _01 后缀也去掉）
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let base_name = stem.strip_suffix("_01").unwrap_or(stem).to_string();

        let material_type = match ext.as_str() {
            "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" => MaterialType::Image,
            "mp4" | "mov" | "avi" | "webm" => MaterialType::Video,
            _ => MaterialType::Other,
        };

        let progress = if material_type == MaterialType::Image {
            determine_progress_image(&base_name, &scale_dir, &done_dir, &nextcloud_dir)
        } else {
            MaterialProgress::Original
        };

        let scales = if material_type == MaterialType::Image {
            collect_scales_for_image(&base_name, &scale_dir)
        } else {
            Vec::new()
        };

        // 优先取 02_done 中的文件大小，回退到 00_original
        let size_bytes = if material_type == MaterialType::Image {
            done_size_image(&done_dir, &base_name)
                .unwrap_or_else(|| path.metadata().map(|m| m.len()).unwrap_or(0))
        } else {
            path.metadata().map(|m| m.len()).unwrap_or(0)
        };

        materials.push(MaterialInfo {
            name: base_name,
            file_name,
            path: path.to_string_lossy().to_string(),
            material_type,
            progress,
            size_bytes,
            frame_count: 0,
            extension: ext,
            preview_path: Some(path.to_string_lossy().to_string()),
            scales,
            fps: None,
        });
    }

    // 按名称排序
    materials.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(materials)
}

/// Prototype 特例：扫描子分类目录下的素材
fn scan_materials_prototype(task_dir: &Path) -> Result<Vec<MaterialInfo>, String> {
    let original_dir = task_dir.join("00_original");
    let scale_dir = task_dir.join("01_scale");
    let done_dir = task_dir.join("02_done");
    let nextcloud_dir = task_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|vfx| vfx.join("nextcloud").join("Prototype"));

    let mut materials = Vec::new();

    let sub_entries = fs::read_dir(&original_dir)
        .map_err(|e| format!("无法读取 Prototype/00_original: {}", e))?;

    for sub_entry in sub_entries {
        let sub_entry = sub_entry.map_err(|e| format!("读取失败: {}", e))?;
        let sub_path = sub_entry.path();
        if !sub_path.is_dir() {
            continue;
        }

        let sub_name = sub_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if sub_name.starts_with('.') {
            continue;
        }

        let inner_entries = fs::read_dir(&sub_path)
            .map_err(|e| format!("无法读取子分类 {}: {}", sub_name, e))?;

        for entry in inner_entries {
            let entry = entry.map_err(|e| format!("读取失败: {}", e))?;
            let path = entry.path();

            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            if file_name.starts_with('.') {
                continue;
            }

            if path.is_dir() {
                // 序列帧
                let frame_count = count_frames(&path);
                let first_frame = find_first_frame(&path);
                let size_bytes = calc_dir_size(&path);
                let base_name = file_name.clone();

                let progress = determine_progress_prototype_seq(
                    &base_name,
                    &sub_name,
                    &done_dir,
                    &nextcloud_dir,
                );
                let scales = collect_scales_for_proto_sequence(&base_name, &sub_name, &done_dir);
                let fps = collect_fps_for_sequence(&base_name, &done_dir);

                materials.push(MaterialInfo {
                    name: format!("{}/{}", sub_name, base_name),
                    file_name,
                    path: path.to_string_lossy().to_string(),
                    material_type: MaterialType::Sequence,
                    progress,
                    size_bytes,
                    frame_count,
                    extension: "seq".to_string(),
                    preview_path: first_frame,
                    scales,
                    fps,
                });
            } else {
                // 单个文件
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let size_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                let base_name = stem.strip_suffix("_01").unwrap_or(stem).to_string();

                let material_type = match ext.as_str() {
                    "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" => MaterialType::Image,
                    "mp4" | "mov" | "avi" | "webm" => MaterialType::Video,
                    _ => MaterialType::Other,
                };

                let progress = if material_type == MaterialType::Image {
                    determine_progress_prototype_img(
                        &base_name,
                        &sub_name,
                        &scale_dir,
                        &done_dir,
                        &nextcloud_dir,
                    )
                } else {
                    MaterialProgress::Original
                };

                let scales = if material_type == MaterialType::Image {
                    collect_scales_for_proto_image(&base_name, &sub_name, &scale_dir)
                } else {
                    Vec::new()
                };

                materials.push(MaterialInfo {
                    name: format!("{}/{}", sub_name, base_name),
                    file_name,
                    path: path.to_string_lossy().to_string(),
                    material_type,
                    progress,
                    size_bytes,
                    frame_count: 0,
                    extension: ext,
                    preview_path: Some(path.to_string_lossy().to_string()),
                    scales,
                    fps: None,
                });
            }
        }
    }

    materials.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(materials)
}

fn determine_progress_prototype_img(
    base_name: &str,
    sub_name: &str,
    scale_dir: &Path,
    done_dir: &Path,
    nextcloud_dir: &Option<std::path::PathBuf>,
) -> MaterialProgress {
    if let Some(nc) = nextcloud_dir {
        let nc_sub = nc.join(sub_name);
        if nc_sub.exists() && find_file_in_dir(&nc_sub, base_name) {
            return MaterialProgress::Uploaded;
        }
    }
    if done_dir.exists() && find_file_in_proto_subdirs(done_dir, base_name, sub_name, "img") {
        return MaterialProgress::Done;
    }
    if scale_dir.exists() && find_file_in_proto_subdirs(scale_dir, base_name, sub_name, "") {
        return MaterialProgress::Scaled;
    }
    MaterialProgress::Original
}

fn determine_progress_prototype_seq(
    base_name: &str,
    sub_name: &str,
    done_dir: &Path,
    nextcloud_dir: &Option<std::path::PathBuf>,
) -> MaterialProgress {
    let in_nextcloud = nextcloud_dir
        .as_ref()
        .map(|nc| {
            let nc_sub = nc.join(sub_name);
            nc_sub.exists() && find_file_in_dir(&nc_sub, base_name)
        })
        .unwrap_or(false);
    let in_done_webp = done_dir.exists()
        && find_webp_in_proto_subdirs(done_dir, base_name, sub_name, "an");
    let in_done_any = done_dir.exists()
        && find_file_in_proto_subdirs(done_dir, base_name, sub_name, "an");

    if in_nextcloud {
        if !in_done_webp {
            return MaterialProgress::Broken;
        }
        return MaterialProgress::Uploaded;
    }
    if in_done_any {
        if !in_done_webp {
            return MaterialProgress::Broken;
        }
        return MaterialProgress::Done;
    }
    MaterialProgress::Original
}

/// 在 [prefix-XX]/{sub_name}/ 下查找文件
fn find_file_in_proto_subdirs(
    dir: &Path,
    base_name: &str,
    sub_name: &str,
    prefix: &str,
) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if prefix.is_empty() || dir_name.starts_with(&format!("[{}-", prefix)) {
                let sub_dir = path.join(sub_name);
                if sub_dir.exists() && find_file_in_dir(&sub_dir, base_name) {
                    return true;
                }
            }
        }
    }
    false
}

/// 在 [an-XX-YY]/{sub_name}/ 下查找 .webp 文件（Prototype 序列帧完成判定）
fn find_webp_in_proto_subdirs(dir: &Path, base_name: &str, sub_name: &str, prefix: &str) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if prefix.is_empty() || dir_name.starts_with(&format!("[{}-", prefix)) {
                let sub_dir = path.join(sub_name);
                if sub_dir.exists() {
                    if let Ok(files) = fs::read_dir(&sub_dir) {
                        for file in files.flatten() {
                            let fname = file.file_name();
                            let fname_str = fname.to_string_lossy();
                            if fname_str.starts_with(base_name) && fname_str.ends_with(".webp") {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

/// 从 02_done/[an-XX-YY]/ 目录名解析序列帧帧率（YY 部分）
fn collect_fps_for_sequence(base_name: &str, done_dir: &Path) -> Option<u32> {
    let entries = fs::read_dir(done_dir).ok()?;
    for entry in entries.flatten() {
        let dir_path = entry.path();
        if !dir_path.is_dir() { continue; }
        let dir_name = dir_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // 格式：[an-XX-YY]
        if !dir_name.starts_with("[an-") || !dir_name.ends_with(']') { continue; }
        let inner = &dir_name[4..dir_name.len()-1]; // "XX-YY"
        let mut parts = inner.split('-');
        let _scale = parts.next();
        let fps_str = parts.next().unwrap_or("");
        let Ok(fps) = fps_str.parse::<u32>() else { continue };
        // 确认该目录下有属于此 base_name 的文件
        let found = fs::read_dir(&dir_path)
            .ok()
            .map(|es| es.flatten().any(|f| {
                f.path().file_stem().and_then(|s| s.to_str()).unwrap_or("") == base_name
            }))
            .unwrap_or(false);
        if found { return Some(fps); }
    }
    None
}

/// Prototype 专用：扫描 02_done/[an-XX-YY]/{sub_name}/ 下包含该序列帧，返回比例列表
fn collect_scales_for_proto_sequence(base_name: &str, sub_name: &str, done_dir: &Path) -> Vec<u32> {
    let mut scales = Vec::new();
    let Ok(entries) = fs::read_dir(done_dir) else { return scales };
    for entry in entries.flatten() {
        let dir_path = entry.path();
        if !dir_path.is_dir() { continue; }
        let dir_name = dir_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // 格式：[an-XX-YY]
        if !dir_name.starts_with("[an-") || !dir_name.ends_with(']') { continue; }
        let inner = &dir_name[4..dir_name.len()-1]; // "XX-YY"
        let scale_str = inner.split('-').next().unwrap_or("");
        let Ok(scale) = scale_str.parse::<u32>() else { continue };
        // Prototype 文件在 [an-XX-YY]/{sub_name}/ 一层更深
        let sub_dir = dir_path.join(sub_name);
        if !sub_dir.is_dir() { continue; }
        let found = fs::read_dir(&sub_dir)
            .ok()
            .map(|entries| entries.flatten().any(|f| {
                f.path().file_stem().and_then(|s| s.to_str()).unwrap_or("") == base_name
            }))
            .unwrap_or(false);
        if found && !scales.contains(&scale) { scales.push(scale); }
    }
    scales.sort_unstable();
    scales
}

/// 扫描 02_done/ 下有哪些 [an-XX-YY] 目录包含该序列帧，返回比例列表
fn collect_scales_for_sequence(base_name: &str, done_dir: &Path) -> Vec<u32> {
    let mut scales = Vec::new();
    let Ok(entries) = fs::read_dir(done_dir) else { return scales };
    for entry in entries.flatten() {
        let dir_path = entry.path();
        if !dir_path.is_dir() { continue; }
        let dir_name = dir_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // 格式：[an-XX-YY]
        if !dir_name.starts_with("[an-") || !dir_name.ends_with(']') { continue; }
        let inner = &dir_name[4..dir_name.len()-1]; // "XX-YY"
        let scale_str = inner.split('-').next().unwrap_or("");
        let Ok(scale) = scale_str.parse::<u32>() else { continue };
        // 该目录下是否有属于该 base_name 的文件
        let found = fs::read_dir(&dir_path)
            .ok()
            .map(|entries| entries.flatten().any(|f| {
                f.path().file_stem().and_then(|s| s.to_str()).unwrap_or("") == base_name
            }))
            .unwrap_or(false);
        if found && !scales.contains(&scale) { scales.push(scale); }
    }
    scales.sort_unstable();
    scales
}

/// 扫描 01_scale/ 下有哪些比例目录包含该静帧（任意扩展名）
fn collect_scales_for_image(base_name: &str, scale_dir: &Path) -> Vec<u32> {
    let mut scales = Vec::new();
    let Ok(entries) = fs::read_dir(scale_dir) else { return scales };
    for entry in entries.flatten() {
        let dir_path = entry.path();
        if !dir_path.is_dir() { continue; }
        let dir_name = dir_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !dir_name.starts_with('[') || !dir_name.ends_with(']') { continue; }
        let Ok(scale) = dir_name[1..dir_name.len()-1].parse::<u32>() else { continue };
        // 检查该比例目录下是否存在同名文件（任意扩展名）
        let found = fs::read_dir(&dir_path)
            .ok()
            .map(|entries| entries.flatten().any(|f| {
                f.path().file_stem().and_then(|s| s.to_str()).unwrap_or("") == base_name
            }))
            .unwrap_or(false);
        if found { scales.push(scale); }
    }
    scales.sort_unstable();
    scales
}

/// Prototype 专用：扫描 01_scale/[XX]/{sub_name}/ 下有哪些比例目录包含该静帧
fn collect_scales_for_proto_image(base_name: &str, sub_name: &str, scale_dir: &Path) -> Vec<u32> {
    let mut scales = Vec::new();
    let Ok(entries) = fs::read_dir(scale_dir) else { return scales };
    for entry in entries.flatten() {
        let dir_path = entry.path();
        if !dir_path.is_dir() { continue; }
        let dir_name = dir_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !dir_name.starts_with('[') || !dir_name.ends_with(']') { continue; }
        let Ok(scale) = dir_name[1..dir_name.len()-1].parse::<u32>() else { continue };
        // 进入 [XX]/{sub_name}/ 子目录查找文件
        let sub_dir = dir_path.join(sub_name);
        if !sub_dir.is_dir() { continue; }
        let found = fs::read_dir(&sub_dir)
            .ok()
            .map(|entries| entries.flatten().any(|f| {
                f.path().file_stem().and_then(|s| s.to_str()).unwrap_or("") == base_name
            }))
            .unwrap_or(false);
        if found { scales.push(scale); }
    }
    scales.sort_unstable();
    scales
}

/// 静帧进度判定（含断裂检测）
fn determine_progress_image(
    base_name: &str,
    scale_dir: &Path,
    done_dir: &Path,
    nextcloud_dir: &Option<std::path::PathBuf>,
) -> MaterialProgress {
    let in_nextcloud = nextcloud_dir
        .as_ref()
        .map(|nc| nc.exists() && find_file_in_dir(nc, base_name))
        .unwrap_or(false);
    let in_done = done_dir.exists() && find_file_in_subdirs(done_dir, base_name, "img");
    let in_scale = scale_dir.exists() && find_file_in_subdirs(scale_dir, base_name, "");

    if in_nextcloud {
        // 最高阶段 = uploaded，往回验证 done 必须存在
        if !in_done {
            return MaterialProgress::Broken;
        }
        return MaterialProgress::Uploaded;
    }
    if in_done {
        // 最高阶段 = done，往回验证 scale 必须存在
        if !in_scale {
            return MaterialProgress::Broken;
        }
        return MaterialProgress::Done;
    }
    if in_scale {
        return MaterialProgress::Scaled;
    }
    MaterialProgress::Original
}

/// 序列帧进度判定（含断裂检测，跳过 01_scale）
fn determine_progress_sequence(
    base_name: &str,
    done_dir: &Path,
    nextcloud_dir: &Option<std::path::PathBuf>,
) -> MaterialProgress {
    let in_nextcloud = nextcloud_dir
        .as_ref()
        .map(|nc| nc.exists() && find_file_in_dir(nc, base_name))
        .unwrap_or(false);
    // 检查 02_done 中是否有 .webp（完整输出）
    let in_done_webp = done_dir.exists() && find_webp_in_subdirs(done_dir, base_name, "an");
    // 检查 02_done 中是否有任意文件（包括只有 .tps 的不完整情况）
    let in_done_any = done_dir.exists() && find_file_in_subdirs(done_dir, base_name, "an");

    if in_nextcloud {
        // 最高阶段 = uploaded，往回验证 done 必须有 .webp
        if !in_done_webp {
            return MaterialProgress::Broken;
        }
        return MaterialProgress::Uploaded;
    }
    if in_done_any {
        // 有 done 目录，但如果缺 .webp 说明三件套不完整
        if !in_done_webp {
            return MaterialProgress::Broken;
        }
        return MaterialProgress::Done;
    }
    MaterialProgress::Original
}

/// 在目录下查找含指定 base_name 的文件
fn find_file_in_dir(dir: &Path, base_name: &str) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(base_name) {
                return true;
            }
        }
    }
    false
}

/// 在子目录（[img-XX] 或 [an-XX-YY]）中查找文件
fn find_file_in_subdirs(dir: &Path, base_name: &str, prefix: &str) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // 匹配前缀（如 [img-XX] 或 [an-XX-YY]，或空前缀匹配所有如 [100]）
            if prefix.is_empty() || dir_name.starts_with(&format!("[{}-", prefix)) {
                if find_file_in_dir(&path, base_name) {
                    return true;
                }
            }
        }
    }
    false
}

/// 在 [an-XX-YY] 子目录中查找指定 base_name 的 .webp 文件（判定序列帧是否真正输出完成）
fn find_webp_in_subdirs(dir: &Path, base_name: &str, prefix: &str) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if prefix.is_empty() || dir_name.starts_with(&format!("[{}-", prefix)) {
                if let Ok(files) = fs::read_dir(&path) {
                    for file in files.flatten() {
                        let fname = file.file_name();
                        let fname_str = fname.to_string_lossy();
                        if fname_str.starts_with(base_name) && fname_str.ends_with(".webp") {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// 计算序列帧目录中的帧数
fn count_frames(dir: &Path) -> u32 {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_file())
                .count() as u32
        })
        .unwrap_or(0)
}

/// 找到序列帧目录中的第一帧
fn find_first_frame(dir: &Path) -> Option<String> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_file())
        .collect();

    files.sort_by_key(|e| e.file_name());
    files
        .first()
        .map(|e| e.path().to_string_lossy().to_string())
}

/// 查找序列帧在 02_done/[an-XX-YY]/ 中的精灵图大小（.plist + .webp，不含 .tps）
fn done_size_sequence(done_dir: &Path, base_name: &str) -> Option<u64> {
    if !done_dir.exists() {
        return None;
    }
    let entries = fs::read_dir(done_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !dir_name.starts_with("[an-") {
            continue;
        }
        // 在该子目录中累加 base_name 开头的文件大小（排除 .tps）
        let size = sum_files_in_dir(&path, base_name);
        if size > 0 {
            return Some(size);
        }
    }
    None
}

/// 查找静帧在 02_done/[img-XX]/ 中的文件大小
fn done_size_image(done_dir: &Path, base_name: &str) -> Option<u64> {
    if !done_dir.exists() {
        return None;
    }
    let entries = fs::read_dir(done_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !dir_name.starts_with("[img-") {
            continue;
        }
        if let Ok(inner) = fs::read_dir(&path) {
            for f in inner.flatten() {
                let fp = f.path();
                let name = fp.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if fp.is_file() && name.starts_with(base_name) {
                    return Some(fp.metadata().map(|m| m.len()).unwrap_or(0));
                }
            }
        }
    }
    None
}

/// 累加目录中以 base_name 开头的文件大小（排除 .tps）
fn sum_files_in_dir(dir: &Path, base_name: &str) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.starts_with(base_name) {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if ext == "tps" {
                continue;
            }
            total += path.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    total
}

/// 列出序列帧目录中的所有帧文件路径（按文件名排序）
/// 当提供 base_name 时，只返回匹配 {base_name}_NN.ext 模式的文件
/// （用于散落序列帧场景，dir_path 为 00_original/ 时过滤出指定序列的帧）
#[tauri::command]
pub fn list_sequence_frames(dir_path: String, base_name: Option<String>) -> Result<Vec<String>, String> {
    let dir = Path::new(&dir_path);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let prefix = base_name.map(|bn| format!("{}_", bn));

    let mut files: Vec<String> = Vec::new();

    let entries = fs::read_dir(dir).map_err(|e| format!("无法读取目录: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp") {
                // 有 base_name 过滤时，只取 {base_name}_NN 模式的文件
                if let Some(ref pfx) = prefix {
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    if let Some(suffix) = stem.strip_prefix(pfx.as_str()) {
                        if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                            files.push(path.to_string_lossy().to_string());
                        }
                    }
                    continue;
                }
                files.push(path.to_string_lossy().to_string());
            }
        }
    }

    files.sort();
    Ok(files)
}

/// 扫描指定素材在各工作流目录中的所有版本
#[tauri::command]
pub fn scan_material_versions(
    task_path: String,
    base_name: String,
    material_type: String,
) -> Result<Vec<MaterialVersion>, String> {
    let task_dir = Path::new(&task_path);
    let mut versions = Vec::new();

    // 00_original
    let original_dir = task_dir.join("00_original");
    if original_dir.exists() {
        collect_versions_flat(&original_dir, &base_name, "00_original", "原始", "", &mut versions);
    }

    // 01_scale — 子目录 [100], [70], [50] 等
    let scale_dir = task_dir.join("01_scale");
    if scale_dir.exists() {
        collect_versions_in_scale_dirs(&scale_dir, &base_name, &mut versions);
    }

    // 02_done — 子目录 [img-XX] 或 [an-XX-YY]
    let done_dir = task_dir.join("02_done");
    let prefix = if material_type == "sequence" { "an" } else { "img" };
    if done_dir.exists() {
        collect_versions_in_done_dirs(&done_dir, &base_name, prefix, &mut versions);
    }

    // nextcloud
    let nextcloud_dir = task_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|vfx| vfx.join("nextcloud").join(task_dir.file_name().unwrap_or_default()));
    if let Some(nc) = nextcloud_dir {
        if nc.exists() {
            collect_versions_flat(&nc, &base_name, "nextcloud", "已上传", "", &mut versions);
        }
    }

    Ok(versions)
}

fn collect_versions_flat(
    dir: &Path,
    base_name: &str,
    stage: &str,
    label: &str,
    scale: &str,
    versions: &mut Vec<MaterialVersion>,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            // 匹配文件名或目录名以 base_name 开头
            if name.starts_with(base_name) {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let size_bytes = if path.is_dir() {
                    calc_dir_size(&path)
                } else {
                    path.metadata().map(|m| m.len()).unwrap_or(0)
                };
                // 序列帧 path 本身是目录，folder_path 指向该目录本身（进入）
                // 静帧 path 是文件，folder_path 指向父目录（用于"打开所在文件夹"高亮文件）
                let folder_path = if path.is_dir() {
                    path.to_string_lossy().to_string()
                } else {
                    dir.to_string_lossy().to_string()
                };
                versions.push(MaterialVersion {
                    stage: stage.to_string(),
                    stage_label: label.to_string(),
                    scale: scale.to_string(),
                    file_path: path.to_string_lossy().to_string(),
                    folder_path,
                    extension: if path.is_dir() {
                        "seq".to_string()
                    } else {
                        ext
                    },
                    size_bytes,
                });
            }
        }
    }
}

fn collect_versions_in_scale_dirs(
    scale_dir: &Path,
    base_name: &str,
    versions: &mut Vec<MaterialVersion>,
) {
    if let Ok(entries) = fs::read_dir(scale_dir) {
        let mut dirs: Vec<_> = entries.flatten().filter(|e| e.path().is_dir()).collect();
        dirs.sort_by_key(|e| e.file_name());

        for entry in dirs {
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let scale = dir_name
                .trim_start_matches('[')
                .trim_end_matches(']')
                .to_string();
            collect_versions_flat(
                &entry.path(),
                base_name,
                "01_scale",
                "已缩放",
                &scale,
                versions,
            );
        }
    }
}

fn collect_versions_in_done_dirs(
    done_dir: &Path,
    base_name: &str,
    prefix: &str,
    versions: &mut Vec<MaterialVersion>,
) {
    if let Ok(entries) = fs::read_dir(done_dir) {
        let mut dirs: Vec<_> = entries.flatten().filter(|e| e.path().is_dir()).collect();
        dirs.sort_by_key(|e| e.file_name());

        for entry in dirs {
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if !dir_name.starts_with(&format!("[{}-", prefix)) {
                continue;
            }
            let inner = dir_name
                .trim_start_matches('[')
                .trim_end_matches(']');
            let scale = inner.split('-').nth(1).unwrap_or("").to_string();
            collect_versions_flat(
                &entry.path(),
                base_name,
                "02_done",
                "已完成",
                &scale,
                versions,
            );
        }
    }
}

// ─── Phase 6: 应用设置 ─────────────────────────────────────────────

/// 加载应用设置，若不存在则探测并创建
#[tauri::command]
pub fn load_settings<R: Runtime>(app_handle: AppHandle<R>) -> Result<AppSettings, String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录: {}", e))?;
    let config_path = config_dir.join("app_settings.json");

    if config_path.exists() {
        let content = fs::read_to_string(&config_path).map_err(|e| format!("读取设置失败: {}", e))?;
        let settings: AppSettings = serde_json::from_str(&content).map_err(|e| format!("解析设置失败: {}", e))?;
        return Ok(settings);
    }

    // 首次运行：自动探测工具路径
    let mut settings = AppSettings::default();
    
    // 探测 Imagine
    let imagine_probes = [
        "C:\\Program Files\\Imagine\\Imagine.exe",
        "C:\\Program Files (x86)\\Imagine\\Imagine.exe",
    ];
    for p in &imagine_probes {
        if Path::new(p).exists() {
            settings.workflow.imagine_path = p.to_string();
            break;
        }
    }
    // 尝试用户目录下的 Local/Programs (常见安装位置)
    if settings.workflow.imagine_path.is_empty() {
        if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA") {
            let p = Path::new(&local_appdata).join("Programs\\Imagine\\Imagine.exe");
            if p.exists() {
                settings.workflow.imagine_path = p.to_string_lossy().to_string();
            }
        }
    }

    // 探测 TexturePacker
    let tp_probes = [
        "C:\\Program Files\\CodeAndWeb\\TexturePacker\\bin\\TexturePacker.exe",
        "C:\\Program Files (x86)\\CodeAndWeb\\TexturePacker\\bin\\TexturePacker.exe",
    ];
    for p in &tp_probes {
        if Path::new(p).exists() {
            settings.workflow.texture_packer_cli_path = p.to_string();
            settings.workflow.texture_packer_gui_path = p.to_string(); // 通常是同一个 exe
            break;
        }
    }

    // 自动保存初始配置
    save_settings(app_handle, settings.clone())?;

    Ok(settings)
}

/// 保存应用设置
#[tauri::command]
pub fn save_settings<R: Runtime>(app_handle: AppHandle<R>, settings: AppSettings) -> Result<(), String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录: {}", e))?;

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(|e| format!("创建配置目录失败: {}", e))?;
    }

    let config_path = config_dir.join("app_settings.json");
    let json = serde_json::to_string_pretty(&settings).map_err(|e| format!("序列化设置失败: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("写入设置文件失败: {}", e))?;

    // 同步开机自启注册表
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app_handle.autolaunch();
    if settings.general.auto_start {
        autolaunch.enable().map_err(|e| format!("设置开机自启失败: {}", e))?;
    } else {
        autolaunch.disable().map_err(|e| format!("取消开机自启失败: {}", e))?;
    }

    Ok(())
}

// ─── Phase 5d: 格式转换 (Conversion) ───────────────────────────────────

/// 启动转换会话
#[tauri::command]
pub fn start_conversion<R: Runtime>(
    app_handle: AppHandle<R>,
    state: State<'_, ConversionState>,
    request: StartConversionRequest,
) -> Result<(), String> {
    let task_dir = Path::new(&request.task_path);
    let done_path = task_dir.join("02_done");
    let scale_dir = task_dir.join("01_scale");

    if !done_path.exists() {
        fs::create_dir_all(&done_path).map_err(|e| e.to_string())?;
    }

    // 1. 序列帧映射 (前端已传入)
    let mut sequence_fps_map = HashMap::new();
    for seq in request.sequences {
        sequence_fps_map.insert(seq.name, seq.fps);
    }

    // 如果有静帧需要转换，01_scale/ 目录必须存在
    if !request.images.is_empty() && !scale_dir.exists() {
        return Err(format!("01_scale 目录不存在，请先执行缩放操作"));
    }

    // 2. 开启 notify 递归监控 01_scale/
    let done_path_clone = done_path.clone();
    let scale_dir_clone = scale_dir.clone();
    let app_handle_inner = app_handle.clone();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        match res {
            Ok(event) => handle_file_event(event, &scale_dir_clone, &done_path_clone, &app_handle_inner),
            Err(e) => log::error!("watch error: {:?}", e),
        }
    }).map_err(|e| e.to_string())?;

    watcher.watch(&scale_dir, RecursiveMode::Recursive).map_err(|e| e.to_string())?;

    // 3. 启动 Imagine (如果选了静帧)
    let mut imagine_pid = None;
    if !request.images.is_empty() {
        let imagine_path = Path::new(&request.imagine_path);
        if imagine_path.exists() {
            // 构造参数列表：所有选中的静帧原始文件路径
            let mut args: Vec<String> = Vec::new();
            for (name, _) in &request.images {
                // 在 01_scale/[XX]/ 下查找该文件（遍历各比例目录，找到即停止）
                let mut found = false;
                if let Ok(entries) = fs::read_dir(&scale_dir) {
                    for entry in entries.flatten() {
                        if found { break; }
                        let dir_path = entry.path();
                        if !dir_path.is_dir() { continue; }
                        let dir_name = dir_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if !dir_name.starts_with('[') || !dir_name.ends_with(']') { continue; }
                        for ext in &["png", "jpg", "jpeg"] {
                            let p = dir_path.join(format!("{}.{}", name, ext));
                            if p.exists() {
                                args.push(p.to_string_lossy().to_string());
                                found = true;
                                break;
                            }
                        }
                    }
                }
            }

            if let Ok(child) = std::process::Command::new(imagine_path).args(&args).spawn() {
                let pid = child.id();
                imagine_pid = Some(pid);
                let pid_clone = pid;
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    bring_window_to_front(pid_clone);
                });
            }
        }
    }

    // 4. 保存状态（_watcher 必须存活，否则监控立即停止）
    let mut state_lock = state.lock().map_err(|e| e.to_string())?;
    *state_lock = Some(ConversionSession {
        imagine_pid,
        done_path,
        sequence_fps_map,
        _watcher: watcher,
        texture_packer_cli: PathBuf::from(request.texture_packer_cli_path),
        texture_packer_gui: PathBuf::from(request.texture_packer_gui_path),
    });

    Ok(())
}

/// 执行序列帧转换 (逐个循环)
#[tauri::command]
pub async fn execute_sequence_conversion<R: Runtime>(
    app_handle: AppHandle<R>,
    state: State<'_, ConversionState>,
    sequences: Vec<ConversionSequenceRequest>,
) -> Result<(), String> {
    let (done_path, cli_path, gui_path, fps_map) = {
        let state_lock = state.lock().map_err(|e| e.to_string())?;
        let session = state_lock.as_ref().ok_or("未启动转换会话")?;
        (
            session.done_path.clone(),
            session.texture_packer_cli.clone(),
            session.texture_packer_gui.clone(),
            session.sequence_fps_map.clone(),
        )
    };

    let task_dir = done_path.parent().ok_or("无效的 done 路径")?;
    let original_dir = task_dir.join("00_original");

    for seq in sequences {
        let name = &seq.name;
        let fps = fps_map.get(name).cloned().unwrap_or(24);

        // 1. 在 00_original 中寻找序列帧文件夹（序列帧不经过 01_scale）
        let source_folder = original_dir.join(name);
        if !source_folder.is_dir() {
            return Err(format!("在 00_original 中未找到序列帧文件夹: {}", name));
        }

        // 2. 调用 CLI 生成初始 .tps
        let tps_path = done_path.join(format!("{}.tps", name));
        let sheet_path = done_path.join(format!("{}.webp", name));
        let data_path = done_path.join(format!("{}.plist", name));

        let mut cli_cmd = std::process::Command::new(&cli_path);
        cli_cmd
            .arg(&source_folder)
            .arg("--sheet").arg(&sheet_path)
            .arg("--data").arg(&data_path)
            .arg("--format").arg("cocos2d-x")
            .arg("--texture-format").arg("webp")
            .arg("--webp-quality").arg("80")
            .arg("--opt").arg("RGB888")
            .arg("--size-constraints").arg("AnySize")
            .arg("--scale").arg("0.5")
            .arg("--multipack")
            .arg("--save").arg(&tps_path);

        let output = cli_cmd.output().map_err(|e| format!("CLI 启动失败: {}", e))?;
        if !output.status.success() {
            return Err(format!("CLI 执行失败: {}", String::from_utf8_lossy(&output.stderr)));
        }

        // 2.5 将 .tps 中 globalSpriteSettings.scale 从默认 1 改为 0.5
        // 找到 globalSpriteSettings 之后的第一个 <double>1</double> 并替换（兼容 LF/CRLF）
        if let Ok(content) = fs::read_to_string(&tps_path) {
            let marker = "<key>globalSpriteSettings</key>";
            let patched = if let Some(pos) = content.find(marker) {
                let (before, after) = content.split_at(pos + marker.len());
                // 在 after 中将第一个 <double>1</double> 替换为 0.5
                let after_patched = after.replacen("<double>1</double>", "<double>0.5</double>", 1);
                format!("{}{}", before, after_patched)
            } else {
                content
            };
            let _ = fs::write(&tps_path, patched);
        }

        // 3. 启动 GUI 并置前
        if let Ok(mut child) = std::process::Command::new(&gui_path).arg(&tps_path).spawn() {
            let pid = child.id();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                bring_window_to_front(pid);
            });

            // 4. 等待 GUI 退出 (阻塞循环)
            let _ = child.wait();
        } else {
            return Err(format!("无法启动 TexturePacker GUI: {}", gui_path.display()));
        }

        // 4.5 检测 .webp 是否生成（用户可能直接关闭 GUI 未点发布）
        let webp_exists = fs::read_dir(&done_path)
            .ok()
            .map(|entries| entries.flatten().any(|e| {
                let fname = e.file_name();
                let s = fname.to_string_lossy();
                let stem = Path::new(s.as_ref()).file_stem().and_then(|x| x.to_str()).unwrap_or("");
                let ext = Path::new(s.as_ref()).extension().and_then(|x| x.to_str()).unwrap_or("");
                ext == "webp" && (stem == name || stem.starts_with(&format!("{}-", name)))
            }))
            .unwrap_or(false);

        if !webp_exists {
            // 删除残留 .tps，避免后续被误判为「已输出」
            if tps_path.exists() {
                let _ = fs::remove_file(&tps_path);
            }
            let _ = app_handle.emit("sequence-conversion-failed", name.clone());
            continue;
        }

        // 5. 解析 .tps 获取最终 scale
        let final_scale = parse_tps_scale(&tps_path)?;

        // 6. 整理三件套
        let target_dir_name = format!("[an-{}-{}]", final_scale, fps);
        let target_dir = done_path.join(&target_dir_name);
        if !target_dir.exists() {
            fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
        }

        // 移动所有属于该素材的文件（支持 multipack：name.webp / name-1.webp / name.plist 等）
        if let Ok(entries) = fs::read_dir(&done_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() { continue; }
                let fname = match path.file_name().and_then(|n| n.to_str()) {
                    Some(f) => f.to_string(),
                    None => continue,
                };
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                // 匹配 name.ext 或 name-N.ext（multipack 分包）
                let belongs = matches!(ext, "webp" | "plist" | "tps")
                    && (stem == name || stem.starts_with(&format!("{}-", name)));
                if belongs {
                    let dest = target_dir.join(&fname);
                    fs::rename(&path, &dest).map_err(|e| format!("移动 {} 失败: {}", fname, e))?;
                }
            }
        }

        // 发送进度事件
        let _ = app_handle.emit("conversion-organized", name.clone());
    }

    Ok(())
}

/// 解析 .tps 获取最终 scale 百分比
/// 必须定位 globalSpriteSettings 区块内的 scale，避免误读 autoSDSettings 的 scale（永远为 1）
fn parse_tps_scale(tps_path: &Path) -> Result<u32, String> {
    let content = fs::read_to_string(tps_path).map_err(|e| e.to_string())?;
    // 先锚定 globalSpriteSettings，再在其后找 <key>scale</key>
    let marker = "<key>globalSpriteSettings</key>";
    if let Some(marker_pos) = content.find(marker) {
        let after_marker = &content[marker_pos + marker.len()..];
        if let Some(scale_key_pos) = after_marker.find("<key>scale</key>") {
            let after_key = &after_marker[scale_key_pos + "<key>scale</key>".len()..];
            if let Some(start) = after_key.find("<double>") {
                let after_tag = &after_key[start + "<double>".len()..];
                if let Some(end) = after_tag.find("</double>") {
                    let val_str = after_tag[..end].trim();
                    if let Ok(val) = val_str.parse::<f64>() {
                        return Ok((val * 100.0).round() as u32);
                    }
                }
            }
        }
    }
    Err(format!("无法从 .tps 解析 scale 值: {}", tps_path.display()))
}

/// 停止转换会话
#[tauri::command]
pub fn stop_conversion(
    state: State<'_, ConversionState>,
) -> Result<(), String> {
    let mut state_lock = state.lock().map_err(|e| e.to_string())?;
    if let Some(session) = state_lock.take() {
        // 终止 Imagine 进程
        if let Some(pid) = session.imagine_pid {
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(&["/F", "/PID", &pid.to_string()])
                    .spawn();
            }
        }
        // watcher 会在 session 销毁时自动停止
    }
    Ok(())
}

// ─── Phase 5b: 规范化 (Normalization) ───────────────────────────────────

use crate::models::{NormalizeActionType, NormalizePreviewItem};
use std::collections::HashMap;

/// 预览规范化操作
/// 扫描 00_original/，识别静帧（去 _01）和序列帧（归类到文件夹）
#[tauri::command]
pub fn preview_normalize(task_path: String) -> Result<Vec<NormalizePreviewItem>, String> {
    let task_dir = Path::new(&task_path);
    let original_dir = task_dir.join("00_original");

    if !original_dir.exists() {
        return Ok(Vec::new());
    }

    let is_prototype = task_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_lowercase() == "prototype")
        .unwrap_or(false);

    let mut preview_items = Vec::new();

    if is_prototype {
        // Prototype: 扫描 7 个固定子分类
        for cat in &PROTOTYPE_SUBCATEGORIES {
            let sub_dir = original_dir.join(cat);
            if sub_dir.is_dir() {
                scan_and_group_files(&sub_dir, &mut preview_items)?;
            }
        }
    } else {
        // 普通任务: 直接扫描 00_original
        scan_and_group_files(&original_dir, &mut preview_items)?;
    }

    // 按原始名称排序
    preview_items.sort_by(|a, b| a.original_name.cmp(&b.original_name));

    Ok(preview_items)
}

/// 执行规范化操作
/// 根据 preview_normalize 返回的预览项，执行物理重命名或移动
#[tauri::command]
pub fn execute_normalize(items: Vec<NormalizePreviewItem>) -> Result<(), String> {
    for item in items {
        let old_path = Path::new(&item.original_path);
        if !old_path.exists() {
            continue; // 原文件不存在，跳过
        }

        match item.action_type {
            NormalizeActionType::Rename => {
                // 重命名：将旧路径的基础名部分改为目标名
                let new_path = old_path
                    .parent()
                    .ok_or_else(|| format!("无法获取父目录: {}", item.original_path))?
                    .join(&item.target_name);
                fs::rename(old_path, new_path)
                    .map_err(|e| format!("重命名失败 ({} -> {}): {}", item.original_name, item.target_name, e))?;
            }
            NormalizeActionType::MoveToFolder => {
                // 移动到文件夹：创建目标文件夹并移动文件
                let parent = old_path
                    .parent()
                    .ok_or_else(|| format!("无法获取父目录: {}", item.original_path))?;
                let target_dir = parent.join(&item.target_name);
                
                if !target_dir.exists() {
                    fs::create_dir_all(&target_dir)
                        .map_err(|e| format!("创建目标目录 {} 失败: {}", item.target_name, e))?;
                }
                
                let dest_path = target_dir.join(
                    old_path
                        .file_name()
                        .ok_or_else(|| format!("无法获取文件名: {}", item.original_path))?,
                );
                fs::rename(old_path, dest_path)
                    .map_err(|e| format!("移动文件 {} 到 {} 失败: {}", item.original_name, item.target_name, e))?;
            }
        }
    }
    Ok(())
}

/// 执行缩放操作
/// 使用 image crate 进行高质量缩放 (Lanczos3)
#[tauri::command]
pub fn execute_scaling(requests: Vec<ScaleRequest>) -> Result<(), String> {
    use image::GenericImageView;
    use image::codecs::jpeg::JpegEncoder;
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;
    use std::fs::File;
    use std::io::BufWriter;

    for req in requests {
        let old_path = Path::new(&req.original_path);
        if !old_path.exists() {
            return Err(format!("原文件不存在: {}", req.original_path));
        }

        // 确保目标目录存在
        let target_dir = Path::new(&req.target_dir);
        if !target_dir.exists() {
            fs::create_dir_all(target_dir)
                .map_err(|e| format!("创建目标目录 {} 失败: {}", req.target_dir, e))?;
        }

        // 打开图像
        let img = image::open(old_path)
            .map_err(|e| format!("无法打开图像 {}: {}", req.original_path, e))?;

        // 计算新尺寸
        let (width, height) = img.dimensions();
        let new_width = (width as f64 * (req.scale_percent as f64 / 100.0)).round() as u32;
        let new_height = (height as f64 * (req.scale_percent as f64 / 100.0)).round() as u32;

        // 执行高质量缩放
        let resized = img.resize(
            new_width,
            new_height,
            image::imageops::FilterType::Lanczos3,
        );

        // 获取原扩展名
        let ext = old_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_lowercase();

        // 保存到目标路径（base_name 可能含子路径如 "symbol/h1_blur"，需确保其父目录存在）
        let dest_path = target_dir.join(format!("{}.{}", req.base_name, ext));
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录 {} 失败: {}", parent.display(), e))?;
        }
        let file = File::create(&dest_path)
            .map_err(|e| format!("无法创建文件 {}: {}", dest_path.display(), e))?;
        let ref mut w = BufWriter::new(file);

        match ext.as_str() {
            "jpg" | "jpeg" => {
                // JPEG: 质量拉满到 100，将压缩留给后续的 webp 转换
                let mut encoder = JpegEncoder::new_with_quality(w, 100);
                encoder.encode_image(&resized).map_err(|e| format!("JPEG 编码失败: {}", e))?;
            },
            "png" => {
                // PNG: 使用默认的最佳压缩 (无损)
                let encoder = PngEncoder::new(w);
                let (w, h) = resized.dimensions();
                let color_type = resized.color();
                encoder.write_image(resized.as_bytes(), w, h, color_type.into())
                    .map_err(|e| format!("PNG 编码失败: {}", e))?;
            },
            _ => {
                // 其他格式回退到默认 save
                resized.save(&dest_path)
                    .map_err(|e| format!("保存失败: {}", e))?;
            }
        }
    }

    Ok(())
}

/// 扫描目录并按基础名分组文件，生成预览项
/// 只对 stem 末尾有 _NN 纯数字后缀的文件做分组（序列帧特征），
/// 同名不同扩展名的文件（如 .jpg+.png 对）不参与分组。
fn scan_and_group_files(
    dir: &Path,
    preview_items: &mut Vec<NormalizePreviewItem>,
) -> Result<(), String> {
    let mut seq_groups: HashMap<String, Vec<PathBuf>> = HashMap::new();

    let entries = fs::read_dir(dir).map_err(|e| format!("读取目录失败: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') {
            continue;
        }

        // 只对 stem 末尾有 _NN 纯数字后缀的文件做分组
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if let Some(pos) = stem.rfind('_') {
            let suffix = &stem[pos + 1..];
            if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                let base = stem[..pos].to_string();
                seq_groups.entry(base).or_default().push(path);
            }
        }
        // 没有 _NN 数字后缀的文件不需要规范化
    }

    for (base_name, mut files) in seq_groups {
        if files.len() == 1 {
            // 单文件以 _01 结尾 → 重命名去后缀
            let path = &files[0];
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

            if stem.ends_with("_01") {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let target_name = format!("{}.{}", base_name, ext);
                preview_items.push(NormalizePreviewItem {
                    original_path: path.to_string_lossy().to_string(),
                    original_name: name.to_string(),
                    target_name,
                    action_type: NormalizeActionType::Rename,
                    is_sequence: false,
                });
            }
        } else {
            // 多文件同基础名 → 序列帧，移动到以 base_name 命名的文件夹
            files.sort();
            for path in files {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                preview_items.push(NormalizePreviewItem {
                    original_path: path.to_string_lossy().to_string(),
                    original_name: name.to_string(),
                    target_name: base_name.clone(),
                    action_type: NormalizeActionType::MoveToFolder,
                    is_sequence: true,
                });
            }
        }
    }

    Ok(())
}

// ─── Phase 5a: 拖拽上传 ─────────────────────────────────────────────

/// 收集素材的拖拽文件列表
/// 根据素材进度，选择最终产物路径（02_done > 01_scale > 00_original）
#[tauri::command]
pub fn collect_drag_files(
    task_path: String,
    materials: Vec<DragMaterialRequest>,
) -> Result<Vec<String>, String> {
    let task_dir = Path::new(&task_path);
    let done_dir = task_dir.join("02_done");
    let scale_dir = task_dir.join("01_scale");
    let original_dir = task_dir.join("00_original");

    let is_prototype = task_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_lowercase() == "prototype")
        .unwrap_or(false);

    let mut file_paths = Vec::new();

    for mat in &materials {
        if is_prototype {
            // Prototype: name 格式 "subcategory/basename"
            let (sub_name, base_name) = split_prototype_name(&mat.name);
            let paths = collect_best_files_prototype(
                &base_name,
                &mat.material_type,
                &sub_name,
                &original_dir,
                &scale_dir,
                &done_dir,
            );
            file_paths.extend(paths);
        } else {
            let paths = collect_best_files(
                &mat.name,
                &mat.material_type,
                &original_dir,
                &scale_dir,
                &done_dir,
            );
            file_paths.extend(paths);
        }
    }

    Ok(file_paths)
}

/// 普通任务：查找素材最佳文件（优先 02_done > 01_scale > 00_original）
fn collect_best_files(
    base_name: &str,
    material_type: &str,
    original_dir: &Path,
    scale_dir: &Path,
    done_dir: &Path,
) -> Vec<String> {
    if material_type == "image" {
        // 静帧：优先 02_done/[img-*]/ 中的 webp
        if done_dir.exists() {
            let files = collect_matching_files_in_subdirs(done_dir, base_name, "img");
            if !files.is_empty() {
                return files;
            }
        }
        // 回退到 01_scale
        if scale_dir.exists() {
            let files = collect_matching_files_in_subdirs(scale_dir, base_name, "");
            if !files.is_empty() {
                return files;
            }
        }
        // 回退到 00_original
        collect_matching_files_flat(original_dir, base_name)
    } else if material_type == "sequence" {
        // 序列帧：优先 02_done/[an-*]/ 中的精灵图三件套（排除 .tps）
        if done_dir.exists() {
            let files = collect_matching_files_in_subdirs(done_dir, base_name, "an");
            if !files.is_empty() {
                return files;
            }
        }
        // 回退到 00_original 中的整个序列帧文件夹
        let seq_dir = original_dir.join(base_name);
        if seq_dir.is_dir() {
            return collect_all_files_in_dir(&seq_dir);
        }
        // 回退到 00_original 中散落的序列帧文件（未规范化）
        collect_scattered_sequence_files(original_dir, base_name)
    } else {
        // video / other：直接用 00_original
        collect_matching_files_flat(original_dir, base_name)
    }
}

/// Prototype：查找素材最佳文件（多一层子分类）
fn collect_best_files_prototype(
    base_name: &str,
    material_type: &str,
    sub_name: &str,
    original_dir: &Path,
    scale_dir: &Path,
    done_dir: &Path,
) -> Vec<String> {
    if material_type == "image" {
        // 02_done/[img-*]/{sub_name}/ 中查找
        if done_dir.exists() {
            let files =
                collect_matching_files_in_proto_subdirs(done_dir, base_name, sub_name, "img");
            if !files.is_empty() {
                return files;
            }
        }
        // 01_scale/[*]/{sub_name}/
        if scale_dir.exists() {
            let files =
                collect_matching_files_in_proto_subdirs(scale_dir, base_name, sub_name, "");
            if !files.is_empty() {
                return files;
            }
        }
        // 00_original/{sub_name}/
        let sub_dir = original_dir.join(sub_name);
        collect_matching_files_flat(&sub_dir, base_name)
    } else if material_type == "sequence" {
        // 02_done/[an-*]/{sub_name}/
        if done_dir.exists() {
            let files =
                collect_matching_files_in_proto_subdirs(done_dir, base_name, sub_name, "an");
            if !files.is_empty() {
                return files;
            }
        }
        // 00_original/{sub_name}/{base_name}/
        let seq_dir = original_dir.join(sub_name).join(base_name);
        if seq_dir.is_dir() {
            return collect_all_files_in_dir(&seq_dir);
        }
        Vec::new()
    } else {
        let sub_dir = original_dir.join(sub_name);
        collect_matching_files_flat(&sub_dir, base_name)
    }
}

/// 在扁平目录中收集 base_name 开头的文件路径
fn collect_matching_files_flat(dir: &Path, base_name: &str) -> Vec<String> {
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let stem = Path::new(name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let clean_stem = stem.strip_suffix("_01").unwrap_or(stem);
            if clean_stem == base_name {
                results.push(path.to_string_lossy().to_string());
            }
        }
    }
    results
}

/// 收集散落在目录中的序列帧文件（未规范化状态，文件名形如 base_name_01.png）
fn collect_scattered_sequence_files(dir: &Path, base_name: &str) -> Vec<String> {
    let prefix = format!("{}_", base_name);
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if let Some(suffix) = stem.strip_prefix(prefix.as_str()) {
                if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                    results.push(path.to_string_lossy().to_string());
                }
            }
        }
    }
    results.sort();
    results
}

/// 在子目录（[img-XX] 或 [an-XX-YY]）中收集匹配文件的路径（排除 .tps）
fn collect_matching_files_in_subdirs(dir: &Path, base_name: &str, prefix: &str) -> Vec<String> {
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !prefix.is_empty() && !dir_name.starts_with(&format!("[{}-", prefix)) {
                continue;
            }
            if prefix.is_empty() && !dir_name.starts_with('[') {
                continue;
            }
            if let Ok(inner) = fs::read_dir(&path) {
                for f in inner.flatten() {
                    let fp = f.path();
                    if !fp.is_file() {
                        continue;
                    }
                    let name = fp.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !name.starts_with(base_name) {
                        continue;
                    }
                    // 排除 .tps
                    let ext = fp
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if ext == "tps" {
                        continue;
                    }
                    results.push(fp.to_string_lossy().to_string());
                }
            }
        }
    }
    results
}

/// Prototype 版：在 [prefix-XX]/{sub_name}/ 下收集匹配文件（排除 .tps）
fn collect_matching_files_in_proto_subdirs(
    dir: &Path,
    base_name: &str,
    sub_name: &str,
    prefix: &str,
) -> Vec<String> {
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !prefix.is_empty() && !dir_name.starts_with(&format!("[{}-", prefix)) {
                continue;
            }
            if prefix.is_empty() && !dir_name.starts_with('[') {
                continue;
            }
            let sub_dir = path.join(sub_name);
            if !sub_dir.exists() {
                continue;
            }
            if let Ok(inner) = fs::read_dir(&sub_dir) {
                for f in inner.flatten() {
                    let fp = f.path();
                    if !fp.is_file() {
                        continue;
                    }
                    let name = fp.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !name.starts_with(base_name) {
                        continue;
                    }
                    let ext = fp
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if ext == "tps" {
                        continue;
                    }
                    results.push(fp.to_string_lossy().to_string());
                }
            }
        }
    }
    results
}

/// 收集目录中所有文件路径
fn collect_all_files_in_dir(dir: &Path) -> Vec<String> {
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                results.push(path.to_string_lossy().to_string());
            }
        }
    }
    results
}

/// 解析 Prototype 素材名（"subcategory/basename" → (subcategory, basename)）
fn split_prototype_name(name: &str) -> (String, String) {
    if let Some(pos) = name.find('/') {
        (name[..pos].to_string(), name[pos + 1..].to_string())
    } else {
        (String::new(), name.to_string())
    }
}

/// 将选中素材从 02_done 复制到 nextcloud/
/// 普通任务：扁平化复制（排除 .tps）
/// Prototype：保留子分类 + 额外复制 01_scale 到 _original/
#[tauri::command]
pub fn copy_to_nextcloud(
    task_path: String,
    material_names: Vec<CopyMaterialRequest>,
) -> Result<CopyResult, String> {
    let task_dir = Path::new(&task_path);
    let task_name = task_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let done_dir = task_dir.join("02_done");
    let scale_dir = task_dir.join("01_scale");

    // 推导 nextcloud 路径
    let nextcloud_dir = task_dir
        .parent() // Export/
        .and_then(|p| p.parent()) // VFX/
        .map(|vfx| vfx.join("nextcloud").join(task_name))
        .ok_or_else(|| "无法推导 nextcloud 路径".to_string())?;

    // 确保 nextcloud 目录存在
    fs::create_dir_all(&nextcloud_dir)
        .map_err(|e| format!("创建 nextcloud 目录失败: {}", e))?;

    let is_prototype = task_name.to_lowercase() == "prototype";
    let mut copied_count = 0u32;
    let mut errors: Vec<String> = Vec::new();

    for mat in &material_names {
        let result = if is_prototype {
            let (sub_name, base_name) = split_prototype_name(&mat.name);
            copy_material_prototype(
                &base_name,
                &mat.material_type,
                &sub_name,
                &done_dir,
                &scale_dir,
                &nextcloud_dir,
            )
        } else {
            copy_material_normal(&mat.name, &mat.material_type, &done_dir, &nextcloud_dir)
        };

        match result {
            Ok(count) => copied_count += count,
            Err(e) => errors.push(format!("{}: {}", mat.name, e)),
        }
    }

    Ok(CopyResult {
        copied_count,
        errors,
    })
}

/// 普通任务：从 02_done/[img-*]/ 或 [an-*]/ 扁平化复制到 nextcloud/
fn copy_material_normal(
    base_name: &str,
    material_type: &str,
    done_dir: &Path,
    nextcloud_dir: &Path,
) -> Result<u32, String> {
    let prefix = if material_type == "sequence" {
        "an"
    } else {
        "img"
    };

    let source_files = collect_matching_files_in_subdirs(done_dir, base_name, prefix);
    if source_files.is_empty() {
        return Err("02_done 中未找到对应文件".to_string());
    }

    let mut count = 0u32;
    for src_path_str in &source_files {
        let src_path = Path::new(src_path_str);
        let file_name = src_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let dest = nextcloud_dir.join(file_name);
        fs::copy(src_path, &dest)
            .map_err(|e| format!("复制失败 {}: {}", file_name, e))?;
        count += 1;
    }
    Ok(count)
}

/// Prototype：保留子分类 + 额外复制 _original
fn copy_material_prototype(
    base_name: &str,
    material_type: &str,
    sub_name: &str,
    done_dir: &Path,
    scale_dir: &Path,
    nextcloud_dir: &Path,
) -> Result<u32, String> {
    // 创建 nextcloud/{sub_name}/ 和 _original/
    let sub_dir = nextcloud_dir.join(sub_name);
    let original_sub_dir = sub_dir.join("_original");
    fs::create_dir_all(&sub_dir)
        .map_err(|e| format!("创建子分类目录失败: {}", e))?;
    fs::create_dir_all(&original_sub_dir)
        .map_err(|e| format!("创建 _original 目录失败: {}", e))?;

    let mut count = 0u32;

    // 从 02_done 复制处理后的文件到 nextcloud/{sub_name}/
    let prefix = if material_type == "sequence" {
        "an"
    } else {
        "img"
    };
    let done_files = collect_matching_files_in_proto_subdirs(done_dir, base_name, sub_name, prefix);
    for src_path_str in &done_files {
        let src_path = Path::new(src_path_str);
        let file_name = src_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let dest = sub_dir.join(file_name);
        fs::copy(src_path, &dest)
            .map_err(|e| format!("复制 done 文件失败 {}: {}", file_name, e))?;
        count += 1;
    }

    // 从 01_scale 复制原始文件到 nextcloud/{sub_name}/_original/
    let scale_files = collect_matching_files_in_proto_subdirs(scale_dir, base_name, sub_name, "");
    for src_path_str in &scale_files {
        let src_path = Path::new(src_path_str);
        let file_name = src_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let dest = original_sub_dir.join(file_name);
        fs::copy(src_path, &dest)
            .map_err(|e| format!("复制 _original 文件失败 {}: {}", file_name, e))?;
        count += 1;
    }

    Ok(count)
}

// ─── 通用文件导入 ────────────────────────────────────

/// 将外部文件/目录复制到指定目标目录
#[tauri::command]
pub fn import_files(source_paths: Vec<String>, target_dir: String) -> Result<ImportResult, String> {
    let target = Path::new(&target_dir);
    if !target.exists() {
        fs::create_dir_all(target).map_err(|e| format!("创建目标目录失败: {}", e))?;
    }

    let mut imported: u32 = 0;
    let mut skipped: u32 = 0;
    let mut errors: Vec<String> = Vec::new();

    for src in &source_paths {
        let src_path = Path::new(src);
        let file_name = match src_path.file_name() {
            Some(n) => n,
            None => {
                errors.push(format!("无法获取文件名: {}", src));
                continue;
            }
        };

        let dest = target.join(file_name);

        // 同名文件跳过
        if dest.exists() {
            skipped += 1;
            continue;
        }

        if src_path.is_dir() {
            match copy_dir_recursive(src_path, &dest) {
                Ok(()) => imported += 1,
                Err(e) => errors.push(format!("{}: {}", src, e)),
            }
        } else {
            match fs::copy(src_path, &dest) {
                Ok(_) => imported += 1,
                Err(e) => errors.push(format!("{}: {}", src, e)),
            }
        }
    }

    Ok(ImportResult { imported_count: imported, skipped_count: skipped, errors })
}

/// 递归复制目录
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("创建目录失败: {}", e))?;

    let entries = fs::read_dir(src).map_err(|e| format!("读取目录失败: {}", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取条目失败: {}", e))?;
        let entry_dest = dest.join(entry.file_name());

        if entry.path().is_dir() {
            copy_dir_recursive(&entry.path(), &entry_dest)?;
        } else {
            fs::copy(entry.path(), &entry_dest).map_err(|e| format!("复制失败: {}", e))?;
        }
    }

    Ok(())
}

// ─── 任务管理系统 ─────────────────────────────────────────────

/// Prototype 下固定的 7 个子分类目录
const PROTOTYPE_SUBCATEGORIES: [&str; 7] = [
    "big_win",
    "infoboard",
    "loading_bonus",
    "main_ui",
    "spinbutton",
    "symbol",
    "total_win",
];

/// 加载全局任务清单（不存在则创建默认模板）
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

/// 默认的 8 个全局任务
fn default_global_tasks() -> GlobalTaskConfig {
    GlobalTaskConfig {
        tasks: vec![
            GlobalTask {
                name: "ambient".to_string(),
                children: vec![],
            },
            GlobalTask {
                name: "free spin".to_string(),
                children: vec![
                    GlobalTaskChild { name: "slow drop".to_string() },
                    GlobalTaskChild { name: "fast spin".to_string() },
                    GlobalTaskChild { name: "scatter".to_string() },
                    GlobalTaskChild { name: "freespin retrigger".to_string() },
                ],
            },
            GlobalTask {
                name: "infoboard".to_string(),
                children: vec![
                    GlobalTaskChild { name: "infoboard".to_string() },
                    GlobalTaskChild { name: "one more scatter".to_string() },
                ],
            },
            GlobalTask {
                name: "main ui".to_string(),
                children: vec![],
            },
            GlobalTask {
                name: "mouse hover".to_string(),
                children: vec![],
            },
            GlobalTask {
                name: "prototype".to_string(),
                children: vec![],
            },
            GlobalTask {
                name: "spin button".to_string(),
                children: vec![],
            },
            GlobalTask {
                name: "win highlight".to_string(),
                children: vec![
                    GlobalTaskChild { name: "small win".to_string() },
                    GlobalTaskChild { name: "wild".to_string() },
                ],
            },
        ],
    }
}

/// 小写名称转 Title Case（每个单词首字母大写）
fn to_title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
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
fn move_dir(src: &Path, dest: &Path) -> Result<(), String> {
    copy_dir_recursive(src, dest)?;
    fs::remove_dir_all(src).map_err(|e| format!("删除原目录失败: {}", e))?;
    Ok(())
}

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

    // 创建标准 6 目录骨架
    let dirs_to_create: Vec<PathBuf> = vec![
        project_dir.join("00_Game Design & Doc"),
        project_dir.join("01_Preproduction"),
        project_dir.join("02_Production"),
        project_dir.join("03_Render_VFX").join("VFX").join("Export"),
        project_dir.join("03_Render_VFX").join("VFX").join("nextcloud"),
        project_dir.join("03_Render_VFX").join("VFX").join("nextcloud").join("preview"),
        project_dir.join("03_Render_VFX").join("VFX").join("nextcloud").join("preview").join("breakdown"),
        project_dir.join("03_Render_VFX").join("VFX").join("PSD"),
        project_dir.join("03_Render_VFX").join("VFX").join("AE"),
        project_dir.join("04_Trailer"),
        project_dir.join("05_Outside"),
    ];

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

/// 打卡自动化内部实现
async fn execute_clock_action_inner(
    app_handle: tauri::AppHandle,
    action: String,
) -> Result<String, String> {
    use tauri::Manager;

    emit_progress(&app_handle, "loading-config", "正在加载配置...");

    // 1. 加载配置和密码
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

    emit_progress(&app_handle, "opening-page", "正在打开打卡网站...");

    // 2. 创建隐藏 WebView 窗口
    let webview_label = "webview-clock";
    if let Some(existing) = app_handle.get_webview_window(webview_label) {
        let _ = existing.close();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let url = config.attendance.url.clone();
    let webview_window = tauri::WebviewWindowBuilder::new(
        &app_handle,
        webview_label,
        tauri::WebviewUrl::External(url.parse().map_err(|e| format!("URL 无效: {}", e))?),
    )
    .title("PGB1 打卡")
    .inner_size(1024.0, 768.0)
    .visible(false)
    .build()
    .map_err(|e| format!("创建打卡 WebView 失败: {}", e))?;

    // 3. 等待页面加载
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    emit_progress(&app_handle, "logging-in", "正在登录...");

    // 4. 填写账号
    let fill_username_js = format!(
        r#"(function() {{
            var el = document.querySelector('input[type="email"], input[name="username"], input[name="email"], input[type="text"]');
            if (el) {{ el.value = '{}'; el.dispatchEvent(new Event('input', {{bubbles: true}})); return 'ok'; }}
            return 'not_found';
        }})()"#,
        config.username.replace('\'', "\\'").replace('"', "\\\"")
    );
    let _ = webview_window.eval(&fill_username_js);

    // 5. 填写密码
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

    // 6. 点击登录按钮（模拟完整鼠标事件链）
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

    // 7. 轮询等待登录跳转（URL 离开 login 页即成功，最多 10 秒）
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
            button_state = state;
            if button_state == "ready" || button_state == "disabled" {
                break;
            }
        }
    }

    // 判断按钮状态
    if button_state == "disabled" {
        // 按钮已禁用 = 已经打过卡了（不覆盖已有的 actual 时间）
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

        emit_progress(&app_handle, "already-done", "今天已经打过卡了");
        return Ok("已打卡（按钮已禁用）".to_string());
    }

    if button_state != "ready" {
        let reason = if button_state == "not_found" {
            "打刻页面结构可能已变化，未找到对应按钮"
        } else {
            "按钮未在 15 秒内变为可用状态，请稍后重试"
        };
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

        emit_progress(&app_handle, "success", "打卡成功");
        Ok("打卡成功".to_string())
    } else {
        emit_progress(&app_handle, "success", "打卡操作已执行，请查看浏览器确认结果");
        Ok("打卡操作已执行，请查看浏览器确认结果".to_string())
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
    use tauri::Manager;

    emit_test_progress(&app_handle, "loading-config", "正在加载配置...");

    // 1. 加载配置和密码
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

    emit_test_progress(&app_handle, "opening-page", "正在打开打卡网站...");

    // 2. 创建 **可见** WebView 窗口
    let webview_label = "webview-clock-test";
    if let Some(existing) = app_handle.get_webview_window(webview_label) {
        let _ = existing.close();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let url = config.attendance.url.clone();
    let webview_window = tauri::WebviewWindowBuilder::new(
        &app_handle,
        webview_label,
        tauri::WebviewUrl::External(url.parse().map_err(|e| format!("URL 无效: {}", e))?),
    )
    .title("打卡测试")
    .inner_size(1024.0, 768.0)
    .visible(true)
    .center()
    .build()
    .map_err(|e| format!("创建测试 WebView 失败: {}", e))?;

    // 3. 等待页面加载
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let current_url = get_webview_url(&webview_window);
    emit_test_progress(
        &app_handle,
        "page-loaded",
        &format!("页面已加载：{}", current_url),
    );

    // 如果已在打刻页（可能配置 URL 直指打刻页或登录后自动跳转），跳过登录
    if current_url.contains("record/register") {
        emit_test_progress(&app_handle, "success", "已直接到达打刻页面，测试通过！");
        return Ok(());
    }

    emit_test_progress(&app_handle, "logging-in", "正在填写账号密码...");

    // 4. 填写账号
    let fill_username_js = format!(
        r#"(function() {{
            var el = document.querySelector('input[type="email"], input[name="username"], input[name="email"], input[type="text"]');
            if (el) {{ el.value = '{}'; el.dispatchEvent(new Event('input', {{bubbles: true}})); return 'ok'; }}
            return 'not_found';
        }})()"#,
        config.username.replace('\'', "\\'").replace('"', "\\\"")
    );
    let _ = webview_window.eval(&fill_username_js);

    // 5. 填写密码
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

    emit_test_progress(&app_handle, "clicking-login", "正在点击登录...");

    // 6. 点击登录按钮
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

    // 7. 轮询等待登录跳转（最多 10 秒）
    let mut login_ok = false;
    let mut current_url = String::new();
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        current_url = get_webview_url(&webview_window);
        if !current_url.is_empty() && !current_url.contains("login") {
            login_ok = true;
            break;
        }
    }

    emit_test_progress(
        &app_handle,
        "after-login",
        &format!("登录后页面：{}", current_url),
    );

    if !login_ok {
        emit_test_progress(
            &app_handle,
            "error",
            &format!("登录失败，仍在登录页：{}", current_url),
        );
        return Err("登录失败".to_string());
    }

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

    // 如果已存在，聚焦
    if let Some(existing) = app_handle.get_webview_window(label) {
        let _ = existing.set_focus();
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
    .build()
    .map_err(|e| format!("创建日报窗口失败: {}", e))?;

    // 后台等待 Google Docs 加载完成后滚动到底部（不阻塞命令返回）
    tauri::async_runtime::spawn(async move {
        // 初始等待 2 秒让页面开始加载
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        // 轮询找到 Google Docs 真实滚动容器后执行滚动（最多等待 10 秒）
        let script = r#"
            (function scroll(retries) {
                var el = document.querySelector('.kix-appview-editor-scroller');
                if (el) {
                    el.scrollTop = el.scrollHeight;
                } else if (retries > 0) {
                    setTimeout(function() { scroll(retries - 1); }, 500);
                }
            })(20);
        "#;
        let _ = window.eval(script);
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
#[tauri::command]
pub fn show_overtime_dialog(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    let label = "reminder-overtime-setting";

    // 如果已存在，聚焦
    if let Some(existing) = app_handle.get_webview_window(label) {
        let _ = existing.set_focus();
        return Ok(());
    }

    let window = tauri::WebviewWindowBuilder::new(
        &app_handle,
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
    .map_err(|e| format!("创建加班设置窗口失败: {}", e))?;

    #[cfg(target_os = "windows")]
    {
        use window_vibrancy::apply_acrylic;
        let _ = apply_acrylic(&window, Some((0, 0, 0, 1)));
    }

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

#[tauri::command]
pub async fn translate_text(
    api_key: String,
    model: String,
    lang_a: String,
    lang_b: String,
    text: String,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("请先在设置中配置 Gemini API Key".to_string());
    }
    if text.trim().is_empty() {
        return Err("翻译内容不能为空".to_string());
    }

    let lang_a_display = match lang_a.as_str() {
        "zh-CN" => "Chinese",
        "en" => "English",
        "ja" => "Japanese",
        other => other,
    };
    let lang_b_display = match lang_b.as_str() {
        "zh-CN" => "Chinese",
        "en" => "English",
        "ja" => "Japanese",
        other => other,
    };

    let prompt = format!(
        "You are a translator between {} and {}.\nDetect the input language and translate to the other one.\nTone: concise, friendly, natural — like casual coworker chat. No fluff.\nOnly output the translation, nothing else.\n\n\"{}\"",
        lang_a_display, lang_b_display, text
    );

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model
    );

    let body = serde_json::json!({
        "contents": [{
            "parts": [{ "text": prompt }]
        }]
    });

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("X-goog-api-key", &api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("网络错误：{}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let err_text = response.text().await.unwrap_or_default();
        return Err(format!("API 错误 {}: {}", status, err_text));
    }

    let resp_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败：{}", e))?;

    let translated = resp_json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| "响应格式异常，无法提取翻译结果".to_string())?
        .trim()
        .to_string();

    Ok(translated)
}

#[tauri::command]
pub async fn toggle_translator_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::hotkey::do_toggle_window(&app);
    Ok(())
}

// ─── 快捷方式栏 ─────────────────────────────────────────────

/// 加载快捷方式列表
#[tauri::command]
pub fn load_shortcuts<R: Runtime>(app_handle: AppHandle<R>) -> Result<Vec<Shortcut>, String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录: {}", e))?;
    let config_path = config_dir.join("shortcuts.json");

    if !config_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取快捷方式失败: {}", e))?;
    let config: ShortcutsConfig = serde_json::from_str(&content)
        .map_err(|e| format!("解析快捷方式失败: {}", e))?;

    Ok(config.shortcuts)
}

/// 保存快捷方式列表
#[tauri::command]
pub fn save_shortcuts<R: Runtime>(
    app_handle: AppHandle<R>,
    shortcuts: Vec<Shortcut>,
) -> Result<(), String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录: {}", e))?;

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("创建配置目录失败: {}", e))?;
    }

    let config = ShortcutsConfig { shortcuts };
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化快捷方式失败: {}", e))?;
    fs::write(config_dir.join("shortcuts.json"), json)
        .map_err(|e| format!("写入快捷方式失败: {}", e))?;

    Ok(())
}

/// 启动快捷方式（应用/文件夹/网页）
#[tauri::command]
pub fn launch_shortcut(shortcut_type: String, path: String) -> Result<(), String> {
    match shortcut_type.as_str() {
        "app" => {
            // 启动 exe
            let exe = Path::new(&path);
            if !exe.exists() {
                return Err(format!("应用不存在: {}", path));
            }
            std::process::Command::new(exe)
                .current_dir(exe.parent().unwrap_or(exe))
                .spawn()
                .map_err(|e| format!("启动应用失败: {}", e))?;
        }
        "folder" => {
            // 用 Explorer 打开文件夹
            let dir = Path::new(&path);
            if !dir.exists() {
                return Err(format!("文件夹不存在: {}", path));
            }
            std::process::Command::new("explorer")
                .arg(dir)
                .spawn()
                .map_err(|e| format!("打开文件夹失败: {}", e))?;
        }
        "web" => {
            // 用系统默认浏览器打开 URL
            std::process::Command::new("cmd")
                .args(["/C", "start", "", &path])
                .spawn()
                .map_err(|e| format!("打开网页失败: {}", e))?;
        }
        _ => return Err(format!("未知快捷方式类型: {}", shortcut_type)),
    }
    Ok(())
}

/// 扫描 Windows 开始菜单和桌面，返回所有 .lnk 快捷方式解析后的应用列表
#[tauri::command]
pub fn scan_app_shortcuts() -> Result<Vec<AppShortcut>, String> {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

    // 扫描路径：用户开始菜单、系统开始菜单、桌面
    let mut scan_dirs: Vec<std::path::PathBuf> = Vec::new();

    if let Ok(appdata) = std::env::var("APPDATA") {
        scan_dirs.push(
            std::path::Path::new(&appdata)
                .join("Microsoft\\Windows\\Start Menu\\Programs"),
        );
    }
    if let Ok(programdata) = std::env::var("PROGRAMDATA") {
        scan_dirs.push(
            std::path::Path::new(&programdata)
                .join("Microsoft\\Windows\\Start Menu\\Programs"),
        );
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        scan_dirs.push(std::path::Path::new(&userprofile).join("Desktop"));
    }

    // 递归收集所有 .lnk 文件
    fn collect_lnk_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_lnk_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("lnk") {
                out.push(path);
            }
        }
    }

    let mut lnk_files: Vec<std::path::PathBuf> = Vec::new();
    for dir in &scan_dirs {
        collect_lnk_files(dir, &mut lnk_files);
    }

    // COM 初始化（STA）
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let mut results: Vec<AppShortcut> = Vec::new();

    for lnk_path in &lnk_files {
        let target = unsafe { resolve_lnk(lnk_path) };
        let Some(target_path) = target else { continue };

        // 只保留以 .exe 结尾的目标
        if !target_path.to_lowercase().ends_with(".exe") {
            continue;
        }

        // 名称：lnk 文件名去掉 .lnk
        let name = lnk_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        if name.is_empty() {
            continue;
        }

        results.push(AppShortcut { name, target_path });
    }

    // 按名称排序，去重（同名只保留第一个）
    results.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    results.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase());

    Ok(results)
}

/// 解析单个 .lnk 文件，返回目标路径
unsafe fn resolve_lnk(lnk_path: &std::path::Path) -> Option<String> {
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER, IPersistFile, STGM_READ};
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::Interface;

    let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
    let persist_file: IPersistFile = shell_link.cast().ok()?;

    // 将路径转为宽字符（null 结尾）
    let wide: Vec<u16> = lnk_path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    persist_file.Load(windows::core::PCWSTR(wide.as_ptr()), STGM_READ).ok()?;

    // 读取目标路径（buf 作为 &mut [u16] 传入）
    let mut buf = [0u16; 260];
    shell_link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;

    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let target = String::from_utf16_lossy(&buf[..end]);

    if target.is_empty() {
        None
    } else {
        Some(target)
    }
}

/// 提取 exe 文件图标，保存为 PNG，返回缓存文件的绝对路径
///
/// - `exe_path`：目标 exe 文件路径
/// - `icon_id`：快捷方式 UUID，用作缓存文件名（`shortcut_icons/<icon_id>.png`）
#[tauri::command]
pub fn extract_exe_icon<R: Runtime>(
    app_handle: AppHandle<R>,
    exe_path: String,
    icon_id: String,
) -> Result<String, String> {
    use windows::Win32::UI::Shell::SHGetFileInfoW;
    use windows::Win32::UI::Shell::{SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};

    // 确保缓存目录存在
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录: {}", e))?;
    let icons_dir = config_dir.join("shortcut_icons");
    if !icons_dir.exists() {
        fs::create_dir_all(&icons_dir)
            .map_err(|e| format!("创建图标缓存目录失败: {}", e))?;
    }

    let out_path = icons_dir.join(format!("{}.png", icon_id));

    // 将 exe 路径转为宽字符
    let wide: Vec<u16> = exe_path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO, DestroyIcon};
        use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC, GetDIBits, DeleteObject,
            BITMAPINFOHEADER, BITMAPINFO, DIB_RGB_COLORS, BI_RGB};

        // 1. 先用 SHGetFileInfoW 拿到文件系统索引（iIcon）
        let mut info = SHFILEINFOW::default();
        let ret = SHGetFileInfoW(
            windows::core::PCWSTR(wide.as_ptr()),
            Default::default(),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if ret == 0 {
            return Err(format!("SHGetFileInfoW 失败: {}", exe_path));
        }
        // 先销毁 SHGetFileInfoW 给的小图标，我们改用大列表
        let _ = DestroyIcon(info.hIcon);
        let icon_index = info.iIcon;

        // 2. 尝试用 SHGetImageList(SHIL_JUMBO=4) 获取 256×256 图标列表
        //    降级顺序：JUMBO(256) → EXTRALARGE(48) → 回退到 SHGFI_LARGEICON(32)
        use windows::Win32::UI::Shell::{SHGetImageList, SHIL_JUMBO, SHIL_EXTRALARGE};
        use windows::Win32::UI::Controls::IImageList;

        let (hicon, icon_size): (_, i32) = 'resolve: {
            // 尝试 JUMBO (256×256)
            if let Ok(img_list) = SHGetImageList::<IImageList>(SHIL_JUMBO as i32) {
                if let Ok(ico) = img_list.GetIcon(icon_index, 0) {
                    break 'resolve (ico, 256);
                }
            }
            // 尝试 EXTRALARGE (48×48)
            if let Ok(img_list) = SHGetImageList::<IImageList>(SHIL_EXTRALARGE as i32) {
                if let Ok(ico) = img_list.GetIcon(icon_index, 0) {
                    break 'resolve (ico, 48);
                }
            }
            // 最后降级：重新用 SHGetFileInfoW 的 32×32
            let mut info2 = SHFILEINFOW::default();
            SHGetFileInfoW(
                windows::core::PCWSTR(wide.as_ptr()),
                Default::default(),
                Some(&mut info2),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            );
            (info2.hIcon, 32)
        };

        // 3. 用 GetIconInfo 取出颜色位图句柄
        let mut icon_info = ICONINFO::default();
        if GetIconInfo(hicon, &mut icon_info).is_err() {
            let _ = DestroyIcon(hicon);
            return Err("GetIconInfo 失败".to_string());
        }
        let hbm_color = icon_info.hbmColor;
        let hbm_mask  = icon_info.hbmMask;
        let screen_dc = GetDC(None);
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: icon_size,
                biHeight: -icon_size, // 负值 = top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [Default::default()],
        };
        let pixel_count = (icon_size * icon_size) as usize;
        let mut pixels: Vec<u8> = vec![0u8; pixel_count * 4];
        GetDIBits(
            screen_dc,
            hbm_color,
            0,
            icon_size as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, screen_dc);

        // 4. 清理 GDI 资源
        let _ = DeleteObject(hbm_color);
        let _ = DeleteObject(hbm_mask);
        let _ = DestroyIcon(hicon);

        // 5. BGRA → RGBA 转换
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.swap(0, 2); // B↔R
        }

        // 6. 用 image crate 保存 PNG
        image::save_buffer(
            &out_path,
            &pixels,
            icon_size as u32,
            icon_size as u32,
            image::ColorType::Rgba8,
        )
        .map_err(|e| format!("保存图标 PNG 失败: {}", e))?;
    }

    // 返回正斜杠路径，确保前端 asset 协议能正确解析
    Ok(out_path.to_string_lossy().replace('\\', "/"))
}

/// 将用户选择的图片文件复制到图标缓存目录，统一转为 PNG 存储。
/// 支持 PNG / JPG / ICO / BMP / WEBP 等 image crate 支持的格式。
///
/// - `src_path`：用户选择的图片文件路径
/// - `icon_id`：快捷方式 UUID，用作缓存文件名
#[tauri::command]
pub fn copy_icon_to_cache<R: Runtime>(
    app_handle: AppHandle<R>,
    src_path: String,
    icon_id: String,
) -> Result<String, String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录: {}", e))?;
    let icons_dir = config_dir.join("shortcut_icons");
    if !icons_dir.exists() {
        std::fs::create_dir_all(&icons_dir)
            .map_err(|e| format!("创建图标缓存目录失败: {}", e))?;
    }

    let out_path = icons_dir.join(format!("{}.png", icon_id));

    let img = image::open(&src_path)
        .map_err(|e| format!("无法读取图片文件: {}", e))?;
    img.save_with_format(&out_path, image::ImageFormat::Png)
        .map_err(|e| format!("保存图标缓存失败: {}", e))?;

    Ok(out_path.to_string_lossy().replace('\\', "/"))
}

/// 获取网页 favicon，保存为 PNG，返回缓存路径。
/// 尺寸 < 32×32 或获取失败时返回 null（前端降级用默认图标）。
///
/// 策略：
///  1. 试 `{origin}/favicon.ico`
///  2. 失败则抓 HTML，找 `<link rel="icon">` 最大图标
///  3. 下载后用 image crate 解码，验证 ≥32×32，保存 PNG
#[tauri::command]
pub async fn fetch_favicon<R: Runtime>(
    app_handle: AppHandle<R>,
    url: String,
    icon_id: String,
) -> Result<Option<String>, String> {
    // 确保缓存目录存在
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录: {}", e))?;
    let icons_dir = config_dir.join("shortcut_icons");
    if !icons_dir.exists() {
        fs::create_dir_all(&icons_dir)
            .map_err(|e| format!("创建图标缓存目录失败: {}", e))?;
    }
    let out_path = icons_dir.join(format!("{}.png", icon_id));

    // 解析 origin（scheme + host）
    let origin = extract_origin(&url)?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    // 收集候选 favicon URL（按优先级排序）
    let mut candidates: Vec<String> = Vec::new();

    // 先抓 HTML 找 <link rel="icon"> 候选，放在最前（通常更高质量）
    if let Ok(html_candidates) = fetch_favicon_from_html(&client, &url, &origin).await {
        candidates.extend(html_candidates);
    }

    // 追加 /favicon.ico 兜底
    candidates.push(format!("{}/favicon.ico", origin));

    // 逐个尝试下载并验证尺寸
    for candidate_url in &candidates {
        if let Ok(Some(path)) = try_download_favicon(&client, candidate_url, &out_path).await {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

/// 从 URL 提取 origin（如 "https://www.example.com"）
fn extract_origin(url: &str) -> Result<String, String> {
    // 简单解析：找到 scheme://host 部分
    let url = url.trim();
    // 确保有 scheme
    let url = if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("https://{}", url)
    };

    // 取出 scheme://host[:port]
    let after_scheme = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(&url);
    let host = after_scheme.split('/').next().unwrap_or(after_scheme);
    let scheme = url.split("://").next().unwrap_or("https");
    Ok(format!("{}://{}", scheme, host))
}

/// 抓取页面 HTML，解析 <link rel="icon"> 返回候选 URL 列表（大尺寸优先）
async fn fetch_favicon_from_html(
    client: &reqwest::Client,
    page_url: &str,
    origin: &str,
) -> Result<Vec<String>, ()> {
    let resp = client.get(page_url).send().await.map_err(|_| ())?;
    let html = resp.text().await.map_err(|_| ())?;

    // 用简单文本解析提取 <link rel="icon|shortcut icon|apple-touch-icon" href="...">
    // 同时提取 sizes 属性用于排序
    let mut icons: Vec<(u32, String)> = Vec::new(); // (max_dim, url)

    // 遍历所有 <link ... > 标签
    let lower = html.to_lowercase();
    let mut search_from = 0;
    while let Some(tag_start) = lower[search_from..].find("<link") {
        let abs_start = search_from + tag_start;
        let tag_end = lower[abs_start..].find('>').unwrap_or(0) + abs_start + 1;
        let tag = &html[abs_start..tag_end];
        let tag_lower = tag.to_lowercase();

        if tag_lower.contains("rel=") {
            let rel = extract_attr(tag, "rel").unwrap_or_default().to_lowercase();
            if rel.contains("icon") {
                let href = extract_attr(tag, "href").unwrap_or_default();
                if !href.is_empty() {
                    // 解析 sizes 属性，取最大维度
                    let max_dim = extract_attr(tag, "sizes")
                        .and_then(|s| {
                            s.split_whitespace()
                                .filter_map(|sz| {
                                    let parts: Vec<&str> = sz.split('x').collect();
                                    if parts.len() == 2 {
                                        parts[0].parse::<u32>().ok()
                                    } else {
                                        None
                                    }
                                })
                                .max()
                        })
                        .unwrap_or(0);

                    // 转为绝对 URL
                    let abs_url = to_absolute_url(&href, origin);
                    icons.push((max_dim, abs_url));
                }
            }
        }
        search_from = tag_end;
    }

    // 按尺寸降序排列（大图优先）
    icons.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(icons.into_iter().map(|(_, u)| u).collect())
}

/// 从 HTML 标签提取指定属性值
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let tag_lower = tag.to_lowercase();
    let search = format!("{}=", attr.to_lowercase());
    let pos = tag_lower.find(&search)?;
    let rest = &tag[pos + search.len()..];
    let value = if rest.starts_with('"') {
        rest[1..].split('"').next()?
    } else if rest.starts_with('\'') {
        rest[1..].split('\'').next()?
    } else {
        rest.split_whitespace().next()?
            .trim_end_matches('>')
    };
    Some(value.to_string())
}

/// 将相对路径转为绝对 URL
fn to_absolute_url(href: &str, origin: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_string()
    } else if href.starts_with("//") {
        format!("https:{}", href)
    } else if href.starts_with('/') {
        format!("{}{}", origin, href)
    } else {
        format!("{}/{}", origin, href)
    }
}

/// 下载 favicon URL，解码验证尺寸（≥32×32），保存 PNG，返回路径或 None
async fn try_download_favicon(
    client: &reqwest::Client,
    favicon_url: &str,
    out_path: &std::path::Path,
) -> Result<Option<String>, ()> {
    let resp = client.get(favicon_url).send().await.map_err(|_| ())?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let bytes = resp.bytes().await.map_err(|_| ())?;
    if bytes.is_empty() {
        return Ok(None);
    }

    // 用 image crate 解码（支持 png/jpg/ico/gif 等）
    let img = image::load_from_memory(&bytes).map_err(|_| ())?;

    // 验证尺寸 ≥ 32×32
    if img.width() < 32 || img.height() < 32 {
        return Ok(None);
    }

    // 保存为 PNG
    img.save(out_path).map_err(|_| ())?;

    Ok(Some(out_path.to_string_lossy().replace('\\', "/")))
}

/// 重命名素材（所有工作流版本同步改名，包括 nextcloud）
///
/// 扫描 00_original / 01_scale/**/ / 02_done/**/ / nextcloud/<task>/ 中
/// 所有以 base_name 开头的文件或目录，将前缀替换为 new_base_name。
/// 序列帧：重命名目录本身 + 内部所有帧文件前缀。
#[tauri::command]
pub fn rename_material(
    task_path: String,
    base_name: String,
    new_base_name: String,
    material_type: String,
) -> Result<(), String> {
    let task_dir = Path::new(&task_path);
    let is_sequence = material_type == "sequence";

    let mut dirs_to_scan: Vec<std::path::PathBuf> = vec![task_dir.join("00_original")];

    let scale_dir = task_dir.join("01_scale");
    if scale_dir.exists() {
        if let Ok(entries) = fs::read_dir(&scale_dir) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    dirs_to_scan.push(e.path());
                }
            }
        }
    }

    let done_dir = task_dir.join("02_done");
    if done_dir.exists() {
        if let Ok(entries) = fs::read_dir(&done_dir) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    dirs_to_scan.push(e.path());
                }
            }
        }
    }

    let nc_dir = task_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|vfx| vfx.join("nextcloud").join(task_dir.file_name().unwrap_or_default()));
    if let Some(ref nc) = nc_dir {
        if nc.exists() {
            dirs_to_scan.push(nc.clone());
        }
    }

    for dir in &dirs_to_scan {
        if !dir.exists() {
            continue;
        }
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if !file_name.starts_with(base_name.as_str()) {
                continue;
            }
            let suffix = &file_name[base_name.len()..];
            let new_name = format!("{}{}", new_base_name, suffix);
            let new_path = dir.join(&new_name);

            if is_sequence && path.is_dir() {
                // 先重命名目录内帧文件
                if let Ok(frames) = fs::read_dir(&path) {
                    for frame_entry in frames.flatten() {
                        let fpath = frame_entry.path();
                        let fname = match fpath.file_name().and_then(|n| n.to_str()) {
                            Some(n) => n.to_string(),
                            None => continue,
                        };
                        if fname.starts_with(base_name.as_str()) {
                            let fsuffix = &fname[base_name.len()..];
                            let new_fname = format!("{}{}", new_base_name, fsuffix);
                            let _ = fs::rename(&fpath, fpath.parent().expect("read_dir 帧文件必有父目录").join(&new_fname));
                        }
                    }
                }
                // 再重命名目录本身
                fs::rename(&path, &new_path)
                    .map_err(|e| format!("重命名目录 {} 失败: {}", file_name, e))?;
            } else if !path.is_dir() {
                fs::rename(&path, &new_path)
                    .map_err(|e| format!("重命名文件 {} 失败: {}", file_name, e))?;
            }
        }
    }

    Ok(())
}

/// 删除素材的所有工作流版本（包括 nextcloud）
///
/// 扫描 00_original / 01_scale/**/ / 02_done/**/ / nextcloud/<task>/ 中
/// 所有以 base_name 开头的文件或目录，全部删除。
#[tauri::command]
pub fn delete_material(
    task_path: String,
    base_name: String,
    material_type: String,
) -> Result<(), String> {
    let task_dir = Path::new(&task_path);
    let is_sequence = material_type == "sequence";

    let mut dirs_to_scan: Vec<std::path::PathBuf> = vec![task_dir.join("00_original")];

    let scale_dir = task_dir.join("01_scale");
    if scale_dir.exists() {
        if let Ok(entries) = fs::read_dir(&scale_dir) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    dirs_to_scan.push(e.path());
                }
            }
        }
    }

    let done_dir = task_dir.join("02_done");
    if done_dir.exists() {
        if let Ok(entries) = fs::read_dir(&done_dir) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    dirs_to_scan.push(e.path());
                }
            }
        }
    }

    let nc_dir = task_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|vfx| vfx.join("nextcloud").join(task_dir.file_name().unwrap_or_default()));
    if let Some(ref nc) = nc_dir {
        if nc.exists() {
            dirs_to_scan.push(nc.clone());
        }
    }

    for dir in &dirs_to_scan {
        if !dir.exists() {
            continue;
        }
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if !file_name.starts_with(base_name.as_str()) {
                continue;
            }
            if is_sequence && path.is_dir() {
                fs::remove_dir_all(&path)
                    .map_err(|e| format!("删除目录 {} 失败: {}", file_name, e))?;
            } else if !path.is_dir() {
                fs::remove_file(&path)
                    .map_err(|e| format!("删除文件 {} 失败: {}", file_name, e))?;
            }
        }
    }

    Ok(())
}

/// 获取文件的修改时间戳（Unix 秒），供前端缓存失效判断用
#[tauri::command]
pub fn get_file_mtime(path: String) -> Result<u64, String> {
    fs::metadata(&path)
        .and_then(|m| m.modified())
        .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
        .map_err(|e| format!("获取文件信息失败: {}", e))
}

/// 读取文本文件内容（UTF-8），供前端侧边栏 TXT 预览用
#[tauri::command]
pub fn read_text_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| format!("读取文件失败: {}", e))
}

/// 设置项目的默认 AE 工程文件（写入 .pgb1_project.json）
#[tauri::command]
pub fn set_default_ae_file(project_path: String, file_name: Option<String>) -> Result<(), String> {
    let config_path = Path::new(&project_path).join(".pgb1_project.json");
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置文件失败: {}", e))?;
    let mut config: ProjectConfig = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置文件失败: {}", e))?;
    config.default_ae_file = file_name;
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化配置失败: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("写入配置文件失败: {}", e))?;
    Ok(())
}

/// 递归扫描目录，寻找 Unity 游戏启动程序
///
/// 策略：Unity 构建必然包含 `UnityCrashHandler64.exe`，与主 exe 在同一目录。
/// 递归遍历 root_dir，找到含该 Crash Handler 的那一层，返回同目录中另一个 .exe 的路径。
/// 不依赖文件夹名称，任意深度均可识别。
#[tauri::command]
pub fn find_unity_game_exe(root_dir: String) -> Result<Option<String>, String> {
    const CRASH_HANDLER: &str = "UnityCrashHandler64.exe";

    fn walk(dir: &Path, crash_handler_lower: &str) -> Option<PathBuf> {
        let entries: Vec<_> = match std::fs::read_dir(dir) {
            Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
            Err(_) => return None,
        };

        // 当前目录是否含 UnityCrashHandler64.exe（大小写不敏感）
        let has_crash_handler = entries.iter().any(|e| {
            e.file_name().to_string_lossy().to_lowercase() == crash_handler_lower
        });

        if has_crash_handler {
            // 同目录内找另一个 .exe
            for e in &entries {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if name.ends_with(".exe") && name != crash_handler_lower {
                    return Some(e.path());
                }
            }
        }

        // 递归进入子目录
        for e in &entries {
            if let Ok(ft) = e.file_type() {
                if ft.is_dir() {
                    if let Some(found) = walk(&e.path(), crash_handler_lower) {
                        return Some(found);
                    }
                }
            }
        }

        None
    }

    let root = Path::new(&root_dir);
    if !root.exists() {
        return Ok(None);
    }
    let crash_handler_lower = CRASH_HANDLER.to_lowercase();
    Ok(walk(root, &crash_handler_lower).map(|p| p.to_string_lossy().to_string()))
}

/// 用系统关联程序打开指定文件（如 .tps → TexturePacker）
#[tauri::command]
pub fn open_file(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("文件不存在: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let path_wide: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let verb: Vec<u16> = "open\0".encode_utf16().collect();
        let result = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(verb.as_ptr()),
                PCWSTR(path_wide.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            )
        };
        // ShellExecuteW 返回的 HINSTANCE，其整数值 > 32 表示成功
        if (result.0 as isize) <= 32 {
            return Err(format!("打开文件失败，错误码: {:?}", result.0));
        }
    }

    Ok(())
}

/// 修改序列帧的帧率：重命名 02_done/ 下所有 [an-XX-{old_fps}] 目录为 [an-XX-{new_fps}]
/// 目录内须含有以 base_name 开头的文件，以确认是同一素材（而非其他素材的同帧率版本）
#[tauri::command]
pub fn rename_sequence_fps(
    task_path: String,
    base_name: String,
    old_fps: u32,
    new_fps: u32,
) -> Result<(), String> {
    if old_fps == new_fps {
        return Ok(());
    }

    let done_dir = Path::new(&task_path).join("02_done");
    if !done_dir.exists() {
        return Ok(());
    }

    let old_suffix = format!("-{}]", old_fps);

    let entries = fs::read_dir(&done_dir)
        .map_err(|e| format!("读取 02_done 失败: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        // 目录名必须符合 [an-XX-{old_fps}] 格式
        if !dir_name.starts_with("[an-") || !dir_name.ends_with(old_suffix.as_str()) {
            continue;
        }
        // 目录内必须含有以 base_name 开头的文件，确认是同一素材
        let has_match = fs::read_dir(&path)
            .map(|rd| {
                rd.flatten().any(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| n.starts_with(base_name.as_str()))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        if !has_match {
            continue;
        }
        // 构造新目录名：替换末尾的 -{old_fps}] 为 -{new_fps}]
        let prefix = &dir_name[..dir_name.len() - old_suffix.len()];
        let new_dir_name = format!("{}-{}]", prefix, new_fps);
        let new_path = done_dir.join(&new_dir_name);
        if new_path.exists() {
            return Err(format!("目标目录已存在: {}", new_dir_name));
        }
        fs::rename(&path, &new_path)
            .map_err(|e| format!("重命名 {} → {} 失败: {}", dir_name, new_dir_name, e))?;
    }

    Ok(())
}

/// 扫描任务的 03_preview 目录，返回视频文件列表（含上传状态）
/// nextcloud_preview_path: nextcloud/preview/ 目录路径（含 breakdown 子目录）
#[tauri::command]
pub fn scan_preview_videos(
    task_path: String,
    nextcloud_preview_path: String,
) -> Result<Vec<PreviewVideoEntry>, String> {
    let preview_dir = Path::new(&task_path).join("03_preview");
    if !preview_dir.exists() {
        return Ok(Vec::new());
    }

    let video_exts: &[&str] = &["mp4", "mov", "avi", "mkv", "webm", "flv"];

    // 收集 nextcloud/preview/ 中的文件名（小写），用于状态判断
    let nc_preview = Path::new(&nextcloud_preview_path);
    let nc_files: std::collections::HashSet<String> = if nc_preview.exists() {
        fs::read_dir(nc_preview)
            .map(|rd| {
                rd.flatten()
                    .filter(|e| e.path().is_file())
                    .filter_map(|e| e.file_name().to_str().map(|s| s.to_lowercase()))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        std::collections::HashSet::new()
    };

    // 收集 nextcloud/preview/breakdown/ 中的文件名（小写）
    let nc_breakdown = nc_preview.join("breakdown");
    let nc_breakdown_files: std::collections::HashSet<String> = if nc_breakdown.exists() {
        fs::read_dir(&nc_breakdown)
            .map(|rd| {
                rd.flatten()
                    .filter(|e| e.path().is_file())
                    .filter_map(|e| e.file_name().to_str().map(|s| s.to_lowercase()))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        std::collections::HashSet::new()
    };

    let entries = fs::read_dir(&preview_dir)
        .map_err(|e| format!("无法读取 03_preview: {}", e))?;

    let mut files: Vec<PreviewVideoEntry> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.is_dir() {
                return None;
            }
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !video_exts.contains(&ext.as_str()) {
                return None;
            }
            let name = path.file_name()?.to_str()?.to_string();
            let name_lower = name.to_lowercase();
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

            // 判断上传状态：breakdown 文件查 nc_breakdown_files，否则查 nc_files
            let is_breakdown = name_lower
                .trim_end_matches(&format!(".{}", ext))
                .contains("_breakdown");
            let upload_status = if is_breakdown {
                if nc_breakdown_files.contains(&name_lower) {
                    "uploaded".to_string()
                } else {
                    // 检查是否存在同 baseName 的旧版本（待更新）
                    let base = name_lower.trim_end_matches(&format!(".{}", ext));
                    let base_no_ver = regex_strip_version(base);
                    let has_older = nc_breakdown_files.iter().any(|f| {
                        let f_base = f.trim_end_matches(&format!(".{}", ext));
                        regex_strip_version(f_base) == base_no_ver
                    });
                    if has_older { "outdated".to_string() } else { "none".to_string() }
                }
            } else if nc_files.contains(&name_lower) {
                "uploaded".to_string()
            } else {
                let base = name_lower.trim_end_matches(&format!(".{}", ext));
                let base_no_ver = regex_strip_version(base);
                let has_older = nc_files.iter().any(|f| {
                    let f_base = f.trim_end_matches(&format!(".{}", ext));
                    regex_strip_version(f_base) == base_no_ver
                });
                if has_older { "outdated".to_string() } else { "none".to_string() }
            };

            Some(PreviewVideoEntry {
                name,
                path: path.to_string_lossy().to_string(),
                extension: ext,
                size_bytes: size,
                upload_status,
            })
        })
        .collect();

    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

/// 从文件名提取版本号 (major, minor)，支持 _7 → (7,0) 和 _7.1 → (7,1)
/// 无版本返回 (0, 0)
fn extract_version_number(filename: &str) -> (u32, u32) {
    let stem = filename.rsplitn(2, '.').nth(1).unwrap_or(filename);
    let bytes = stem.as_bytes();
    let mut i = bytes.len();

    // 尾部数字
    while i > 0 && bytes[i - 1].is_ascii_digit() { i -= 1; }
    if i == bytes.len() { return (0, 0); }

    let last_start = i;
    let last_digits = &stem[last_start..];

    // 检查 .digits 模式（_major.minor）
    if i > 0 && bytes[i - 1] == b'.' {
        let dot_pos = i - 1;
        let mut j = dot_pos;
        while j > 0 && bytes[j - 1].is_ascii_digit() { j -= 1; }
        if j < dot_pos && j > 0 && bytes[j - 1] == b'_' {
            let major: u32 = stem[j..dot_pos].parse().unwrap_or(0);
            let minor: u32 = last_digits.parse().unwrap_or(0);
            return (major, minor);
        }
    }

    // 简单 _digits
    if last_start > 0 && bytes[last_start - 1] == b'_' {
        return (last_digits.parse().unwrap_or(0), 0);
    }

    (0, 0)
}

/// 去掉文件名末尾的版本号后缀（_01, _7.1 ...），用于判断是否同一视频的不同版本
fn regex_strip_version(name: &str) -> &str {
    let bytes = name.as_bytes();
    let mut i = bytes.len();

    // 1. 从末尾匹配数字
    while i > 0 && bytes[i - 1].is_ascii_digit() { i -= 1; }
    if i == bytes.len() { return name; } // 无尾部数字

    let minor_start = i;

    // 2. 检查 .digits 模式（子版本号 _major.minor）
    if i > 0 && bytes[i - 1] == b'.' {
        let dot_pos = i - 1;
        let mut j = dot_pos;
        while j > 0 && bytes[j - 1].is_ascii_digit() { j -= 1; }
        if j < dot_pos && j > 0 && bytes[j - 1] == b'_' {
            return &name[..j - 1]; // _major.minor 全部剥离
        }
    }

    // 3. 简单 _digits 模式
    if minor_start > 0 && bytes[minor_start - 1] == b'_' {
        return &name[..minor_start - 1];
    }

    name
}

/// 将选中的预览视频复制到 nextcloud/preview/（breakdown 的到 preview/breakdown/）
#[tauri::command]
pub fn copy_preview_to_nextcloud(
    file_path: String,
    nextcloud_preview_path: String,
) -> Result<(), String> {
    let src = Path::new(&file_path);
    if !src.exists() {
        return Err(format!("源文件不存在: {}", file_path));
    }

    let name = src
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "无法获取文件名".to_string())?;

    // 判断是否 breakdown（去扩展名后检查是否含 _breakdown）
    let name_no_ext = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let is_breakdown = name_no_ext.to_lowercase().contains("_breakdown");

    let nc_preview = Path::new(&nextcloud_preview_path);
    let dest_dir = if is_breakdown {
        nc_preview.join("breakdown")
    } else {
        nc_preview.to_path_buf()
    };

    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("创建目录失败: {}", e))?;

    // 删除同组旧版本：baseName 相同但文件名不同的旧文件
    let new_stem_lower = name_no_ext.to_lowercase();
    let new_base = regex_strip_version(&new_stem_lower);
    if let Ok(entries) = fs::read_dir(&dest_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() { continue; }
            let existing_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            // 跳过即将写入的同名文件
            if existing_name.eq_ignore_ascii_case(name) { continue; }
            let existing_stem = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if regex_strip_version(&existing_stem) == new_base {
                let _ = fs::remove_file(&path); // 静默删除旧版本
            }
        }
    }

    let dest = dest_dir.join(name);
    fs::copy(src, &dest)
        .map_err(|e| format!("复制文件失败: {}", e))?;

    Ok(())
}

/// 渲染 PSD/PSB 文件的合并图层，生成缩略图，返回 data URI（base64 JPEG）
/// max_size: 最长边像素上限（卡片用 256，侧边栏用 800）
/// 异步执行，CPU 密集部分放到阻塞线程池，不阻塞 Tauri 主线程
#[tauri::command]
pub async fn extract_psd_thumbnail(path: String, max_size: u32) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || {
        use image::{RgbaImage, imageops};

        let data = fs::read(&path).map_err(|e| format!("读取文件失败: {}", e))?;

        let psd = match psd::Psd::from_bytes(&data) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("PSD 解析失败 {}: {}", path, e);
                return Ok(None);
            }
        };

        let w = psd.width();
        let h = psd.height();
        if w == 0 || h == 0 {
            return Ok(None);
        }

        let rgba = psd.rgba();
        let img = RgbaImage::from_raw(w, h, rgba)
            .ok_or_else(|| "RGBA 数据长度不匹配".to_string())?;

        // resize 到最长边 max_size px（若原图更小则不放大）
        let thumb_max = max_size.max(1).min(w.max(h));
        let (thumb_w, thumb_h) = if w >= h {
            (thumb_max, (h as f32 * thumb_max as f32 / w as f32).round() as u32)
        } else {
            ((w as f32 * thumb_max as f32 / h as f32).round() as u32, thumb_max)
        };
        let thumb_w = thumb_w.max(1);
        let thumb_h = thumb_h.max(1);

        let thumb = imageops::resize(&img, thumb_w, thumb_h, imageops::FilterType::Triangle);

        // 编码为 JPEG（转为 RGB，JPEG 不支持 alpha）
        let rgb_thumb = image::DynamicImage::ImageRgba8(thumb).to_rgb8();
        let mut jpeg_buf: Vec<u8> = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buf, 85);
        encoder.encode_image(&rgb_thumb).map_err(|e| format!("JPEG 编码失败: {}", e))?;

        let b64 = BASE64.encode(&jpeg_buf);
        Ok(Some(format!("data:image/jpeg;base64,{}", b64)))
    })
    .await
    .map_err(|e| format!("线程执行失败: {}", e))?
}

/// 在指定目录下查找名字含 "appicon"（大小写不敏感）的文件
/// 优先返回 PNG，其次返回 PSD/PSB，都没有则返回 None
fn find_app_icon(preproduction_dir: &Path) -> Option<String> {
    let entries = fs::read_dir(preproduction_dir).ok()?;

    let mut png_candidate: Option<PathBuf> = None;
    let mut psd_candidate: Option<PathBuf> = None;

    for entry in entries.flatten() {
        let file_path = entry.path();
        if !file_path.is_file() {
            continue;
        }
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !file_name.contains("appicon") {
            continue;
        }

        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "png" => {
                if png_candidate.is_none() {
                    png_candidate = Some(file_path);
                }
            }
            "psd" | "psb" => {
                if psd_candidate.is_none() {
                    psd_candidate = Some(file_path);
                }
            }
            _ => {}
        }
    }

    png_candidate
        .or(psd_candidate)
        .map(|p| p.to_string_lossy().to_string())
}

/// 更新项目截止日期
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
    })
}
