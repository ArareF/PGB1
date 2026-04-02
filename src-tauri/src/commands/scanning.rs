use crate::models::{
    FileEntry, MaterialInfo, MaterialProgress, MaterialType, MaterialVersion,
    PreviewVideoEntry, ProjectInfo, TaskInfo,
};
use super::helpers::{
    calc_dir_size, count_preview_progress, count_upload_progress,
    find_app_icon, load_or_create_config, read_notes_file, regex_strip_version,
    scan_task_names,
};
use std::fs;
use std::path::Path;

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

        // 读取笔记文件
        let project_note = read_notes_file(&path)
            .get(&format!("card:{}", project_name.to_lowercase()))
            .cloned();

        projects.push(ProjectInfo {
            name: project_name,
            path: path.to_string_lossy().to_string(),
            deadline: config.deadline,
            default_ae_file: config.default_ae_file,
            app_icon,
            priority: config.priority,
            note: project_note,
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

    // 读取项目配置，获取任务优先度 Map
    let task_priorities: std::collections::HashMap<String, String> = {
        let config_path = project_dir.join(".pgb1_project.json");
        fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| {
                #[derive(serde::Deserialize, Default)]
                struct PriorityOnly {
                    #[serde(default)]
                    task_priorities: std::collections::HashMap<String, String>,
                }
                serde_json::from_str::<PriorityOnly>(&s).ok()
            })
            .map(|c| c.task_priorities)
            .unwrap_or_default()
    };

    // 读取笔记文件（一次读取，复用给所有任务）
    let notes_map = read_notes_file(project_dir);

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

        let priority = task_priorities.get(&task_name.to_lowercase()).cloned();
        let note = notes_map.get(&format!("card:{}", task_name.to_lowercase())).cloned();
        tasks.push(TaskInfo {
            name: task_name,
            path: path.to_string_lossy().to_string(),
            size_bytes,
            has_subtasks,
            material_total,
            material_uploaded,
            video_total,
            video_uploaded,
            priority,
            note,
        });
    }

    // 按任务名排序
    tasks.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(tasks)
}

/// 扫描指定目录，返回文件和子目录列表（非递归，只扫一层）
/// PSD/PSB 文件若磁盘缓存命中，thumbnail_path 直接填入路径，
/// 前端可用 loading="lazy" 渲染，与 PNG/JPG 行为完全一致
#[tauri::command]
pub fn scan_directory(app_handle: tauri::AppHandle, dir_path: String) -> Result<Vec<FileEntry>, String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use tauri::Manager;

    let dir = Path::new(&dir_path);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    // 预取一次缓存目录（所有 PSD/PSB 文件复用）
    let psd_cache_dir = app_handle.path().app_config_dir()
        .ok()
        .map(|d| d.join("psd_thumbnails"));

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
        // metadata 获取一次，size 和 mtime 共用
        let meta = if !is_dir { path.metadata().ok() } else { None };
        let size_bytes = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let extension = if is_dir {
            String::new()
        } else {
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase()
        };

        // PSD/PSB：检查 256px 缓存是否命中，hash 计算与 extract_psd_thumbnail 完全一致
        let thumbnail_path = if !is_dir && (extension == "psd" || extension == "psb") {
            psd_cache_dir.as_ref().and_then(|cache_dir| {
                let mtime = meta.as_ref()
                    .and_then(|m| m.modified().ok())
                    .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
                    .unwrap_or(0);
                let path_str = path.to_string_lossy().to_string();
                let mut hasher = DefaultHasher::new();
                path_str.hash(&mut hasher);
                mtime.hash(&mut hasher);
                256u32.hash(&mut hasher);
                let hash = hasher.finish();
                let cache_file = cache_dir.join(format!("{:016x}.jpg", hash));
                if cache_file.exists() {
                    Some(cache_file.to_string_lossy().to_string())
                } else {
                    None
                }
            })
        } else {
            None
        };

        entries.push(FileEntry {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir,
            size_bytes,
            extension,
            thumbnail_path,
        });
    }

    // 目录在前，文件在后；各自按名称排序
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));

    Ok(entries)
}

/// 预读目录结构缓存：避免每个素材重复扫描 01_scale/、02_done/、nextcloud/
/// key = 子目录名（如 "[an-70-30]"、"[img-70]"、"[70]"），"." 表示根目录本身
/// value = 该子目录下所有文件/目录的名称列表 + 大小
struct DirSnapshot {
    /// subdir_name -> Vec<(entry_name, size_bytes, is_file)>
    subdirs: std::collections::HashMap<String, Vec<(String, u64, bool)>>,
}

impl DirSnapshot {
    /// 一次性读取 dir 下所有子目录及其内容
    fn from_dir(dir: &Path) -> Self {
        let mut subdirs = std::collections::HashMap::new();
        if !dir.exists() {
            return Self { subdirs };
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let dir_name = entry.file_name().to_string_lossy().to_string();
                let children = Self::read_children(&path);
                subdirs.insert(dir_name, children);
            }
        }
        Self { subdirs }
    }

    /// 读取一个平面目录（如 nextcloud/TaskName/）的文件列表
    fn from_flat_dir(dir: &Path) -> Self {
        let mut subdirs = std::collections::HashMap::new();
        if !dir.exists() {
            return Self { subdirs };
        }
        let children = Self::read_children(dir);
        subdirs.insert(".".to_string(), children);
        Self { subdirs }
    }

    fn read_children(dir: &Path) -> Vec<(String, u64, bool)> {
        fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let is_file = e.path().is_file();
                let size = if is_file { e.metadata().map(|m| m.len()).unwrap_or(0) } else { 0 };
                (name, size, is_file)
            })
            .collect()
    }

    /// 在根目录（"."键）中查找以 base_name 开头的文件
    fn has_file_in_root(&self, base_name: &str) -> bool {
        self.subdirs.get(".")
            .map(|files| files.iter().any(|(n, _, _)| n.starts_with(base_name)))
            .unwrap_or(false)
    }

    /// 在子目录中查找以 base_name 开头的文件（匹配前缀过滤子目录名）
    fn has_file_in_subdirs(&self, base_name: &str, prefix: &str) -> bool {
        for (dir_name, files) in &self.subdirs {
            if prefix.is_empty() || dir_name.starts_with(&format!("[{}-", prefix)) {
                if files.iter().any(|(n, _, _)| n.starts_with(base_name)) {
                    return true;
                }
            }
        }
        false
    }

    /// 在子目录中查找以 base_name 开头的 .webp 文件
    fn has_webp_in_subdirs(&self, base_name: &str, prefix: &str) -> bool {
        for (dir_name, files) in &self.subdirs {
            if !dir_name.starts_with(&format!("[{}-", prefix)) {
                continue;
            }
            for (name, _, is_file) in files {
                if !is_file { continue; }
                if name.starts_with(base_name) {
                    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
                    if ext == "webp" {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 收集包含 base_name 文件的 [an-XX-YY] 子目录的 scale 值
    fn collect_seq_scales(&self, base_name: &str) -> Vec<u32> {
        let mut scales = Vec::new();
        for (dir_name, files) in &self.subdirs {
            if !dir_name.starts_with("[an-") || !dir_name.ends_with(']') {
                continue;
            }
            if files.iter().any(|(n, _, _)| n.starts_with(base_name)) {
                let inner = dir_name.trim_start_matches('[').trim_end_matches(']');
                let parts: Vec<&str> = inner.split('-').collect();
                if parts.len() >= 2 {
                    if let Ok(scale) = parts[1].parse::<u32>() {
                        scales.push(scale);
                    }
                }
            }
        }
        scales.sort();
        scales.dedup();
        scales
    }

    /// 收集包含 base_name 文件的 [XX] 子目录的 scale 值
    fn collect_img_scales(&self, base_name: &str) -> Vec<u32> {
        let mut scales = Vec::new();
        for (dir_name, files) in &self.subdirs {
            if !dir_name.starts_with('[') || !dir_name.ends_with(']') {
                continue;
            }
            if files.iter().any(|(n, _, _)| n.starts_with(base_name)) {
                let scale_str = dir_name.trim_start_matches('[').trim_end_matches(']');
                if let Ok(scale) = scale_str.parse::<u32>() {
                    scales.push(scale);
                }
            }
        }
        scales.sort();
        scales.dedup();
        scales
    }

    /// 从 [an-*] 子目录中提取 fps（找到第一个匹配 base_name 的子目录）
    fn extract_fps(&self, base_name: &str) -> Option<u32> {
        for (dir_name, files) in &self.subdirs {
            if !dir_name.starts_with("[an-") || !dir_name.ends_with(']') {
                continue;
            }
            if files.iter().any(|(n, _, _)| n.starts_with(base_name)) {
                let inner = dir_name.trim_start_matches('[').trim_end_matches(']');
                if let Some(fps_str) = inner.rsplitn(2, '-').next() {
                    if let Ok(fps) = fps_str.parse::<u32>() {
                        return Some(fps);
                    }
                }
            }
        }
        None
    }

    /// 计算 [an-*] 子目录中匹配 base_name 的文件总大小
    fn sum_seq_size(&self, base_name: &str) -> Option<u64> {
        let mut total: u64 = 0;
        let mut found = false;
        for (dir_name, files) in &self.subdirs {
            if !dir_name.starts_with("[an-") {
                continue;
            }
            for (name, size, is_file) in files {
                if *is_file && name.starts_with(base_name) {
                    total += size;
                    found = true;
                }
            }
        }
        if found { Some(total) } else { None }
    }

    /// 计算 [img-*] 子目录中匹配 base_name 的文件总大小
    fn sum_img_size(&self, base_name: &str) -> Option<u64> {
        let mut total: u64 = 0;
        let mut found = false;
        for (dir_name, files) in &self.subdirs {
            if !dir_name.starts_with("[img-") {
                continue;
            }
            for (name, size, is_file) in files {
                if *is_file && name.starts_with(base_name) {
                    total += size;
                    found = true;
                }
            }
        }
        if found { Some(total) } else { None }
    }
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
    let nextcloud_dir = task_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|vfx| vfx.join("nextcloud").join(task_dir.file_name().unwrap_or_default()));

    // ── 预读目录结构（一次性读取，后续查询走内存） ──
    let scale_cache = DirSnapshot::from_dir(&scale_dir);
    let done_cache = DirSnapshot::from_dir(&done_dir);
    let nc_cache = nextcloud_dir.as_ref()
        .map(|nc| DirSnapshot::from_flat_dir(nc))
        .unwrap_or_else(|| DirSnapshot { subdirs: std::collections::HashMap::new() });

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

        let progress = determine_progress_sequence_cached(&base_name, &done_cache, &nc_cache);
        let scales = done_cache.collect_seq_scales(&base_name);
        let fps = done_cache.extract_fps(&base_name);

        // 优先取 02_done 中精灵图三件套大小，回退到 00_original 目录大小
        let size_bytes = done_cache.sum_seq_size(&base_name)
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
            let progress = determine_progress_sequence_cached(&base_name, &done_cache, &nc_cache);
            let scales = done_cache.collect_seq_scales(&base_name);
            let fps = done_cache.extract_fps(&base_name);
            let size_bytes = done_cache.sum_seq_size(&base_name)
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
            determine_progress_image_cached(&base_name, &scale_cache, &done_cache, &nc_cache)
        } else {
            MaterialProgress::Original
        };

        let scales = if material_type == MaterialType::Image {
            scale_cache.collect_img_scales(&base_name)
        } else {
            Vec::new()
        };

        // 优先取 02_done 中的文件大小，回退到 00_original
        let size_bytes = if material_type == MaterialType::Image {
            done_cache.sum_img_size(&base_name)
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
                let scales = collect_scales_for_sequence(&base_name, &done_dir, Some(&sub_name));
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
                    collect_scales_for_image(&base_name, &scale_dir, Some(&sub_name))
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
    if done_dir.exists() && find_file_in_subdirs(done_dir, base_name, "img", Some(sub_name)) {
        return MaterialProgress::Done;
    }
    if scale_dir.exists() && find_file_in_subdirs(scale_dir, base_name, "", Some(sub_name)) {
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
        && find_webp_in_subdirs(done_dir, base_name, "an", Some(sub_name));
    let in_done_any = done_dir.exists()
        && find_file_in_subdirs(done_dir, base_name, "an", Some(sub_name));

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


fn collect_fps_for_sequence(base_name: &str, done_dir: &Path) -> Option<u32> {
    if !done_dir.exists() {
        return None;
    }
    if let Ok(entries) = fs::read_dir(done_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            // [an-XX-YY] 格式
            if !dir_name.starts_with("[an-") || !dir_name.ends_with(']') {
                continue;
            }
            // 检查目录内是否有以 base_name 开头的文件
            if let Ok(files) = fs::read_dir(&path) {
                let has_match = files.flatten().any(|f| {
                    f.file_name()
                        .to_str()
                        .map(|n| n.starts_with(base_name))
                        .unwrap_or(false)
                });
                if has_match {
                    // 解析 fps：[an-XX-YY] → YY
                    let inner = dir_name.trim_start_matches('[').trim_end_matches(']');
                    if let Some(fps_str) = inner.rsplitn(2, '-').next() {
                        if let Ok(fps) = fps_str.parse::<u32>() {
                            return Some(fps);
                        }
                    }
                }
            }
        }
    }
    None
}


fn collect_scales_for_sequence(base_name: &str, done_dir: &Path, sub_name: Option<&str>) -> Vec<u32> {
    let mut scales = Vec::new();
    if !done_dir.exists() {
        return scales;
    }
    if let Ok(entries) = fs::read_dir(done_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if !dir_name.starts_with("[an-") || !dir_name.ends_with(']') {
                continue;
            }
            let sub_dir = sub_name.map(|s| path.join(s));
            let search_dir = sub_dir.as_deref().unwrap_or(&path);
            if search_dir.exists() && find_file_in_dir(search_dir, base_name) {
                let inner = dir_name.trim_start_matches('[').trim_end_matches(']');
                let parts: Vec<&str> = inner.split('-').collect();
                if parts.len() >= 2 {
                    if let Ok(scale) = parts[1].parse::<u32>() {
                        scales.push(scale);
                    }
                }
            }
        }
    }
    scales.sort();
    scales.dedup();
    scales
}

fn collect_scales_for_image(base_name: &str, scale_dir: &Path, sub_name: Option<&str>) -> Vec<u32> {
    let mut scales = Vec::new();
    if !scale_dir.exists() {
        return scales;
    }
    if let Ok(entries) = fs::read_dir(scale_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            // [XX] 格式
            if !dir_name.starts_with('[') || !dir_name.ends_with(']') {
                continue;
            }
            let sub_dir = sub_name.map(|s| path.join(s));
            let search_dir = sub_dir.as_deref().unwrap_or(&path);
            if search_dir.exists() && find_file_in_dir(search_dir, base_name) {
                let scale_str = dir_name.trim_start_matches('[').trim_end_matches(']');
                if let Ok(scale) = scale_str.parse::<u32>() {
                    scales.push(scale);
                }
            }
        }
    }
    scales.sort();
    scales.dedup();
    scales
}



/// 缓存版：判定静帧进度（scan_materials 专用，避免重复 read_dir）
fn determine_progress_image_cached(
    base_name: &str,
    scale_cache: &DirSnapshot,
    done_cache: &DirSnapshot,
    nc_cache: &DirSnapshot,
) -> MaterialProgress {
    if nc_cache.has_file_in_root(base_name) {
        return MaterialProgress::Uploaded;
    }
    if done_cache.has_file_in_subdirs(base_name, "img") {
        return MaterialProgress::Done;
    }
    if scale_cache.has_file_in_subdirs(base_name, "") {
        return MaterialProgress::Scaled;
    }
    MaterialProgress::Original
}

/// 缓存版：判定序列帧进度（scan_materials 专用，避免重复 read_dir）
fn determine_progress_sequence_cached(
    base_name: &str,
    done_cache: &DirSnapshot,
    nc_cache: &DirSnapshot,
) -> MaterialProgress {
    let in_nextcloud = nc_cache.has_file_in_root(base_name);
    let in_done_webp = done_cache.has_webp_in_subdirs(base_name, "an");
    let in_done_any = done_cache.has_file_in_subdirs(base_name, "an");

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

fn find_file_in_dir(dir: &Path, base_name: &str) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if name.starts_with(base_name) {
                return true;
            }
        }
    }
    false
}

fn find_file_in_subdirs(dir: &Path, base_name: &str, prefix: &str, sub_name: Option<&str>) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if prefix.is_empty() || dir_name.starts_with(&format!("[{}-", prefix)) {
                let sub_dir = sub_name.map(|s| path.join(s));
                let search_dir = sub_dir.as_deref().unwrap_or(&path);
                if search_dir.exists() && find_file_in_dir(search_dir, base_name) {
                    return true;
                }
            }
        }
    }
    false
}

fn find_webp_in_subdirs(dir: &Path, base_name: &str, prefix: &str, sub_name: Option<&str>) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if !dir_name.starts_with(&format!("[{}-", prefix)) {
                continue;
            }
            let sub_dir = sub_name.map(|s| path.join(s));
            let search_dir = sub_dir.as_deref().unwrap_or(&path);
            if !search_dir.exists() {
                continue;
            }
            if let Ok(files) = fs::read_dir(search_dir) {
                for f in files.flatten() {
                    let fpath = f.path();
                    if !fpath.is_file() {
                        continue;
                    }
                    let fname = fpath.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let fext = fpath.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    if fext == "webp" && fname.starts_with(base_name) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

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

fn find_first_frame(dir: &Path) -> Option<String> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.path())
        .collect();
    files.sort();
    files.first().map(|p| p.to_string_lossy().to_string())
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

    // Prototype 素材 name 格式为 "subcat/basename"，需拆分后分别处理
    let (subcat, actual_base_name) = super::helpers::split_prototype_name(&base_name);

    // 00_original（Prototype：进入 subcat 子目录）
    let original_dir = task_dir.join("00_original");
    let original_search_dir = if subcat.is_empty() {
        original_dir
    } else {
        original_dir.join(&subcat)
    };
    if original_search_dir.exists() {
        collect_versions_flat(&original_search_dir, &actual_base_name, "00_original", "原始", "", &mut versions);
    }

    // 01_scale — 子目录 [100], [70], [50] 等
    let scale_dir = task_dir.join("01_scale");
    if scale_dir.exists() {
        collect_versions_in_scale_dirs(&scale_dir, &actual_base_name, &subcat, &mut versions);
    }

    // 02_done — 子目录 [img-XX] 或 [an-XX-YY]
    let done_dir = task_dir.join("02_done");
    let prefix = if material_type == "sequence" { "an" } else { "img" };
    if done_dir.exists() {
        collect_versions_in_done_dirs(&done_dir, &actual_base_name, prefix, &subcat, &mut versions);
    }

    // nextcloud（Prototype：进入 subcat 子目录）
    let nextcloud_dir = task_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|vfx| vfx.join("nextcloud").join(task_dir.file_name().unwrap_or_default()));
    if let Some(nc) = nextcloud_dir {
        let nc_search_dir = if subcat.is_empty() {
            nc
        } else {
            nc.join(&subcat)
        };
        if nc_search_dir.exists() {
            collect_versions_flat(&nc_search_dir, &actual_base_name, "nextcloud", "已上传", "", &mut versions);
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
    subcat: &str,
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
            // Prototype：在 [XX]/{subcat}/ 下查找；普通任务：直接在 [XX]/ 下查找
            let search_dir = if subcat.is_empty() {
                entry.path()
            } else {
                entry.path().join(subcat)
            };
            if search_dir.exists() {
                collect_versions_flat(
                    &search_dir,
                    base_name,
                    "01_scale",
                    "已缩放",
                    &scale,
                    versions,
                );
            }
        }
    }
}

fn collect_versions_in_done_dirs(
    done_dir: &Path,
    base_name: &str,
    prefix: &str,
    subcat: &str,
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
            // Prototype：在 [img-XX]/{subcat}/ 下查找；普通任务：直接在 [img-XX]/ 下查找
            let search_dir = if subcat.is_empty() {
                entry.path()
            } else {
                entry.path().join(subcat)
            };
            if search_dir.exists() {
                collect_versions_flat(
                    &search_dir,
                    base_name,
                    "02_done",
                    "已完成",
                    &scale,
                    versions,
                );
            }
        }
    }
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

/// 提取 PSD/PSB 缩略图，写入磁盘缓存并返回缓存文件路径（前端用 convertFileSrc 引用）
/// max_size: 最长边像素上限（卡片用 256，侧边栏用 800）
/// 使用信号量限制并发数（2），避免大量 PSD 同时解析导致线程池饱和
static PSD_SEMAPHORE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

#[tauri::command]
pub async fn extract_psd_thumbnail(
    app_handle: tauri::AppHandle,
    path: String,
    max_size: u32,
) -> Result<Option<String>, String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use tauri::Manager;

    // 用 mtime 做缓存失效判定（tokio::fs 异步版本，不阻塞 executor 线程）
    let mtime = tokio::fs::metadata(&path).await
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
        .unwrap_or(0);

    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    mtime.hash(&mut hasher);
    max_size.hash(&mut hasher);
    let hash = hasher.finish();

    let cache_dir = app_handle.path().app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {}", e))?
        .join("psd_thumbnails");
    let cache_file = cache_dir.join(format!("{:016x}.jpg", hash));

    // 磁盘缓存命中 → 直接返回路径（tokio::fs 异步检查，不阻塞 executor 线程）
    if tokio::fs::metadata(&cache_file).await.is_ok() {
        return Ok(Some(cache_file.to_string_lossy().to_string()));
    }

    let _permit = PSD_SEMAPHORE.acquire().await.map_err(|e| format!("信号量错误: {}", e))?;
    let cache_file_clone = cache_file.clone();
    let cache_dir_clone = cache_dir.clone();

    tokio::task::spawn_blocking(move || {
        let data = fs::read(&path).map_err(|e| format!("读取文件失败: {}", e))?;

        // 判断文件版本：1=PSD，2=PSB
        let is_psb = data.len() >= 6
            && &data[0..4] == b"8BPS"
            && u16::from_be_bytes([data[4], data[5]]) == 2;

        let jpeg_data = if is_psb {
            // PSB：psd crate 不支持，只能用内嵌 JPEG 缩略图
            extract_embedded_thumbnail(&data)
        } else {
            // PSD：优先图层合并（高质量），失败时 fallback 到内嵌 JPEG
            match extract_psd_via_layer_composite(&data, max_size, &path) {
                Ok(Some(jpeg)) => Some(jpeg),
                _ => extract_embedded_thumbnail(&data),
            }
        };

        match jpeg_data {
            Some(jpeg) => {
                // 写入磁盘缓存，前端用 convertFileSrc 引用
                fs::create_dir_all(&cache_dir_clone)
                    .map_err(|e| format!("创建缓存目录失败: {}", e))?;
                fs::write(&cache_file_clone, &jpeg)
                    .map_err(|e| format!("写入缓存文件失败: {}", e))?;
                Ok(Some(cache_file_clone.to_string_lossy().to_string()))
            }
            None => Ok(None),
        }
    })
    .await
    .map_err(|e| format!("线程执行失败: {}", e))?
}

/// 从 PSD/PSB 文件的 Image Resources 段提取内嵌 JPEG 缩略图
/// 资源 ID 0x040C（Photoshop 5.0+）或 0x0409（Photoshop 4.0）
fn extract_embedded_thumbnail(data: &[u8]) -> Option<Vec<u8>> {
    // 文件头固定 26 字节：4(sig) + 2(ver) + 6(reserved) + 2(ch) + 4(h) + 4(w) + 2(depth) + 2(mode)
    if data.len() < 26 {
        return None;
    }
    // 校验签名 "8BPS"
    if &data[0..4] != b"8BPS" {
        return None;
    }
    let version = u16::from_be_bytes([data[4], data[5]]);
    if version != 1 && version != 2 {
        return None;
    }

    let mut pos: usize = 26;

    // 跳过 Color Mode Data 段（4 字节长度 + 数据）
    if pos + 4 > data.len() { return None; }
    let cm_len = u32::from_be_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
    pos += 4 + cm_len;

    // Image Resources 段（4 字节长度 + 数据）
    if pos + 4 > data.len() { return None; }
    let ir_len = u32::from_be_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
    pos += 4;
    let ir_end = pos + ir_len;
    if ir_end > data.len() { return None; }

    // 遍历 Image Resource Block
    let mut safety = 0u32;
    while pos + 12 <= ir_end && safety < 500 {
        safety += 1;
        // 签名 "8BIM"
        if &data[pos..pos+4] != b"8BIM" { break; }
        let res_id = u16::from_be_bytes([data[pos+4], data[pos+5]]);
        // Pascal string（name）
        let name_len = data[pos+6] as usize;
        let padded_name_len = if (name_len + 1) % 2 != 0 { name_len + 2 } else { name_len + 1 };
        let data_len_offset = pos + 6 + padded_name_len;
        if data_len_offset + 4 > ir_end { break; }
        let block_data_len = u32::from_be_bytes([
            data[data_len_offset], data[data_len_offset+1],
            data[data_len_offset+2], data[data_len_offset+3],
        ]) as usize;
        let block_data_start = data_len_offset + 4;

        if (res_id == 0x040C || res_id == 0x0409) && block_data_len > 28 {
            // 缩略图资源头：4(format) + 4(w) + 4(h) + 4(widthbytes) + 4(totalsize) + 4(compressed_size) + 4(bpp) = 28 字节
            let jpeg_start = block_data_start + 28;
            let jpeg_end = block_data_start + block_data_len;
            if jpeg_end <= data.len() {
                let fmt = u32::from_be_bytes([
                    data[block_data_start], data[block_data_start+1],
                    data[block_data_start+2], data[block_data_start+3],
                ]);
                if fmt == 1 {
                    // format=1 表示 JPEG
                    let jpeg = data[jpeg_start..jpeg_end].to_vec();
                    if jpeg.len() >= 2 && jpeg[0] == 0xFF && jpeg[1] == 0xD8 {
                        return Some(jpeg);
                    }
                }
            }
        }

        // 跳到下一个 resource block（数据长度按偶数对齐）
        let padded_data_len = if block_data_len % 2 != 0 { block_data_len + 1 } else { block_data_len };
        pos = block_data_start + padded_data_len;
    }

    None
}

/// 使用 psd crate 合并图层生成缩略图 JPEG 字节（仅 PSD，PSB 不支持）
fn extract_psd_via_layer_composite(data: &[u8], max_size: u32, path: &str) -> Result<Option<Vec<u8>>, String> {
    use image::{RgbaImage, imageops};

    let psd = match psd::Psd::from_bytes(data) {
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

    let thumb_max = max_size.max(1).min(w.max(h));
    let (thumb_w, thumb_h) = if w >= h {
        (thumb_max, (h as f32 * thumb_max as f32 / w as f32).round() as u32)
    } else {
        ((w as f32 * thumb_max as f32 / h as f32).round() as u32, thumb_max)
    };
    let thumb_w = thumb_w.max(1);
    let thumb_h = thumb_h.max(1);

    let thumb = imageops::resize(&img, thumb_w, thumb_h, imageops::FilterType::Triangle);

    let rgb_thumb = image::DynamicImage::ImageRgba8(thumb).to_rgb8();
    let mut jpeg_buf: Vec<u8> = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buf, 85);
    encoder.encode_image(&rgb_thumb).map_err(|e| format!("JPEG 编码失败: {}", e))?;

    Ok(Some(jpeg_buf))
}
