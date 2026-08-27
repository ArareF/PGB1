use crate::models::{
    ConversionSequenceRequest, CopyMaterialRequest, CopyResult,
    DragMaterialRequest, ImportResult, NormalizeItem,
    NormalizeRequest, ScaleRequest, StartConversionRequest,
};
use crate::conversion::{ConversionState, ConversionSession, handle_file_event, bring_window_to_front};
use super::helpers::{split_prototype_name, copy_dir_recursive, is_sequence_stem, matches_base_name, read_not_sequence_list, PROTOTYPE_SUBCATEGORIES, regex_strip_version};
use std::collections::HashSet;
use super::workflow_paths::{
    an_dir_name, nextcloud_task_dir, stage_dir_prefix, DIR_DONE, DIR_NC_BREAKDOWN, DIR_NC_ORIGINAL,
    DIR_ORIGINAL, DIR_SCALE, STAGE_PREFIX_ANIM, STAGE_PREFIX_IMG,
};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
    let done_path = task_dir.join(DIR_DONE);
    let scale_dir = task_dir.join(DIR_SCALE);

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
        return Err(format!("{} 目录不存在，请先执行缩放操作", DIR_SCALE));
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
        cancel_flag: Arc::new(AtomicBool::new(false)),
        tp_child_pid: Arc::new(std::sync::Mutex::new(None)),
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
    let (done_path, cli_path, gui_path, fps_map, tp_scale, tp_webp_quality, cancel_flag, tp_child_pid) = {
        let state_lock = state.lock().map_err(|e| e.to_string())?;
        let session = state_lock.as_ref().ok_or("未启动转换会话")?;
        (
            session.done_path.clone(),
            session.texture_packer_cli.clone(),
            session.texture_packer_gui.clone(),
            session.sequence_fps_map.clone(),
            session.tp_scale,
            session.tp_webp_quality,
            session.cancel_flag.clone(),
            session.tp_child_pid.clone(),
        )
    };

    // 入口预清理：删除 done_path 根目录下匹配 sequences 名字的残留三件套
    // 防止上次中断遗留的 .tps/.webp/.plist 锁住新 CLI 写入（multipack 会产出 name-1/2/...）
    cleanup_residual_artifacts(&done_path, &sequences);

    let task_dir = done_path.parent().ok_or("无效的 done 路径")?;
    let original_dir = task_dir.join(DIR_ORIGINAL);

    for seq in sequences {
        // 取消检查点：每帧迭代前
        if cancel_flag.load(Ordering::SeqCst) {
            log::info!("[conversion] 用户取消，序列帧循环提前退出");
            return Ok(());
        }

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

        // 改 spawn + wait_with_output：spawn 后立即记录 pid，等 stop_conversion 能 taskkill
        // stdin piped + 写 "agree\n"：TP CLI 首次启动会要求接受 license，不喂 agree 会卡死或非零退出
        // 非首次时 CLI 不读 stdin，关闭无害
        let mut cli_child = cli_cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("CLI 启动失败: {}", e))?;

        if let Some(mut stdin) = cli_child.stdin.take() {
            let _ = stdin.write_all(b"agree\n");
            // stdin 在作用域结束 drop → CLI 看到 EOF，不会再等输入
        }

        let cli_pid = cli_child.id();
        if let Ok(mut g) = tp_child_pid.lock() { *g = Some(cli_pid); }

        let output = cli_child.wait_with_output().map_err(|e| format!("CLI 执行失败: {}", e))?;
        if let Ok(mut g) = tp_child_pid.lock() { *g = None; }

        if !output.status.success() {
            // 取消导致的 CLI 被 taskkill：友好退出，不报错
            if cancel_flag.load(Ordering::SeqCst) {
                log::info!("[conversion] CLI 被取消，序列帧循环退出");
                return Ok(());
            }
            return Err(format!("CLI 执行失败: {}", String::from_utf8_lossy(&output.stderr)));
        }

        // 2.5 将 .tps 中 globalSpriteSettings.scale 从默认 1 改为 0.5
        // 关键状态写回：读/写失败必须立即返回错误，否则 GUI 会打开未打补丁的 .tps，
        // 导致"配置值 / GUI 实际值 / 整理目录名"三者不一致（GPT P2-04）。
        let content = fs::read_to_string(&tps_path)
            .map_err(|e| format!("读取 .tps 预设失败 ({}): {}", tps_path.display(), e))?;
        let marker = "<key>globalSpriteSettings</key>";
        let patched = if let Some(pos) = content.find(marker) {
            let (before, after) = content.split_at(pos + marker.len());
            let after_patched = after.replacen("<double>1</double>", &format!("<double>{}</double>", tp_scale), 1);
            format!("{}{}", before, after_patched)
        } else {
            log::warn!("[tp] .tps 未找到 globalSpriteSettings 标记，跳过 scale 补丁: {}", tps_path.display());
            content
        };
        fs::write(&tps_path, patched)
            .map_err(|e| format!("写回 .tps 预设失败 ({}): {}", tps_path.display(), e))?;

        // 3. 启动 GUI 并置前
        if let Ok(mut child) = std::process::Command::new(&gui_path).arg(&tps_path).spawn() {
            let pid = child.id();
            if let Ok(mut g) = tp_child_pid.lock() { *g = Some(pid); }
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                bring_window_to_front(pid);
            });

            // 4. 等待 GUI 退出 (阻塞循环)
            let _ = child.wait();
            if let Ok(mut g) = tp_child_pid.lock() { *g = None; }
        } else {
            return Err(format!("无法启动 TexturePacker GUI: {}", gui_path.display()));
        }

        // 4.1 GUI 退出后追加取消检查点（用户在 GUI 阶段点取消会先 kill GUI，本帧不再整理）
        if cancel_flag.load(Ordering::SeqCst) {
            log::info!("[conversion] GUI 退出后检测到取消，跳过本帧整理");
            // 清理本帧未发布的 .tps，避免成为下次会话的残留
            if tps_path.exists() {
                if let Err(e) = fs::remove_file(&tps_path) {
                    log::warn!("[conversion] 取消清理 .tps 失败 {}: {}", tps_path.display(), e);
                }
            }
            return Ok(());
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
                if let Err(e) = fs::remove_file(&tps_path) {
                    log::warn!("[conversion] 清理失败的 .tps 预设失败 {}: {}", tps_path.display(), e);
                }
            }
            let _ = app_handle.emit("sequence-conversion-failed", name.clone());
            continue;
        }

        // 5. 解析 .tps 获取最终 scale
        let final_scale = parse_tps_scale(&tps_path)?;

        // 6. 整理三件套
        let target_dir_name = an_dir_name(final_scale, fps);
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
                    // .tps 从 02_done/ 根移入 [an-X-Y]/ 子目录后，TP 写入的 sprite 源相对路径
                    // 错位一级（少一个 ../），立即补偿，否则用户再用 GUI 打开会报"素材缺失"。
                    if ext == "tps" {
                        fix_tps_sprite_depth(&dest)?;
                    }
                }
            }
        }

        let _ = app_handle.emit("conversion-organized", name.clone());
    }

    Ok(())
}

/// 入口预清理：删除 done_path 根目录下匹配 sequences 名字的残留 .tps/.webp/.plist
///
/// 上次中断会留下未整理的三件套；TP CLI 用 `--save` 写 .tps 时如果文件被占用或损坏会失败。
/// 颗粒度：只删根目录下"name.ext"和"name-N.ext"（multipack 产物），不递归子目录（[an-X-Y]/ 是历史成果，保留）。
fn cleanup_residual_artifacts(done_path: &Path, sequences: &[ConversionSequenceRequest]) {
    let entries = match fs::read_dir(done_path) {
        Ok(e) => e,
        Err(_) => return,
    };

    let target_names: Vec<&str> = sequences.iter().map(|s| s.name.as_str()).collect();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() { continue; }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if !matches!(ext.as_str(), "tps" | "webp" | "plist") { continue; }

        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => continue,
        };

        // 判定残留：stem == name 或 stem == "{name}-{N}"（N 为纯数字，multipack 产物）
        let belongs = target_names.iter().any(|name| {
            if stem == *name { return true; }
            if let Some(suffix) = stem.strip_prefix(&format!("{}-", name)) {
                return !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit());
            }
            false
        });

        if belongs {
            if let Err(e) = fs::remove_file(&path) {
                log::warn!("[conversion] 入口预清理失败 {}: {}", path.display(), e);
            } else {
                log::info!("[conversion] 入口预清理: {}", path.display());
            }
        }
    }
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

// .tps 最终落在 02_done/[an-X-Y]/ 子目录，距 task_dir 两级，故 sprite 源（位于 task_dir/00_original/）
// 的正确相对前缀为 ../../00_original/。TP 在生成阶段把 .tps 暂存于 02_done/ 根目录，写入的是一级前缀
// ../00_original/；移动进 [an-X-Y]/ 后即错位一级。以下两个常量用于把一级前缀升级为两级。
// 锚定 <filename> 前缀：两级形态 "<filename>../../00_original/" 不含一级子串 "<filename>../00_original/"
// （前者第 11 字符起是 ".." 而非 "00"），故 replace 幂等，可安全重复调用。
const TPS_SPRITE_REF_DEPTH1: &str = "<filename>../00_original/";
const TPS_SPRITE_REF_DEPTH2: &str = "<filename>../../00_original/";

/// 修复 .tps 内 sprite 源路径深度：一级前缀 ../00_original/ 升级为两级 ../../00_original/。
/// 已是两级的形态不受影响（幂等）。返回是否发生了实际修改。读/写失败立即报错，不静默。
fn fix_tps_sprite_depth(tps_path: &Path) -> Result<bool, String> {
    let content = fs::read_to_string(tps_path)
        .map_err(|e| format!("读取 .tps 修复源路径失败 ({}): {}", tps_path.display(), e))?;
    if !content.contains(TPS_SPRITE_REF_DEPTH1) {
        return Ok(false);
    }
    let fixed = content.replace(TPS_SPRITE_REF_DEPTH1, TPS_SPRITE_REF_DEPTH2);
    fs::write(tps_path, &fixed)
        .map_err(|e| format!("写回 .tps 修复源路径失败 ({}): {}", tps_path.display(), e))?;
    log::info!("[conversion] 已修复 .tps sprite 源路径深度: {}", tps_path.display());
    Ok(true)
}

/// 「修改」序列帧工程文件（.tps）：阻塞打开 TexturePacker GUI，等其关闭后按新 scale 重整理。
///
/// 背景：.tps 的输出路径（textureFileName 空 → 由 data 名派生；plist 为相对文件名）都相对
/// .tps 自身所在目录。用户在 GUI 里改 scale 重新发布，webp/plist 会写回 [an-<旧scale>-<fps>]/，
/// 而目录名里的 scale 不会更新，导致扫描（按目录名解析 scale）仍归为旧尺寸。故 GUI 关闭后
/// 重解析 .tps 的 scale，若变化就把成品目录整体重命名到 [an-<新scale>-<fps>]。
///
/// 打开前先自愈 sprite 源路径深度（兼顾历史错位一级的坏 .tps）。
/// gui_path 为空则退回系统关联打开——无法等待，也就不做重整理，至少保证能打开。
#[tauri::command]
pub async fn edit_sequence_tps(tps_path: String, gui_path: String) -> Result<(), String> {
    let tps = Path::new(&tps_path);
    if !tps.exists() {
        return Err(format!("工程文件不存在: {}", tps_path));
    }
    if let Err(e) = fix_tps_sprite_depth(tps) {
        log::warn!("[edit_sequence_tps] 打开前修复 .tps 路径失败，继续打开: {}", e);
    }

    // 未配置 GUI 路径：退回系统关联打开（fire-and-forget，无法等待→不重整理）
    if gui_path.trim().is_empty() {
        return super::files::open_file(tps_path.clone());
    }

    // 成品目录名 [an-<scale>-<fps>]：留存旧名 + 取 fps，供关闭后判断 scale 是否变化。
    // 仅当位于 [an-...] 成品目录时才在关闭后重整理；历史散落 .tps → 只开不整理。
    let stage_dir = tps
        .parent()
        .ok_or_else(|| format!("无法定位工程文件所在目录: {}", tps_path))?;
    let old_dir_name = stage_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let an_prefix = stage_dir_prefix(STAGE_PREFIX_ANIM); // "[an-"
    let fps = if old_dir_name.starts_with(&an_prefix) && old_dir_name.ends_with(']') {
        old_dir_name
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split('-')
            .nth(2)
            .and_then(|s| s.parse::<u32>().ok())
    } else {
        None
    };

    // 阻塞打开 GUI 并等待关闭（沿用 execute_sequence_conversion 的模式，跑在异步线程不冻结 UI）
    let mut child = std::process::Command::new(&gui_path)
        .arg(&tps_path)
        .spawn()
        .map_err(|e| format!("无法启动 TexturePacker GUI ({}): {}", gui_path, e))?;
    let _ = child.wait();

    // 关闭后重整理：需在成品目录内且能取到 fps
    let fps = match fps {
        Some(f) => f,
        None => return Ok(()),
    };

    // 重解析最终 scale；目录名不变（尺寸没改）则无需整理
    let new_scale = parse_tps_scale(tps)?;
    let new_dir_name = an_dir_name(new_scale, fps);
    if new_dir_name == old_dir_name {
        return Ok(());
    }

    // 把成品目录整体重命名到新 scale 目录；目标已存在则报错（避免覆盖既有同尺寸成果）
    let done_dir = stage_dir
        .parent()
        .ok_or_else(|| "无法定位 02_done 目录".to_string())?;
    let new_dir = done_dir.join(&new_dir_name);
    if new_dir.exists() {
        return Err(format!(
            "已存在同尺寸成品目录「{}」，请先处理后再改尺寸",
            new_dir_name
        ));
    }
    fs::rename(stage_dir, &new_dir)
        .map_err(|e| format!("重命名成品目录 {} -> {} 失败: {}", old_dir_name, new_dir_name, e))?;
    log::info!(
        "[edit_sequence_tps] 尺寸变化，成品目录重命名: {} -> {}",
        old_dir_name, new_dir_name
    );

    Ok(())
}

/// 停止转换会话
///
/// 闭环动作（顺序敏感）：
/// 1. 置取消令牌（Arc 副本已 clone 到 execute_sequence_conversion 中，跨 take 仍有效）
/// 2. taskkill 当前 TP 子进程（CLI 或 GUI）→ 解除 wait 阻塞，循环回到取消检查点
/// 3. kill Imagine（原有逻辑）
/// 4. take session → drop watcher 停止监控
#[tauri::command]
pub fn stop_conversion(
    state: State<'_, ConversionState>,
) -> Result<(), String> {
    let mut state_lock = state.lock().map_err(|e| e.to_string())?;
    if let Some(session) = state_lock.take() {
        // 1. 置取消令牌
        session.cancel_flag.store(true, Ordering::SeqCst);

        // 2. taskkill 当前 TP 子进程，让循环里的 wait 立即解除
        if let Ok(g) = session.tp_child_pid.lock() {
            if let Some(pid) = *g {
                #[cfg(windows)]
                {
                    let _ = std::process::Command::new("taskkill")
                        .args(&["/F", "/PID", &pid.to_string()])
                        .spawn();
                }
            }
        }

        // 3. kill Imagine（原有逻辑）
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

        // 后端组装路径，避免前端拼接 Windows 分隔符
        let target_dir = Path::new(&req.task_path)
            .join(DIR_SCALE)
            .join(format!("[{}]", req.scale_percent));
        if !target_dir.exists() {
            fs::create_dir_all(&target_dir)
                .map_err(|e| format!("创建目标目录 {} 失败: {}", target_dir.display(), e))?;
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

// ─── Phase 5b+: 规范化独立页面（全量盘点 + 多操作执行）──────────────

/// 规范化原件备份目录名（隐藏目录，盘点时按 '.' 前缀跳过）
const NORMALIZE_BACKUP_DIR: &str = ".normalize_backup";

/// 盘点 00_original 全部素材（含已命名的），供规范化独立页面使用。
/// 与 preview_normalize 的区别：序列帧合并成一项，且已规范素材也会列出。
#[tauri::command]
pub fn scan_normalize_items(task_path: String) -> Result<Vec<NormalizeItem>, String> {
    let task_dir = Path::new(&task_path);
    let original_dir = task_dir.join(DIR_ORIGINAL);

    if !original_dir.exists() {
        return Ok(Vec::new());
    }

    let is_prototype = task_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_lowercase() == "prototype")
        .unwrap_or(false);

    // 用户手动标记的「非序列帧」基础名集合（task 级 00_original/非序列帧.txt，小写）
    let not_seq = read_not_sequence_list(&original_dir);

    let mut items = Vec::new();

    if is_prototype {
        for cat in &PROTOTYPE_SUBCATEGORIES {
            let sub_dir = original_dir.join(cat);
            if sub_dir.is_dir() {
                inventory_dir(&sub_dir, &mut items, &not_seq)?;
            }
        }
    } else {
        inventory_dir(&original_dir, &mut items, &not_seq)?;
    }

    items.sort_by(|a, b| a.base_name.cmp(&b.base_name));
    Ok(items)
}

/// 执行规范化（多操作版）。每个素材按"内容操作（裁切→黑底）→ 命名操作"的固定顺序处理。
/// `backup=true` 时，内容操作前把原件复制到同目录 `.normalize_backup/`（不覆盖已有备份）。
#[tauri::command]
pub fn execute_normalize_v2(
    app_handle: AppHandle,
    requests: Vec<NormalizeRequest>,
    backup: bool,
) -> Result<(), String> {
    let total = requests.len();

    for (index, req) in requests.into_iter().enumerate() {
        // ── 1. 内容操作：仅静帧 PNG，且勾选了裁切或黑底 ──
        if req.material_type == "static" && (req.do_trim || req.do_black_bg) {
            if let Some(p) = req.paths.first() {
                let path = Path::new(p);
                if path.exists() {
                    if backup {
                        // 按规范后名做备份 key：跨"去后缀改名"稳定，二次处理永不覆盖纯净原件
                        backup_original(path, &req.target_name)?;
                    }
                    apply_png_ops(path, req.do_trim, req.do_black_bg)?;
                }
            }
        }

        // ── 2. 命名操作 ──
        if req.do_rename {
            match req.material_type.as_str() {
                "static" => {
                    if let Some(p) = req.paths.first() {
                        let old = Path::new(p);
                        if old.exists() {
                            let new_path = old
                                .parent()
                                .ok_or_else(|| format!("无法获取父目录: {}", p))?
                                .join(&req.target_name);
                            if new_path != old {
                                fs::rename(old, &new_path).map_err(|e| {
                                    format!("重命名失败 ({} -> {}): {}", p, req.target_name, e)
                                })?;
                            }
                        }
                    }
                }
                "sequence" => {
                    let first = req
                        .paths
                        .first()
                        .ok_or_else(|| "序列帧路径为空".to_string())?;
                    let first_path = Path::new(first);
                    let parent = first_path
                        .parent()
                        .ok_or_else(|| format!("无法获取父目录: {}", first))?;

                    // 已在目标文件夹内（已规范序列帧）则跳过，避免 base/base 嵌套
                    let already_in_folder = parent
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n == req.target_name)
                        .unwrap_or(false);

                    if !already_in_folder {
                        let target_dir = parent.join(&req.target_name);
                        if !target_dir.exists() {
                            fs::create_dir_all(&target_dir).map_err(|e| {
                                format!("创建序列帧目录 {} 失败: {}", req.target_name, e)
                            })?;
                        }
                        for p in &req.paths {
                            let src = Path::new(p);
                            if !src.exists() {
                                continue;
                            }
                            let dest = target_dir.join(
                                src.file_name()
                                    .ok_or_else(|| format!("无法获取文件名: {}", p))?,
                            );
                            fs::rename(src, dest)
                                .map_err(|e| format!("移动帧 {} 失败: {}", p, e))?;
                        }
                    }
                }
                _ => {}
            }
        }

        let _ = app_handle.emit("normalize-progress", serde_json::json!({
            "current": index + 1,
            "total": total,
            "name": req.target_name,
        }));
    }

    Ok(())
}

/// 盘点单个目录：子目录视为已规范序列帧夹；松散文件先按序列帧特征分组，其余按独立静帧处理。
///
/// 序列帧判定与 `scan_materials` 同轴：编号必须紧跟混合模式（[`is_sequence_stem`]），
/// 避免把静帧变体（如 `..._seed_01/02`）误并为序列帧。`not_seq`（用户手动名簿）命中的组拆回静帧。
fn inventory_dir(
    dir: &Path,
    items: &mut Vec<NormalizeItem>,
    not_seq: &HashSet<String>,
) -> Result<(), String> {
    // 序列帧候选：seq_base -> 帧文件；其余松散文件按独立静帧逐个处理
    let mut seq: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut statics: Vec<PathBuf> = Vec::new();

    let entries = fs::read_dir(dir).map_err(|e| format!("读取目录失败: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') {
            continue; // 跳过隐藏项（含 .normalize_backup）
        }

        // 已规范的序列帧文件夹
        if path.is_dir() {
            let frames = collect_frame_files(&path);
            if frames.is_empty() {
                continue;
            }
            let ext = frames[0]
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            items.push(NormalizeItem {
                base_name: name.to_string(),
                material_type: "sequence".to_string(),
                ext,
                frame_count: frames.len() as u32,
                needs_rename: false,
                is_png: false,        // 两项新操作仅静帧，序列帧一律不可选
                is_add_or_screen: false,
                thumbnail_path: frames[0].to_string_lossy().to_string(),
                paths: frames.iter().map(|p| p.to_string_lossy().to_string()).collect(),
                target_name: name.to_string(),
                has_backup: false, // 序列帧不做内容操作，无备份
            });
            continue;
        }

        if !path.is_file() {
            continue;
        }

        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        // 序列帧特征：编号紧跟 add/screen/normal。否则按独立静帧处理。
        if let Some(seq_base) = is_sequence_stem(stem) {
            seq.entry(seq_base).or_default().push(path);
        } else {
            statics.push(path);
        }
    }

    // 序列帧候选组：多帧且未被手动排除 → 序列帧；否则（单帧/被排除）拆回静帧
    for (base, mut files) in seq {
        if files.len() > 1 && !not_seq.contains(&base.to_lowercase()) {
            files.sort();
            let ext = files[0]
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            items.push(NormalizeItem {
                base_name: base.clone(),
                material_type: "sequence".to_string(),
                ext,
                frame_count: files.len() as u32,
                needs_rename: true,
                is_png: false,
                is_add_or_screen: false,
                thumbnail_path: files[0].to_string_lossy().to_string(),
                paths: files.iter().map(|p| p.to_string_lossy().to_string()).collect(),
                target_name: base,
                has_backup: false, // 序列帧不做内容操作，无备份
            });
        } else {
            statics.extend(files);
        }
    }

    // 独立静帧：逐个处理（带 _NN 后缀者去后缀）
    for path in statics {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let (base, had_suffix) = split_base_suffix(stem);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_png = ext == "png";
        let target_name = format!("{}.{}", base, ext);
        // 备份按规范后名做 key（跨"去后缀改名"稳定，二次处理不会覆盖纯净原件）
        let has_backup = dir.join(NORMALIZE_BACKUP_DIR).join(&target_name).exists();
        items.push(NormalizeItem {
            base_name: base.clone(),
            material_type: "static".to_string(),
            ext: ext.clone(),
            frame_count: 1,
            needs_rename: had_suffix, // 带 _NN 后缀才需去后缀
            is_png,
            is_add_or_screen: is_png && base_is_add_or_screen(&base),
            thumbnail_path: path.to_string_lossy().to_string(),
            paths: vec![path.to_string_lossy().to_string()],
            target_name,
            has_backup,
        });
    }

    Ok(())
}

/// 拆出基础名与"是否带纯数字帧号后缀"。`main_a_01` -> ("main_a", true)；`main_a` -> ("main_a", false)
fn split_base_suffix(stem: &str) -> (String, bool) {
    if let Some(pos) = stem.rfind('_') {
        let suffix = &stem[pos + 1..];
        if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
            return (stem[..pos].to_string(), true);
        }
    }
    (stem.to_string(), false)
}

/// base 按 '_' 切分后，任一段等于 add 或 screen（区分 address/screenshot 等误伤）
fn base_is_add_or_screen(base: &str) -> bool {
    base.split('_').any(|seg| seg == "add" || seg == "screen")
}

/// 收集序列帧文件夹内的帧文件（非隐藏、是文件），按名排序
fn collect_frame_files(dir: &Path) -> Vec<PathBuf> {
    let mut frames: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || !p.is_file() {
                continue;
            }
            frames.push(p);
        }
    }
    frames.sort();
    frames
}

/// 执行前把原件复制到同目录 `.normalize_backup/{backup_name}`。
/// 以规范后名（target_name）为 key，且已存在备份则不覆盖——保证永远保留最早的纯净原件，
/// 即使同一素材后续多次处理（先裁切、再加黑底）也不会把中间态当原件。
fn backup_original(path: &Path, backup_name: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法获取父目录: {}", path.display()))?;
    let backup_dir = parent.join(NORMALIZE_BACKUP_DIR);
    if !backup_dir.exists() {
        fs::create_dir_all(&backup_dir)
            .map_err(|e| format!("创建备份目录失败: {}", e))?;
    }
    let dest = backup_dir.join(backup_name);
    if !dest.exists() {
        fs::copy(path, &dest)
            .map_err(|e| format!("备份 {} 失败: {}", path.display(), e))?;
    }
    Ok(())
}

/// 从备份恢复纯净原件。`current_path` 为当前文件路径（可能已改名/已处理），
/// `backup_name` 为规范后名（备份文件 key）。恢复后原文件被纯净原件覆盖。
#[tauri::command]
pub fn restore_normalize_backup(current_path: String, backup_name: String) -> Result<(), String> {
    let current = Path::new(&current_path);
    let parent = current
        .parent()
        .ok_or_else(|| format!("无法获取父目录: {}", current_path))?;
    let backup = parent.join(NORMALIZE_BACKUP_DIR).join(&backup_name);
    if !backup.exists() {
        return Err(format!("备份不存在: {}", backup.display()));
    }
    fs::copy(&backup, current)
        .map_err(|e| format!("恢复 {} 失败: {}", backup_name, e))?;
    // 恢复后删除备份：备份已完成使命，盘点时 has_backup 转 false，「恢复」按钮随之消失
    fs::remove_file(&backup)
        .map_err(|e| format!("删除备份 {} 失败: {}", backup_name, e))?;
    Ok(())
}

/// 对静帧 PNG 应用裁切 / 加黑底，结果原地写回
fn apply_png_ops(path: &Path, do_trim: bool, do_black_bg: bool) -> Result<(), String> {
    let img = image::open(path)
        .map_err(|e| format!("无法打开 PNG {}: {}", path.display(), e))?;
    let mut rgba = img.to_rgba8();

    if do_trim {
        if let Some(cropped) = trim_transparent(&rgba) {
            rgba = cropped;
        }
    }
    if do_black_bg {
        rgba = composite_on_black(&rgba);
    }

    rgba.save(path)
        .map_err(|e| format!("保存 PNG {} 失败: {}", path.display(), e))?;
    Ok(())
}

/// 裁掉四周完全透明的区域，返回裁切后的图。全透明或无可裁时返回 None（不改动）。
fn trim_transparent(img: &image::RgbaImage) -> Option<image::RgbaImage> {
    let (w, h) = img.dimensions();
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, h, 0u32, 0u32);
    let mut found = false;

    for y in 0..h {
        for x in 0..w {
            if img.get_pixel(x, y)[3] != 0 {
                found = true;
                if x < min_x { min_x = x; }
                if y < min_y { min_y = y; }
                if x > max_x { max_x = x; }
                if y > max_y { max_y = y; }
            }
        }
    }

    if !found {
        return None; // 全透明，保持原样
    }
    let cw = max_x - min_x + 1;
    let ch = max_y - min_y + 1;
    if cw == w && ch == h {
        return None; // 没有可裁的透明边
    }
    Some(image::imageops::crop_imm(img, min_x, min_y, cw, ch).to_image())
}

/// 把图合成到纯黑底上（add/screen 混合模式下黑色不贡献）。out_rgb = src_rgb × src_alpha，alpha 置 255。
fn composite_on_black(img: &image::RgbaImage) -> image::RgbaImage {
    let (w, h) = img.dimensions();
    let mut out = image::RgbaImage::new(w, h);
    for (x, y, px) in img.enumerate_pixels() {
        let a = px[3] as u32;
        let r = (px[0] as u32 * a / 255) as u8;
        let g = (px[1] as u32 * a / 255) as u8;
        let b = (px[2] as u32 * a / 255) as u8;
        out.put_pixel(x, y, image::Rgba([r, g, b, 255]));
    }
    out
}

// ─── 拖拽上传 ─────────────────────────────────────────────

/// 收集素材的拖拽文件列表
#[tauri::command]
pub fn collect_drag_files(
    task_path: String,
    materials: Vec<DragMaterialRequest>,
) -> Result<Vec<String>, String> {
    let task_dir = Path::new(&task_path);
    let done_dir = task_dir.join(DIR_DONE);
    let scale_dir = task_dir.join(DIR_SCALE);
    let original_dir = task_dir.join(DIR_ORIGINAL);

    let is_prototype = task_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_lowercase() == "prototype")
        .unwrap_or(false);

    let mut file_paths = Vec::new();

    for mat in &materials {
        // Prototype 素材名为 "subcat/basename"，拆出子分类；普通任务无子分类
        let (sub_name, base_name) = if is_prototype {
            let (s, b) = split_prototype_name(&mat.name);
            (Some(s), b)
        } else {
            (None, mat.name.clone())
        };
        let paths = collect_best_files(
            &base_name,
            &mat.material_type,
            sub_name.as_deref().filter(|s| !s.is_empty()),
            &original_dir,
            &scale_dir,
            &done_dir,
        );
        file_paths.extend(paths);
    }

    Ok(file_paths)
}

/// 收集素材的最佳拖拽文件（优先级：02_done > 01_scale > 00_original）。
/// sub_name = Prototype 子分类（None = 普通任务，各阶段目录直接查；Some = 深入一层子分类）
fn collect_best_files(
    base_name: &str,
    material_type: &str,
    sub_name: Option<&str>,
    original_dir: &Path,
    scale_dir: &Path,
    done_dir: &Path,
) -> Vec<String> {
    // Prototype 的 00_original 素材在子分类一层之下
    let original_search = match sub_name {
        Some(s) => original_dir.join(s),
        None => original_dir.to_path_buf(),
    };

    if material_type == "image" {
        if done_dir.exists() {
            let files = collect_matching_files_in_subdirs(done_dir, base_name, STAGE_PREFIX_IMG, sub_name);
            if !files.is_empty() { return files; }
        }
        if scale_dir.exists() {
            let files = collect_matching_files_in_subdirs(scale_dir, base_name, "", sub_name);
            if !files.is_empty() { return files; }
        }
        collect_matching_files_flat(&original_search, base_name)
    } else if material_type == "sequence" {
        if done_dir.exists() {
            let files = collect_matching_files_in_subdirs(done_dir, base_name, STAGE_PREFIX_ANIM, sub_name);
            if !files.is_empty() { return files; }
        }
        let seq_dir = original_search.join(base_name);
        if seq_dir.is_dir() {
            return collect_all_files_in_dir(&seq_dir);
        }
        collect_scattered_sequence_files(&original_search, base_name)
    } else {
        collect_matching_files_flat(&original_search, base_name)
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

/// 在各 `[...]` 阶段子目录中收集匹配 base_name 的文件（排除 .tps）。
/// prefix 过滤子目录名（"img"/"an"，空 = 任意 `[` 开头）；
/// sub_name = Prototype 子分类（Some 时深入一层）
fn collect_matching_files_in_subdirs(
    dir: &Path, base_name: &str, prefix: &str, sub_name: Option<&str>,
) -> Vec<String> {
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() { continue; }
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !prefix.is_empty() && !dir_name.starts_with(&format!("[{}-", prefix)) { continue; }
            if prefix.is_empty() && !dir_name.starts_with('[') { continue; }
            let search_dir = match sub_name {
                Some(s) => {
                    let p = path.join(s);
                    if !p.exists() { continue; }
                    p
                }
                None => path,
            };
            if let Ok(inner) = fs::read_dir(&search_dir) {
                for f in inner.flatten() {
                    let fp = f.path();
                    if !fp.is_file() { continue; }
                    let name = fp.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !matches_base_name(name, base_name) { continue; }
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

/// 将已规范化的序列帧目录复制到 nextcloud/original/{base_name}/。
/// 返回实际复制的帧文件数；空目录不能作为“已上传”事实来源。
fn copy_spine_sequence_original(
    source_dir: &Path,
    original_dest_dir: &Path,
    base_name: &str,
) -> Result<u32, String> {
    if !source_dir.is_dir() {
        return Err(format!("序列帧原件目录不存在: {}", source_dir.display()));
    }
    let entries = fs::read_dir(source_dir)
        .map_err(|e| format!("读取序列帧原件目录失败: {}", e))?;
    let mut frame_count = 0u32;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取序列帧条目失败: {}", e))?;
        if entry.path().is_file() {
            frame_count += 1;
        }
    }
    if frame_count == 0 {
        return Err("序列帧原件目录为空".to_string());
    }

    let destination = original_dest_dir.join(base_name);
    if destination.exists() {
        return Err(format!("Spine 目标目录已存在，请刷新后重试: {}", destination.display()));
    }
    if let Err(error) = copy_dir_recursive(source_dir, &destination) {
        if destination.exists() {
            fs::remove_dir_all(&destination)
                .map_err(|cleanup| format!("{}；清理半成品目录失败: {}", error, cleanup))?;
        }
        return Err(error);
    }
    Ok(frame_count)
}

/// Spine 直传兼容两种 00_original 状态：优先复制已规范化目录，
/// 若目录尚未生成，则把已识别的散落帧归入目标端同名目录。
fn copy_spine_sequence_from_original(
    original_search_dir: &Path,
    original_dest_dir: &Path,
    base_name: &str,
) -> Result<u32, String> {
    let sequence_dir = original_search_dir.join(base_name);
    if sequence_dir.is_dir() {
        return copy_spine_sequence_original(&sequence_dir, original_dest_dir, base_name);
    }

    let scattered_frames = collect_scattered_sequence_files(original_search_dir, base_name);
    if scattered_frames.is_empty() {
        return Err("00_original 中未找到对应序列帧".to_string());
    }
    let destination = original_dest_dir.join(base_name);
    if destination.exists() {
        return Err(format!("Spine 目标目录已存在，请刷新后重试: {}", destination.display()));
    }
    fs::create_dir_all(&destination)
        .map_err(|e| format!("创建 Spine 序列帧目录失败: {}", e))?;
    for source in &scattered_frames {
        let source_path = Path::new(source);
        let file_name = source_path.file_name().and_then(|name| name.to_str()).unwrap_or("");
        if let Err(error) = fs::copy(source_path, destination.join(file_name)) {
            fs::remove_dir_all(&destination)
                .map_err(|cleanup| format!("复制 Spine 序列帧 {} 失败: {}；清理半成品目录失败: {}", file_name, error, cleanup))?;
            return Err(format!("复制 Spine 序列帧 {} 失败: {}", file_name, error));
        }
    }
    Ok(scattered_frames.len() as u32)
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
    let done_dir = task_dir.join(DIR_DONE);
    let scale_dir = task_dir.join(DIR_SCALE);
    let original_dir = task_dir.join(DIR_ORIGINAL);

    let nextcloud_dir = nextcloud_task_dir(task_dir)
        .ok_or_else(|| "无法推导 nextcloud 路径".to_string())?;

    fs::create_dir_all(&nextcloud_dir)
        .map_err(|e| format!("创建 nextcloud 目录失败: {}", e))?;

    let is_prototype = task_name.to_lowercase() == "prototype";
    let mut copied_count = 0u32;
    let mut errors: Vec<String> = Vec::new();

    for mat in &material_names {
        let result = if is_prototype {
            let (sub_name, base_name) = split_prototype_name(&mat.name);
            copy_material_prototype(&base_name, &mat.material_type, &sub_name, &done_dir, &scale_dir, &original_dir, &nextcloud_dir)
        } else {
            copy_material_normal(&mat.name, &mat.material_type, &done_dir, &original_dir, &nextcloud_dir)
        };

        match result {
            Ok(count) => copied_count += count,
            Err(e) => errors.push(format!("{}: {}", mat.name, e)),
        }
    }

    Ok(CopyResult { copied_count, errors })
}

fn copy_material_normal(base_name: &str, material_type: &str, done_dir: &Path, original_dir: &Path, nextcloud_dir: &Path) -> Result<u32, String> {
    let prefix = if material_type == "sequence" { STAGE_PREFIX_ANIM } else { STAGE_PREFIX_IMG };

    // 正常交付物（02_done 产物）→ nextcloud 根目录
    let done_files = collect_matching_files_in_subdirs(done_dir, base_name, prefix, None);
    if !done_files.is_empty() {
        let mut count = 0u32;
        for src_path_str in &done_files {
            let src_path = Path::new(src_path_str);
            let file_name = src_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let dest = nextcloud_dir.join(file_name);
            fs::copy(src_path, &dest).map_err(|e| format!("复制失败 {}: {}", file_name, e))?;
            count += 1;
        }
        return Ok(count);
    }

    // Spine 原件直传兜底：02_done 无产物时，把 00_original 原件复制到
    // nextcloud/<任务>/original/，与 webp 交付物隔离。
    if material_type == "image" {
        let original_files = collect_matching_files_flat(original_dir, base_name);
        if !original_files.is_empty() {
            let original_dest_dir = nextcloud_dir.join(DIR_NC_ORIGINAL);
            fs::create_dir_all(&original_dest_dir).map_err(|e| format!("创建 original 目录失败: {}", e))?;
            let mut count = 0u32;
            for src_path_str in &original_files {
                let src_path = Path::new(src_path_str);
                let file_name = src_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let dest = original_dest_dir.join(file_name);
                fs::copy(src_path, &dest).map_err(|e| format!("复制原件失败 {}: {}", file_name, e))?;
                count += 1;
            }
            return Ok(count);
        }
    } else if material_type == "sequence" {
        let original_dest_dir = nextcloud_dir.join(DIR_NC_ORIGINAL);
        return copy_spine_sequence_from_original(original_dir, &original_dest_dir, base_name);
    }

    Err("02_done 与 00_original 中均未找到对应文件".to_string())
}

fn copy_material_prototype(
    base_name: &str, material_type: &str, sub_name: &str,
    done_dir: &Path, scale_dir: &Path, original_dir: &Path, nextcloud_dir: &Path,
) -> Result<u32, String> {
    let sub_dir = nextcloud_dir.join(sub_name);
    let original_sub_dir = sub_dir.join("_original");
    fs::create_dir_all(&sub_dir).map_err(|e| format!("创建子分类目录失败: {}", e))?;
    fs::create_dir_all(&original_sub_dir).map_err(|e| format!("创建 _original 目录失败: {}", e))?;

    let mut count = 0u32;
    let prefix = if material_type == "sequence" { STAGE_PREFIX_ANIM } else { STAGE_PREFIX_IMG };
    let done_files = collect_matching_files_in_subdirs(done_dir, base_name, prefix, Some(sub_name));
    for src_path_str in &done_files {
        let src_path = Path::new(src_path_str);
        let file_name = src_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let dest = sub_dir.join(file_name);
        fs::copy(src_path, &dest).map_err(|e| format!("复制 done 文件失败 {}: {}", file_name, e))?;
        count += 1;
    }

    let scale_files = collect_matching_files_in_subdirs(scale_dir, base_name, "", Some(sub_name));
    for src_path_str in &scale_files {
        let src_path = Path::new(src_path_str);
        let file_name = src_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let dest = original_sub_dir.join(file_name);
        fs::copy(src_path, &dest).map_err(|e| format!("复制 _original 文件失败 {}: {}", file_name, e))?;
        count += 1;
    }

    // Spine 原件直传兜底：done + scale 均无产物（仅原件场景）时，
    // 从 00_original/<sub>/ 复制原件到 nextcloud/<sub>/original/（方案 B：与交付物隔离，
    // determine_progress_prototype_img 会识别该 original/ 子目录判为已上传）。
    if count == 0 && material_type == "image" {
        let original_sub = original_dir.join(sub_name);
        let original_files = collect_matching_files_flat(&original_sub, base_name);
        if !original_files.is_empty() {
            let original_dest_dir = sub_dir.join(DIR_NC_ORIGINAL);
            fs::create_dir_all(&original_dest_dir).map_err(|e| format!("创建 original 目录失败: {}", e))?;
            for src_path_str in original_files {
                let src_path = Path::new(&src_path_str);
                let file_name = src_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let dest = original_dest_dir.join(file_name);
                fs::copy(src_path, &dest).map_err(|e| format!("复制原件失败 {}: {}", file_name, e))?;
                count += 1;
            }
        }
    } else if count == 0 && material_type == "sequence" {
        let original_search_dir = original_dir.join(sub_name);
        let original_dest_dir = sub_dir.join(DIR_NC_ORIGINAL);
        count += copy_spine_sequence_from_original(&original_search_dir, &original_dest_dir, base_name)?;
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

#[cfg(test)]
mod spine_upload_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_test_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("系统时间应晚于 Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("pgb1_{}_{}_{}", name, std::process::id(), nonce))
    }

    #[test]
    fn copy_spine_sequence_original_preserves_directory_and_counts_frames() {
        let root = temp_test_dir("spine_copy");
        let source = root.join("00_original").join("bonus_vfx_a_add");
        let original_dest = root.join("nextcloud").join("original");
        fs::create_dir_all(&source).expect("应能创建测试源目录");
        fs::write(source.join("bonus_vfx_a_add_01.png"), b"frame-1").expect("应能写入第一帧");
        fs::write(source.join("bonus_vfx_a_add_02.png"), b"frame-2").expect("应能写入第二帧");

        let copied = copy_spine_sequence_original(&source, &original_dest, "bonus_vfx_a_add")
            .expect("Spine 序列帧复制应成功");

        assert_eq!(copied, 2);
        assert!(original_dest.join("bonus_vfx_a_add/bonus_vfx_a_add_01.png").is_file());
        assert!(original_dest.join("bonus_vfx_a_add/bonus_vfx_a_add_02.png").is_file());

        fs::remove_dir_all(&root).expect("应能清理测试目录");
    }
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
    let dest_dir = if is_breakdown { nc_preview.join(DIR_NC_BREAKDOWN) } else { nc_preview.to_path_buf() };

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
                // latest-only 清理：失败会导致 nextcloud 同组残留旧版本（GPT P3-05）
                if let Err(e) = fs::remove_file(&path) {
                    log::warn!("[preview-upload] 清理旧版本失败 {}: {}", path.display(), e);
                }
            }
        }
    }

    let dest = dest_dir.join(name);
    fs::copy(src, &dest).map_err(|e| format!("复制文件失败: {}", e))?;

    Ok(())
}
