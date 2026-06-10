//! PSD/PSB 缩略图提取（图层合并 + 内嵌 JPEG fallback + 磁盘缓存）
//! 缓存 key = hash(路径, mtime, max_size)，与 scan_directory 的命中检查共用 psd_cache_file

use std::fs;
use std::path::{Path, PathBuf};

/// PSD 解析并发上限：避免大量 PSD 同时解析导致线程池饱和
static PSD_SEMAPHORE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

/// 计算 PSD 缩略图缓存文件路径。hash 输入（路径字符串 + mtime + max_size）
/// 必须与所有调用方保持一致，否则缓存永不命中
pub(crate) fn psd_cache_file(cache_dir: &Path, path: &str, mtime: u64, max_size: u32) -> PathBuf {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    mtime.hash(&mut hasher);
    max_size.hash(&mut hasher);
    cache_dir.join(format!("{:016x}.jpg", hasher.finish()))
}

/// 提取 PSD/PSB 缩略图，写入磁盘缓存并返回缓存文件路径（前端用 convertFileSrc 引用）
/// max_size: 最长边像素上限（卡片用 256，侧边栏用 800）
#[tauri::command]
pub async fn extract_psd_thumbnail(
    app_handle: tauri::AppHandle,
    path: String,
    max_size: u32,
) -> Result<Option<String>, String> {
    use tauri::Manager;

    // 用 mtime 做缓存失效判定（tokio::fs 异步版本，不阻塞 executor 线程）
    let mtime = tokio::fs::metadata(&path).await
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
        .unwrap_or(0);

    let cache_dir = app_handle.path().app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {}", e))?
        .join("psd_thumbnails");
    let cache_file = psd_cache_file(&cache_dir, &path, mtime, max_size);

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
