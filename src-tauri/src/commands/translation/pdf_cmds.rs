//! PDF 翻译 · 命令入口（build_translated_pdf / check_translated_pdf_exists）
//!
//! 职责边界：对外暴露 #[tauri::command]，内部整合 `pdf_font` 字体加载和
//! `pdf_reflow` 流式排版，生成 `{原文件名}_zh.pdf`。

use super::pdf_font::{add_yahe_font, load_cjk_font_bytes};
use super::pdf_reflow::{
    compute_avg_font_size, extract_image_placements, get_page_xobject_dict, obj_to_f32,
    render_flow_pages, ImagePlacement,
};

/// 构建翻译后的 PDF，保存为 {原文件名}_zh.pdf
///
/// 策略（Reflow）：
///   - 从原始内容流中提取图片 XObject 位置和尺寸
///   - 将翻译文字和图片按流式布局重新排版到全新页面
///   - 页数自适应内容量（不够自动加页，多余自动减）
///   - 图片按原始相对位置穿插在翻译文字之间，居中显示
#[tauri::command]
pub async fn build_translated_pdf(
    path: String,
    translations: Vec<String>,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || build_translated_pdf_inner(&path, &translations))
        .await
        .map_err(|e| format!("构建任务失败: {}", e))?
}

fn build_translated_pdf_inner(
    path: &str,
    translations: &[String],
) -> Result<String, String> {
    use lopdf::{content::Content, Dictionary, Object, Stream};

    let font_data = load_cjk_font_bytes()
        .ok_or_else(|| "未找到系统中文字体（msyh.ttc / simhei.ttf），请确认 Windows 字体已安装".to_string())?;

    let mut doc = lopdf::Document::load(path)
        .map_err(|e| format!("打开 PDF 失败: {}", e))?;

    let used_chars: std::collections::BTreeSet<char> = translations.iter()
        .flat_map(|t| t.chars()).collect();
    let font_id = add_yahe_font(&mut doc, &font_data, &used_chars);

    let orig_page_ids: Vec<lopdf::ObjectId> = doc.page_iter().collect();

    // ── 不可变阶段：逐页收集 MediaBox、图片、文字 bbox、XObject 字典 ──
    struct PageData {
        media_box: (f32, f32),
        images: Vec<ImagePlacement>,
        avg_fs: f32,
        xobject_dict: Dictionary,
    }

    let page_data: Vec<PageData> = orig_page_ids.iter().map(|&pid| {
        let w = doc.get_object(pid).ok()
            .and_then(|o| o.as_dict().ok().map(|d| d.clone()))
            .and_then(|d| d.get(b"MediaBox").ok().map(|o| o.clone()))
            .and_then(|o| o.as_array().ok().map(|a| a.clone()))
            .and_then(|arr| arr.get(2).cloned())
            .map(|o| obj_to_f32(&o)).filter(|&v| v > 0.0).unwrap_or(595.0);
        let h = doc.get_object(pid).ok()
            .and_then(|o| o.as_dict().ok().map(|d| d.clone()))
            .and_then(|d| d.get(b"MediaBox").ok().map(|o| o.clone()))
            .and_then(|o| o.as_array().ok().map(|a| a.clone()))
            .and_then(|arr| arr.get(3).cloned())
            .map(|o| obj_to_f32(&o)).filter(|&v| v > 0.0).unwrap_or(842.0);

        let (images, avg_fs) = match doc.get_page_content(pid) {
            Ok(data) => match Content::decode(&data) {
                Ok(c) => {
                    let imgs = extract_image_placements(&c.operations);
                    let fs = compute_avg_font_size(&c.operations);
                    (imgs, fs)
                }
                Err(_) => (vec![], 11.0),
            },
            Err(_) => (vec![], 11.0),
        };

        let xobject_dict = get_page_xobject_dict(&doc, pid);

        PageData { media_box: (w, h), images, avg_fs, xobject_dict }
    }).collect();

    // 找到 Pages 根节点 ID
    let pages_root_id = doc.catalog()
        .map_err(|e| format!("无法读取 Catalog: {}", e))?
        .get(b"Pages").map_err(|_| "Catalog 无 Pages".to_string())?
        .as_reference().map_err(|_| "Pages 不是引用".to_string())?;

    // ── 可变阶段：逐页生成 reflow 内容，构建新页面树 ──
    let mut new_kids: Vec<Object> = Vec::new();

    for (page_idx, &orig_page_id) in orig_page_ids.iter().enumerate() {
        let translation = match translations.get(page_idx) {
            Some(t) if !t.trim().is_empty() => t.as_str(),
            _ => {
                // 无翻译：保留原页
                new_kids.push(Object::Reference(orig_page_id));
                continue;
            }
        };

        let data = &page_data[page_idx];
        let (pw, ph) = data.media_box;

        // 渲染为一页或多页
        let rendered = render_flow_pages(
            translation, &data.images, pw, ph, data.avg_fs, &font_data,
        );

        for rp in &rendered {
            // 创建内容流对象
            let content_id = doc.add_object(Object::Stream(
                Stream::new(Dictionary::new(), rp.content_bytes.clone())
            ));

            // 构建 Resources：字体 + 需要的 XObject
            let mut font_dict = Dictionary::new();
            font_dict.set("ZhF0", Object::Reference(font_id));

            let mut resources = Dictionary::new();
            resources.set("Font", Object::Dictionary(font_dict));

            if !rp.xobject_names.is_empty() {
                let mut xobj = Dictionary::new();
                for name in &rp.xobject_names {
                    if let Ok(obj_ref) = data.xobject_dict.get(name.as_bytes()) {
                        xobj.set(name.as_str(), obj_ref.clone());
                    }
                }
                if !xobj.is_empty() {
                    resources.set("XObject", Object::Dictionary(xobj));
                }
            }

            // 创建新页面对象
            let mut page_dict = Dictionary::new();
            page_dict.set("Type", Object::Name(b"Page".to_vec()));
            page_dict.set("Parent", Object::Reference(pages_root_id));
            page_dict.set("MediaBox", Object::Array(vec![
                Object::Integer(0), Object::Integer(0),
                Object::Real(pw), Object::Real(ph),
            ]));
            page_dict.set("Contents", Object::Reference(content_id));
            page_dict.set("Resources", Object::Dictionary(resources));

            let new_page_id = doc.add_object(Object::Dictionary(page_dict));
            new_kids.push(Object::Reference(new_page_id));
        }
    }

    // 更新 Pages 根节点
    let pages_root = doc.get_object_mut(pages_root_id)
        .map_err(|_| "Pages 根节点不存在".to_string())?
        .as_dict_mut()
        .map_err(|_| "Pages 根节点不是 dict".to_string())?;
    pages_root.set("Kids", Object::Array(new_kids.clone()));
    pages_root.set("Count", Object::Integer(new_kids.len() as i64));

    // 确保所有新页的 Parent 指向根节点
    for kid in &new_kids {
        if let Ok(pid) = kid.as_reference() {
            if let Ok(obj) = doc.get_object_mut(pid) {
                if let Ok(d) = obj.as_dict_mut() {
                    d.set("Parent", Object::Reference(pages_root_id));
                }
            }
        }
    }

    let input_path = std::path::Path::new(path);
    let stem = input_path.file_stem().and_then(|s| s.to_str()).unwrap_or("translated");
    let output_path = input_path
        .with_file_name(format!("{}_zh.pdf", stem))
        .to_str().ok_or("输出路径生成失败")?.to_string();

    doc.save(&output_path).map_err(|e| format!("保存 PDF 失败: {}", e))?;
    Ok(output_path)
}

/// 检测已有翻译文件 {stem}_zh.pdf 是否存在
#[tauri::command]
pub async fn check_translated_pdf_exists(path: String) -> Result<Option<String>, String> {
    let input = std::path::Path::new(&path);
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let zh_path = input.with_file_name(format!("{}_zh.pdf", stem));
    if tokio::fs::metadata(&zh_path).await.is_ok() {
        Ok(Some(zh_path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}
