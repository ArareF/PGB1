use crate::models::{AppShortcut, Shortcut, ShortcutsConfig};
use std::fs;
use std::path::Path;
use tauri::{AppHandle, Manager, Runtime};

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
