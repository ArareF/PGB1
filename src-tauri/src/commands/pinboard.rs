use crate::models::{PinboardCanvas, PinboardData};
use std::fs;
use std::path::Path;
use tauri::{Emitter, Manager};

// ─── 贴图板独立窗口 ─────────────────────────────────────────

const PINBOARD_WINDOW_LABEL: &str = "pinboard";

/// 打开/聚焦贴图板独立窗口，并切换到指定标签
/// - 窗口已存在：emit 事件通知前端切换标签
/// - 窗口不存在：把初始标签数据编码到 URL query params，前端 mount 时读取
#[tauri::command]
pub async fn open_pinboard_window(
    app: tauri::AppHandle,
    dir_path: String,
    canvas_key: String,
    title: String,
) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(PINBOARD_WINDOW_LABEL) {
        // 窗口已存在，emit 事件让前端添加/切换标签
        let payload = serde_json::json!({
            "dirPath": dir_path,
            "canvasKey": canvas_key,
            "title": title,
        });
        let _ = win.emit("pinboard-open-tab", payload);
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }

    // 窗口不存在：创建新窗口，初始标签编码到 URL
    // spawn 到 async runtime 避免死锁（Tauri 2 铁律）
    tauri::async_runtime::spawn(async move {
        // 手动百分号编码（避免新增依赖）
        fn percent_encode(s: &str) -> String {
            let mut out = String::with_capacity(s.len() * 3);
            for b in s.bytes() {
                match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        out.push(b as char);
                    }
                    _ => {
                        out.push_str(&format!("%{:02X}", b));
                    }
                }
            }
            out
        }
        let encoded_dir = percent_encode(&dir_path);
        let encoded_key = percent_encode(&canvas_key);
        let encoded_title = percent_encode(&title);
        let path = format!(
            "/pinboard?dirPath={}&canvasKey={}&title={}",
            encoded_dir, encoded_key, encoded_title
        );
        let url = tauri::WebviewUrl::App(path.into());
        match tauri::WebviewWindowBuilder::new(&app, PINBOARD_WINDOW_LABEL, url)
            .title("贴图板")
            .inner_size(900.0, 700.0)
            .resizable(true)
            .decorations(false)
            .transparent(true)
            .center()
            .build()
        {
            Ok(win) => {
                let _ = win.show();
                #[cfg(target_os = "windows")]
                {
                    let win_clone = win.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        use window_vibrancy::apply_acrylic;
                        let _ = apply_acrylic(&win_clone, Some(crate::MAIN_ACRYLIC));
                    });
                }
            }
            Err(e) => {
                log::error!("[pinboard] 创建贴图板窗口失败: {}", e);
            }
        }
    });

    Ok(())
}

// ─── 贴图板数据系统 ───────────────────────────────────────────

/// 读取 .pgb1_pinboard.json
fn read_pinboard_file(dir: &Path) -> PinboardData {
    let path = dir.join(".pgb1_pinboard.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// 写入 .pgb1_pinboard.json（空时删文件）
fn write_pinboard_file(dir: &Path, data: &PinboardData) -> Result<(), String> {
    let path = dir.join(".pgb1_pinboard.json");
    let non_empty: PinboardData = data.iter()
        .filter(|(_, canvas)| !canvas.pins.is_empty() || !canvas.annotations.is_empty())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if non_empty.is_empty() {
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
    } else {
        let json = serde_json::to_string_pretty(&non_empty)
            .map_err(|e| format!("序列化贴图板失败: {}", e))?;
        fs::write(&path, json)
            .map_err(|e| format!("写入贴图板失败: {}", e))?;
    }
    Ok(())
}

/// 获取指定画布的贴图数据
#[tauri::command]
pub fn get_pinboard(dir_path: String, key: String) -> Result<PinboardCanvas, String> {
    let dir = Path::new(&dir_path);
    let data = read_pinboard_file(dir);
    Ok(data.get(&key).cloned().unwrap_or(PinboardCanvas {
        pins: vec![],
        viewport: None,
        annotations: vec![],
    }))
}

/// 保存整个画布的贴图数据
#[tauri::command]
pub fn save_pinboard(dir_path: String, key: String, canvas: PinboardCanvas) -> Result<(), String> {
    let dir = Path::new(&dir_path);
    let mut data = read_pinboard_file(dir);
    if canvas.pins.is_empty() && canvas.annotations.is_empty() {
        data.remove(&key);
    } else {
        data.insert(key, canvas);
    }
    write_pinboard_file(dir, &data)
}

/// 保存剪贴板图片到 .pgb1_pins/ 目录，返回 (文件名, 宽, 高)
///
/// image_data: RGBA 原始像素字节，width/height: 图片尺寸
#[tauri::command]
pub fn save_pin_image(dir_path: String, image_data: Vec<u8>, width: u32, height: u32) -> Result<(String, u32, u32), String> {
    let dir = Path::new(&dir_path);
    let pins_dir = dir.join(".pgb1_pins");
    if !pins_dir.exists() {
        fs::create_dir_all(&pins_dir)
            .map_err(|e| format!("创建 .pgb1_pins 目录失败: {}", e))?;
    }
    let id = format!("{:016x}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() & 0xFFFFFFFFFFFFFFFF);
    let filename = format!("{}.png", id);
    let file_path = pins_dir.join(&filename);

    // RGBA → PNG 编码
    let img_buf = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, image_data)
        .ok_or_else(|| "RGBA 数据尺寸不匹配".to_string())?;
    img_buf.save(&file_path)
        .map_err(|e| format!("PNG 编码/写入失败: {}", e))?;

    Ok((filename, width, height))
}

/// 删除贴图图片文件
#[tauri::command]
pub fn delete_pin_image(dir_path: String, filename: String) -> Result<(), String> {
    let dir = Path::new(&dir_path);
    let file_path = dir.join(".pgb1_pins").join(&filename);
    if file_path.exists() {
        fs::remove_file(&file_path)
            .map_err(|e| format!("删除贴图文件失败: {}", e))?;
    }
    Ok(())
}
