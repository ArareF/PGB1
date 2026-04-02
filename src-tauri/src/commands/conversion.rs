use crate::models::{
    ConversionSequenceRequest, CopyMaterialRequest, CopyResult,
    DragMaterialRequest, ImportResult, NormalizeActionType, NormalizePreviewItem,
    ScaleRequest, StartConversionRequest,
};
use crate::conversion::{ConversionState, ConversionSession, handle_file_event, bring_window_to_front};
use super::helpers::{split_prototype_name, copy_dir_recursive, PROTOTYPE_SUBCATEGORIES, regex_strip_version};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Runtime, State};
use notify::{Watcher, RecursiveMode, Event};

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
        tp_scale: request.tp_scale,
        tp_webp_quality: request.tp_webp_quality,
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
    let (done_path, cli_path, gui_path, fps_map, tp_scale, tp_webp_quality) = {
        let state_lock = state.lock().map_err(|e| e.to_string())?;
        let session = state_lock.as_ref().ok_or("未启动转换会话")?;
        (
            session.done_path.clone(),
            session.texture_packer_cli.clone(),
            session.texture_packer_gui.clone(),
            session.sequence_fps_map.clone(),
            session.tp_scale,
            session.tp_webp_quality,
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
            .arg("--webp-quality").arg(tp_webp_quality.to_string())
            .arg("--opt").arg(if name.to_lowercase().ends_with("normal") { "RGBA8888" } else { "RGB888" })
            .arg("--size-constraints").arg("AnySize")
            .arg("--scale").arg(tp_scale.to_string())
            .arg("--multipack")
            .arg("--save").arg(&tps_path);

        let output = cli_cmd.output().map_err(|e| format!("CLI 启动失败: {}", e))?;
        if !output.status.success() {
            return Err(format!("CLI 执行失败: {}", String::from_utf8_lossy(&output.stderr)));
        }

        // 2.5 将 .tps 中 globalSpriteSettings.scale 从默认 1 改为 0.5
        if let Ok(content) = fs::read_to_string(&tps_path) {
            let marker = "<key>globalSpriteSettings</key>";
            let patched = if let Some(pos) = content.find(marker) {
                let (before, after) = content.split_at(pos + marker.len());
                let after_patched = after.replacen("<double>1</double>", &format!("<double>{}</double>", tp_scale), 1);
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

        // 移动所有属于该素材的文件
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
                let belongs = matches!(ext, "webp" | "plist" | "tps")
                    && (stem == name || stem.starts_with(&format!("{}-", name)));
                if belongs {
                    let dest = target_dir.join(&fname);
                    fs::rename(&path, &dest).map_err(|e| format!("移动 {} 失败: {}", fname, e))?;
                }
            }
        }

        let _ = app_handle.emit("conversion-organized", name.clone());
    }

    Ok(())
}

/// 解析 .tps 获取最终 scale 百分比
fn parse_tps_scale(tps_path: &Path) -> Result<u32, String> {
    let content = fs::read_to_string(tps_path).map_err(|e| e.to_string())?;
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
        if let Some(pid) = session.imagine_pid {
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(&["/F", "/PID", &pid.to_string()])
                    .spawn();
            }
        }
    }
    Ok(())
}

// ─── Phase 5b: 规范化 (Normalization) ───────────────────────────────────

/// 预览规范化操作
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
        for cat in &PROTOTYPE_SUBCATEGORIES {
            let sub_dir = original_dir.join(cat);
            if sub_dir.is_dir() {
                scan_and_group_files(&sub_dir, &mut preview_items)?;
            }
        }
    } else {
        scan_and_group_files(&original_dir, &mut preview_items)?;
    }

    preview_items.sort_by(|a, b| a.original_name.cmp(&b.original_name));

    Ok(preview_items)
}

/// 执行规范化操作
#[tauri::command]
pub fn execute_normalize(items: Vec<NormalizePreviewItem>) -> Result<(), String> {
    for item in items {
        let old_path = Path::new(&item.original_path);
        if !old_path.exists() {
            continue;
        }

        match item.action_type {
            NormalizeActionType::Rename => {
                let new_path = old_path
                    .parent()
                    .ok_or_else(|| format!("无法获取父目录: {}", item.original_path))?
                    .join(&item.target_name);
                fs::rename(old_path, new_path)
                    .map_err(|e| format!("重命名失败 ({} -> {}): {}", item.original_name, item.target_name, e))?;
            }
            NormalizeActionType::MoveToFolder => {
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
#[tauri::command]
pub fn execute_scaling(app_handle: AppHandle, requests: Vec<ScaleRequest>) -> Result<(), String> {
    use image::GenericImageView;
    use image::codecs::jpeg::JpegEncoder;
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;
    use std::fs::File;
    use std::io::BufWriter;

    let total = requests.len();
    for (index, req) in requests.into_iter().enumerate() {
        let old_path = Path::new(&req.original_path);
        if !old_path.exists() {
            return Err(format!("原文件不存在: {}", req.original_path));
        }

        let target_dir = Path::new(&req.target_dir);
        if !target_dir.exists() {
            fs::create_dir_all(target_dir)
                .map_err(|e| format!("创建目标目录 {} 失败: {}", req.target_dir, e))?;
        }

        let img = image::open(old_path)
            .map_err(|e| format!("无法打开图像 {}: {}", req.original_path, e))?;

        let (width, height) = img.dimensions();
        let new_width = (width as f64 * (req.scale_percent as f64 / 100.0)).round() as u32;
        let new_height = (height as f64 * (req.scale_percent as f64 / 100.0)).round() as u32;

        let resized = img.resize(
            new_width,
            new_height,
            image::imageops::FilterType::Lanczos3,
        );

        let ext = old_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_lowercase();

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
                let mut encoder = JpegEncoder::new_with_quality(w, 100);
                encoder.encode_image(&resized).map_err(|e| format!("JPEG 编码失败: {}", e))?;
            },
            "png" => {
                let encoder = PngEncoder::new(w);
                let (w, h) = resized.dimensions();
                let color_type = resized.color();
                encoder.write_image(resized.as_bytes(), w, h, color_type.into())
                    .map_err(|e| format!("PNG 编码失败: {}", e))?;
            },
            _ => {
                resized.save(&dest_path)
                    .map_err(|e| format!("保存失败: {}", e))?;
            }
        }

        let _ = app_handle.emit("scaling-progress", serde_json::json!({
            "current": index + 1,
            "total": total,
            "name": req.base_name,
        }));
    }

    Ok(())
}

/// 扫描目录并按基础名分组文件，生成预览项
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

        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if let Some(pos) = stem.rfind('_') {
            let suffix = &stem[pos + 1..];
            if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                let base = stem[..pos].to_string();
                seq_groups.entry(base).or_default().push(path);
            }
        }
    }

    for (base_name, mut files) in seq_groups {
        if files.len() == 1 {
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

// ─── 拖拽上传 ─────────────────────────────────────────────

/// 收集素材的拖拽文件列表
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

fn collect_best_files(
    base_name: &str,
    material_type: &str,
    original_dir: &Path,
    scale_dir: &Path,
    done_dir: &Path,
) -> Vec<String> {
    if material_type == "image" {
        if done_dir.exists() {
            let files = collect_matching_files_in_subdirs(done_dir, base_name, "img");
            if !files.is_empty() { return files; }
        }
        if scale_dir.exists() {
            let files = collect_matching_files_in_subdirs(scale_dir, base_name, "");
            if !files.is_empty() { return files; }
        }
        collect_matching_files_flat(original_dir, base_name)
    } else if material_type == "sequence" {
        if done_dir.exists() {
            let files = collect_matching_files_in_subdirs(done_dir, base_name, "an");
            if !files.is_empty() { return files; }
        }
        let seq_dir = original_dir.join(base_name);
        if seq_dir.is_dir() {
            return collect_all_files_in_dir(&seq_dir);
        }
        collect_scattered_sequence_files(original_dir, base_name)
    } else {
        collect_matching_files_flat(original_dir, base_name)
    }
}

fn collect_best_files_prototype(
    base_name: &str,
    material_type: &str,
    sub_name: &str,
    original_dir: &Path,
    scale_dir: &Path,
    done_dir: &Path,
) -> Vec<String> {
    if material_type == "image" {
        if done_dir.exists() {
            let files = collect_matching_files_in_proto_subdirs(done_dir, base_name, sub_name, "img");
            if !files.is_empty() { return files; }
        }
        if scale_dir.exists() {
            let files = collect_matching_files_in_proto_subdirs(scale_dir, base_name, sub_name, "");
            if !files.is_empty() { return files; }
        }
        let sub_dir = original_dir.join(sub_name);
        collect_matching_files_flat(&sub_dir, base_name)
    } else if material_type == "sequence" {
        if done_dir.exists() {
            let files = collect_matching_files_in_proto_subdirs(done_dir, base_name, sub_name, "an");
            if !files.is_empty() { return files; }
        }
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

fn collect_matching_files_flat(dir: &Path, base_name: &str) -> Vec<String> {
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() { continue; }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let clean_stem = stem.strip_suffix("_01").unwrap_or(stem);
            if clean_stem == base_name {
                results.push(path.to_string_lossy().to_string());
            }
        }
    }
    results
}

fn collect_scattered_sequence_files(dir: &Path, base_name: &str) -> Vec<String> {
    let prefix = format!("{}_", base_name);
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() { continue; }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
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

fn collect_matching_files_in_subdirs(dir: &Path, base_name: &str, prefix: &str) -> Vec<String> {
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() { continue; }
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !prefix.is_empty() && !dir_name.starts_with(&format!("[{}-", prefix)) { continue; }
            if prefix.is_empty() && !dir_name.starts_with('[') { continue; }
            if let Ok(inner) = fs::read_dir(&path) {
                for f in inner.flatten() {
                    let fp = f.path();
                    if !fp.is_file() { continue; }
                    let name = fp.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !name.starts_with(base_name) { continue; }
                    let ext = fp.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    if ext == "tps" { continue; }
                    results.push(fp.to_string_lossy().to_string());
                }
            }
        }
    }
    results
}

fn collect_matching_files_in_proto_subdirs(
    dir: &Path, base_name: &str, sub_name: &str, prefix: &str,
) -> Vec<String> {
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() { continue; }
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !prefix.is_empty() && !dir_name.starts_with(&format!("[{}-", prefix)) { continue; }
            if prefix.is_empty() && !dir_name.starts_with('[') { continue; }
            let sub_dir = path.join(sub_name);
            if !sub_dir.exists() { continue; }
            if let Ok(inner) = fs::read_dir(&sub_dir) {
                for f in inner.flatten() {
                    let fp = f.path();
                    if !fp.is_file() { continue; }
                    let name = fp.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !name.starts_with(base_name) { continue; }
                    let ext = fp.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    if ext == "tps" { continue; }
                    results.push(fp.to_string_lossy().to_string());
                }
            }
        }
    }
    results
}

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

// ─── Nextcloud 复制 ────────────────────────────────────

/// 将选中素材从 02_done 复制到 nextcloud/
#[tauri::command]
pub fn copy_to_nextcloud(
    task_path: String,
    material_names: Vec<CopyMaterialRequest>,
) -> Result<CopyResult, String> {
    let task_dir = Path::new(&task_path);
    let task_name = task_dir.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
    let done_dir = task_dir.join("02_done");
    let scale_dir = task_dir.join("01_scale");

    let nextcloud_dir = task_dir
        .parent().and_then(|p| p.parent())
        .map(|vfx| vfx.join("nextcloud").join(task_name))
        .ok_or_else(|| "无法推导 nextcloud 路径".to_string())?;

    fs::create_dir_all(&nextcloud_dir)
        .map_err(|e| format!("创建 nextcloud 目录失败: {}", e))?;

    let is_prototype = task_name.to_lowercase() == "prototype";
    let mut copied_count = 0u32;
    let mut errors: Vec<String> = Vec::new();

    for mat in &material_names {
        let result = if is_prototype {
            let (sub_name, base_name) = split_prototype_name(&mat.name);
            copy_material_prototype(&base_name, &mat.material_type, &sub_name, &done_dir, &scale_dir, &nextcloud_dir)
        } else {
            copy_material_normal(&mat.name, &mat.material_type, &done_dir, &nextcloud_dir)
        };

        match result {
            Ok(count) => copied_count += count,
            Err(e) => errors.push(format!("{}: {}", mat.name, e)),
        }
    }

    Ok(CopyResult { copied_count, errors })
}

fn copy_material_normal(base_name: &str, material_type: &str, done_dir: &Path, nextcloud_dir: &Path) -> Result<u32, String> {
    let prefix = if material_type == "sequence" { "an" } else { "img" };
    let source_files = collect_matching_files_in_subdirs(done_dir, base_name, prefix);
    if source_files.is_empty() {
        return Err("02_done 中未找到对应文件".to_string());
    }
    let mut count = 0u32;
    for src_path_str in &source_files {
        let src_path = Path::new(src_path_str);
        let file_name = src_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let dest = nextcloud_dir.join(file_name);
        fs::copy(src_path, &dest).map_err(|e| format!("复制失败 {}: {}", file_name, e))?;
        count += 1;
    }
    Ok(count)
}

fn copy_material_prototype(
    base_name: &str, material_type: &str, sub_name: &str,
    done_dir: &Path, scale_dir: &Path, nextcloud_dir: &Path,
) -> Result<u32, String> {
    let sub_dir = nextcloud_dir.join(sub_name);
    let original_sub_dir = sub_dir.join("_original");
    fs::create_dir_all(&sub_dir).map_err(|e| format!("创建子分类目录失败: {}", e))?;
    fs::create_dir_all(&original_sub_dir).map_err(|e| format!("创建 _original 目录失败: {}", e))?;

    let mut count = 0u32;
    let prefix = if material_type == "sequence" { "an" } else { "img" };
    let done_files = collect_matching_files_in_proto_subdirs(done_dir, base_name, sub_name, prefix);
    for src_path_str in &done_files {
        let src_path = Path::new(src_path_str);
        let file_name = src_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let dest = sub_dir.join(file_name);
        fs::copy(src_path, &dest).map_err(|e| format!("复制 done 文件失败 {}: {}", file_name, e))?;
        count += 1;
    }

    let scale_files = collect_matching_files_in_proto_subdirs(scale_dir, base_name, sub_name, "");
    for src_path_str in &scale_files {
        let src_path = Path::new(src_path_str);
        let file_name = src_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let dest = original_sub_dir.join(file_name);
        fs::copy(src_path, &dest).map_err(|e| format!("复制 _original 文件失败 {}: {}", file_name, e))?;
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
            None => { errors.push(format!("无法获取文件名: {}", src)); continue; }
        };

        let dest = target.join(file_name);
        if dest.exists() { skipped += 1; continue; }

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

/// 将选中的预览视频复制到 nextcloud/preview/
#[tauri::command]
pub fn copy_preview_to_nextcloud(
    file_path: String,
    nextcloud_preview_path: String,
) -> Result<(), String> {
    let src = Path::new(&file_path);
    if !src.exists() {
        return Err(format!("源文件不存在: {}", file_path));
    }

    let name = src.file_name().and_then(|n| n.to_str())
        .ok_or_else(|| "无法获取文件名".to_string())?;

    let name_no_ext = src.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let is_breakdown = name_no_ext.to_lowercase().contains("_breakdown");

    let nc_preview = Path::new(&nextcloud_preview_path);
    let dest_dir = if is_breakdown { nc_preview.join("breakdown") } else { nc_preview.to_path_buf() };

    fs::create_dir_all(&dest_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    // 删除同组旧版本
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
            if existing_name.eq_ignore_ascii_case(name) { continue; }
            let existing_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            if regex_strip_version(&existing_stem) == new_base {
                let _ = fs::remove_file(&path);
            }
        }
    }

    let dest = dest_dir.join(name);
    fs::copy(src, &dest).map_err(|e| format!("复制文件失败: {}", e))?;

    Ok(())
}
