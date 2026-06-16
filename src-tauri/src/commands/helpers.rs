use super::workflow_paths::{export_dir, DIR_NC_BREAKDOWN};
use crate::models::{
    GlobalTask, GlobalTaskChild, GlobalTaskConfig, MaterialType, ProjectConfig,
};
use std::fs;
use std::path::Path;

/// 文件类型扩展名 SSOT（小写，与前端 src/config/fileTypes.ts 对齐）
pub(crate) const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "gif"];
pub(crate) const VIDEO_EXTS: &[&str] = &["mp4", "mov", "avi", "mkv", "webm", "flv"];
/// 序列帧帧文件允许的扩展名（不含 gif：帧文件必须是静态位图）
pub(crate) const FRAME_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp"];

/// 按扩展名（须已小写）判定素材类型
pub(crate) fn material_type_from_ext(ext: &str) -> MaterialType {
    if IMAGE_EXTS.contains(&ext) {
        MaterialType::Image
    } else if VIDEO_EXTS.contains(&ext) {
        MaterialType::Video
    } else {
        MaterialType::Other
    }
}

/// 精确匹配文件 stem 是否属于指定 base_name
/// 兼容 TexturePacker multipack 输出的 `name-0.webp`
pub(crate) fn matches_base_name(file_name: &str, base_name: &str) -> bool {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    stem == base_name || stem.starts_with(&format!("{base_name}-"))
}

/// 序列帧判定 SSOT：帧编号必须紧跟在这些混合模式 token 之后
/// （业务规则：真序列帧形如 `<base>_<混合模式>_<帧编号>`）
pub(crate) const SEQUENCE_BLEND_MODES: &[&str] = &["add", "screen", "normal"];

/// 判定 stem 是否为序列帧的某一帧；是则返回基础名（去掉末尾 `_<帧编号>`）。
///
/// 规则：剥掉末尾 `_<纯数字>` 后，新的末尾 token（小写）必须 ∈ [`SEQUENCE_BLEND_MODES`]。
/// 不依赖帧编号位数 —— 2 位 `_01` 与 0 补位 `_0001` 同等支持。
///
/// 示例：
///   - `bonus_vfx_a_screen_01`  → Some("bonus_vfx_a_screen")  （screen 后接编号）
///   - `main_vfx_a_add_seed_01`  → None  （编号前是补充名 seed，非混合模式）
pub(crate) fn is_sequence_stem(stem: &str) -> Option<String> {
    // 1. 末尾必须是 `_<纯数字>`
    let pos = stem.rfind('_')?;
    let suffix = &stem[pos + 1..];
    if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let base = &stem[..pos];
    // 2. base 的末尾 token 必须是混合模式
    let mode_token = base.rsplit('_').next().unwrap_or(base);
    if SEQUENCE_BLEND_MODES.contains(&mode_token.to_lowercase().as_str()) {
        Some(base.to_string())
    } else {
        None
    }
}

/// 手动「非序列帧」名簿文件名（置于任务 `00_original/` 下，显性可读、可手编辑）
pub(crate) const NOT_SEQUENCE_LIST_FILE: &str = "非序列帧.txt";

/// 「非序列帧」名簿的自我说明头（文件不存在时自动写入，引导用户手动撤回）
pub(crate) const NOT_SEQUENCE_LIST_HEADER: &str =
    "# 此文件列出被手动标记为「非序列帧」的素材基础名，每行一个。\n\
     # 删除对应行并刷新，即可恢复为序列帧识别。\n";

/// 读取 `00_original/非序列帧.txt`，返回被手动排除的基础名集合（小写）。
/// 防御性：文件不存在/读取失败 → 空集合；`#` 开头与空行忽略。
pub(crate) fn read_not_sequence_list(original_dir: &Path) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let content = match fs::read_to_string(original_dir.join(NOT_SEQUENCE_LIST_FILE)) {
        Ok(c) => c,
        Err(_) => return set,
    };
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        set.insert(trimmed.to_lowercase());
    }
    set
}

/// Windows 文件名非法字符（Win32 API 限制）
pub(crate) const WINDOWS_ILLEGAL_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Windows 保留字（不能用作文件名 stem，不区分大小写）
pub(crate) const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL",
    "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// 校验文件/目录名是否合法（Windows + 跨平台通用）
///
/// 检查顺序：
///   1. 不能为空（trim 后）
///   2. 不含 Windows 非法字符 `< > : " / \ | ? *`
///   3. 不含控制字符（\0~\x1F）
///   4. 不以点或空格结尾
///   5. stem 不是 Windows 保留字（CON / PRN / AUX / NUL / COM1~9 / LPT1~9）
///
/// 返回 trim 后的合法名称，或错误信息
pub(crate) fn validate_file_name<'a>(name: &'a str, kind: &str) -> Result<&'a str, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(format!("{}不能为空", kind));
    }
    if trimmed.chars().any(|c| WINDOWS_ILLEGAL_CHARS.contains(&c)) {
        return Err(format!(
            "{}包含非法字符，不能使用: {}",
            kind,
            WINDOWS_ILLEGAL_CHARS.iter().collect::<String>()
        ));
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(format!("{}包含控制字符", kind));
    }
    if trimmed.ends_with('.') || trimmed.ends_with(' ') {
        return Err(format!("{}不能以点或空格结尾", kind));
    }
    let upper = trimmed.to_uppercase();
    let stem = upper.split('.').next().unwrap_or(&upper);
    if WINDOWS_RESERVED_NAMES.contains(&stem) {
        return Err(format!("{}是 Windows 保留字: {}", kind, trimmed));
    }
    Ok(trimmed)
}

/// Prototype 下固定的 7 个子分类目录
pub(crate) const PROTOTYPE_SUBCATEGORIES: [&str; 7] = [
    "big_win",
    "infoboard",
    "loading_bonus",
    "main_ui",
    "spinbutton",
    "symbol",
    "total_win",
];

/// PSD/ 下固定的 8 个子目录（每个项目统一，与任务列表无关）
pub(crate) const PSD_SUBCATEGORIES: [&str; 8] = [
    "big_win",
    "feature_buy",
    "infoboard",
    "loading_bonus",
    "main_ui",
    "spin_button",
    "symbols",
    "total_win",
];

/// 递归计算目录大小（字节）
pub(crate) fn calc_dir_size(path: &Path) -> u64 {
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

/// 从文件名提取版本号 (major, minor)，支持 _7 → (7,0) 和 _7.1 → (7,1)
/// 无版本返回 (0, 0)
pub(crate) fn extract_version_number(filename: &str) -> (u32, u32) {
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
pub(crate) fn regex_strip_version(name: &str) -> &str {
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

/// 解析 Prototype 素材名（"subcategory/basename" → (subcategory, basename)）
pub(crate) fn split_prototype_name(name: &str) -> (String, String) {
    if let Some(pos) = name.find('/') {
        (name[..pos].to_string(), name[pos + 1..].to_string())
    } else {
        (String::new(), name.to_string())
    }
}

/// 读取目录下的 .pgb1_notes.json，防御性解析，文件不存在或格式错误返回空 Map
pub(crate) fn read_notes_file(dir: &Path) -> std::collections::HashMap<String, String> {
    let notes_path = dir.join(".pgb1_notes.json");
    fs::read_to_string(&notes_path)
        .ok()
        .and_then(|s| serde_json::from_str::<std::collections::HashMap<String, String>>(&s).ok())
        .unwrap_or_default()
}

/// 在指定目录下查找名字含 "appicon"（大小写不敏感）的文件
/// 优先返回 PNG，其次返回 PSD/PSB，都没有则返回 None
pub(crate) fn find_app_icon(preproduction_dir: &Path) -> Option<String> {
    let entries = fs::read_dir(preproduction_dir).ok()?;

    let mut png_candidate: Option<std::path::PathBuf> = None;
    let mut psd_candidate: Option<std::path::PathBuf> = None;

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

/// 读取或自动创建 .pgb1_project.json
pub(crate) fn load_or_create_config(project_path: &Path) -> Result<ProjectConfig, String> {
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

    let export_path = export_dir(project_path);
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
        priority: None,
        task_priorities: std::collections::HashMap::new(),
    };

    let json =
        serde_json::to_string_pretty(&config).map_err(|e| format!("序列化配置失败: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("写入配置文件失败: {}", e))?;

    Ok(config)
}

/// 原子更新 `.pgb1_project.json`：读 → 闭包修改 → 写回
///
/// 用于消除 `set_project_priority` / `set_task_priority` /
/// `set_default_ae_file` / `update_project_deadline` 四个命令里的重复模板：
/// 每个命令只差一行"在 config 上改字段"，其余读写代码完全一致。
pub(crate) fn mutate_project_config<F>(project_path: &Path, mutator: F) -> Result<(), String>
where
    F: FnOnce(&mut ProjectConfig),
{
    let config_path = project_path.join(".pgb1_project.json");
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let mut config: ProjectConfig = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置失败: {}", e))?;
    mutator(&mut config);
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(&config_path, json)
        .map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}

/// 扫描 Export 目录下的任务名称列表
pub(crate) fn scan_task_names(export_path: &Path) -> Result<Vec<String>, String> {
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

/// 收集目录第一层的名称（小写，文件去扩展名，目录保留名称）
pub(crate) fn collect_first_level_names(dir: &Path) -> std::collections::HashSet<String> {
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
pub(crate) fn collect_base_names(dir: &Path) -> std::collections::HashSet<String> {
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

/// 统计素材上传进度：(总数, 已上传数)
/// 普通任务：总数 = 00_original 第一层文件/目录数
/// Prototype：总数 = 00_original 递归所有文件数（子分类下的素材）
pub(crate) fn count_upload_progress(original_dir: &Path, nc_dir: &Path, is_prototype: bool) -> (u32, u32) {
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
pub(crate) fn count_preview_progress(preview_dir: &Path, nc_preview_dir: &Path) -> (u32, u32) {
    if !preview_dir.exists() {
        return (0, 0);
    }

    // 收集 03_preview/ 第一层视频文件名（小写，含扩展名），并标记是否为 breakdown
    let mut video_names: Vec<(String, bool)> = Vec::new();
    if let Ok(entries) = fs::read_dir(preview_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() { continue; }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if !VIDEO_EXTS.contains(&ext.as_str()) { continue; }
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
    let nc_breakdown = nc_preview_dir.join(DIR_NC_BREAKDOWN);
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

/// 默认的 8 个全局任务
pub(crate) fn default_global_tasks() -> GlobalTaskConfig {
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
pub(crate) fn to_title_case(s: &str) -> String {
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

/// 递归复制目录
///
/// Symlink / NTFS junction 防护（N-30）：
/// Windows NTFS junction 可能形成环（C:\foo -> C:\foo\bar -> C:\foo），会导致无限递归。
/// 我们用 symlink_metadata 识别 symlink，遇到就跳过并记 warn，不跟随。
pub(crate) fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    // 入口本身是 symlink 直接拒绝（防止调用方误把 junction 当目录传入）
    if src.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) {
        return Err(format!("拒绝复制 symlink 目录（可能形成环）: {}", src.display()));
    }

    fs::create_dir_all(dest).map_err(|e| format!("创建目录失败: {}", e))?;

    let entries = fs::read_dir(src).map_err(|e| format!("读取目录失败: {}", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取条目失败: {}", e))?;
        let entry_path = entry.path();
        let entry_dest = dest.join(entry.file_name());

        // 跳过 symlink（不跟随，不拷贝）—— 防 NTFS junction 形成环
        if entry_path.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) {
            log::warn!("[copy_dir_recursive] 跳过 symlink/junction: {}", entry_path.display());
            continue;
        }

        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &entry_dest)?;
        } else {
            fs::copy(&entry_path, &entry_dest).map_err(|e| format!("复制失败: {}", e))?;
        }
    }

    Ok(())
}

/// 移动目录：同卷优先走 `fs::rename`（瞬时且原子，大目录归档/恢复不再逐文件复制），
/// 跨卷或 rename 失败时回退「复制 + 删除原目录」
pub(crate) fn move_dir(src: &Path, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目标父目录失败: {}", e))?;
    }
    if fs::rename(src, dest).is_ok() {
        return Ok(());
    }
    copy_dir_recursive(src, dest)?;
    fs::remove_dir_all(src).map_err(|e| format!("删除原目录失败: {}", e))?;
    Ok(())
}
