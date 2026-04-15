//! PDF 翻译 · 内容流提取 + 流式排版
//!
//! 职责边界：从原始 PDF 内容流提取图片/文字块、分析矩阵 CTM、按字符宽度流式排版
//! 翻译文本并穿插图片重新生成内容流。
//! 不负责：字体字节加载（见 `pdf_font`）、命令入口（见 `pdf_cmds`）。

/// lopdf::Object → f32（Integer 和 Real 两种变体，其余返回 0.0）
#[inline]
pub(super) fn obj_to_f32(obj: &lopdf::Object) -> f32 {
    match obj {
        lopdf::Object::Integer(i) => *i as f32,
        lopdf::Object::Real(f)    => *f as f32,
        _ => 0.0,
    }
}

/// 将文字编码为 Identity-H CID hex（使用 ttf-parser 查找实际 GlyphID）
/// Identity-H 下 CID 直接作为 GlyphID 使用，必须用真实值否则乱码
fn encode_chars_to_cid_hex(text: &str, font_data: &[u8]) -> String {
    use ttf_parser::Face;
    let face = match Face::parse(font_data, 0) {
        Ok(f) => f,
        Err(_) => {
            // 解析失败：fallback 到 Unicode codepoint（大概率乱码但至少写进去）
            let mut hex = String::new();
            for c in text.chars() {
                if (c as u32) <= 0xFFFF {
                    hex.push_str(&format!("{:04X}", c as u32));
                }
            }
            return hex;
        }
    };

    let mut hex = String::new();
    for c in text.chars() {
        if let Some(glyph_id) = face.glyph_index(c) {
            hex.push_str(&format!("{:04X}", glyph_id.0));
        }
        // 找不到字形（如特殊控制字符）则跳过
    }
    hex
}

/// 文字块信息（字号，用于计算页面平均字号）
struct TextBlock {
    font_size: f32,
}

/// 从 lopdf 内容流操作中提取所有文字块（每次位置变更拆分为独立 block）
fn extract_text_blocks_from_ops(operations: &[lopdf::content::Operation]) -> Vec<TextBlock> {
    // 每次位置变更（Tm/Td/TD/T*）将累积文字刷新为一个 TextBlock
    fn flush(has_text: &mut bool, tf: f32, tmd: f32, out: &mut Vec<TextBlock>) {
        if *has_text {
            let efs = tf * tmd.abs();
            let fs = if efs > 0.5 { efs } else { tf };
            out.push(TextBlock { font_size: fs });
            *has_text = false;
        }
    }

    let mut blocks: Vec<TextBlock> = Vec::new();
    let mut in_bt = false;
    let mut tf_size: f32 = 12.0;
    let mut tm_d: f32 = 1.0;          // Tm 矩阵竖直缩放分量
    let mut has_text = false;          // 当前 run 是否有文字

    for op in operations {
        match op.operator.as_str() {
            "BT" => {
                in_bt = true;
                tm_d = 1.0;
                has_text = false;
            }
            "ET" => {
                if in_bt {
                    flush(&mut has_text, tf_size, tm_d, &mut blocks);
                }
                in_bt = false;
            }
            "Tf" if in_bt => {
                if op.operands.len() >= 2 {
                    tf_size = obj_to_f32(&op.operands[1]);
                    if tf_size <= 0.0 { tf_size = 12.0; }
                }
            }
            "Tm" if in_bt => {
                if op.operands.len() >= 6 {
                    flush(&mut has_text, tf_size, tm_d, &mut blocks);
                    tm_d = obj_to_f32(&op.operands[3]);
                }
            }
            "Td" if in_bt => {
                if op.operands.len() >= 2 {
                    flush(&mut has_text, tf_size, tm_d, &mut blocks);
                }
            }
            "TD" if in_bt => {
                if op.operands.len() >= 2 {
                    flush(&mut has_text, tf_size, tm_d, &mut blocks);
                }
            }
            "T*" if in_bt => {
                flush(&mut has_text, tf_size, tm_d, &mut blocks);
            }
            "Tj" if in_bt => {
                if op.operands.first().and_then(|o| if let lopdf::Object::String(_, _) = o { Some(()) } else { None }).is_some() {
                    has_text = true;
                }
            }
            "TJ" if in_bt => {
                if let Some(lopdf::Object::Array(arr)) = op.operands.first() {
                    if arr.iter().any(|item| matches!(item, lopdf::Object::String(_, _))) {
                        has_text = true;
                    }
                }
            }
            _ => {}
        }
    }
    blocks
}

/// 计算页面平均字号（用于 reflow 排版字号推断，空页兜底 11.0，下界 8.0）
pub(super) fn compute_avg_font_size(ops: &[lopdf::content::Operation]) -> f32 {
    let blocks = extract_text_blocks_from_ops(ops);
    if blocks.is_empty() {
        11.0
    } else {
        (blocks.iter().map(|b| b.font_size).sum::<f32>() / blocks.len() as f32).max(8.0)
    }
}

/// 生成覆盖文字区域的白色矩形 PDF 路径操作（调用方自行包含 padding）
/// 将 hex 字符串编码的 CID 转为字节并输出 Tj 操作
fn emit_cid_text(
    chunk: &str,
    font_data: &[u8],
    ops: &mut Vec<lopdf::content::Operation>,
) {
    use lopdf::{content::Operation, Object, StringFormat};
    let hex = encode_chars_to_cid_hex(chunk, font_data);
    if hex.is_empty() { return; }
    let cid_bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .filter_map(|i| {
            if i + 2 <= hex.len() { u8::from_str_radix(&hex[i..i + 2], 16).ok() } else { None }
        })
        .collect();
    ops.push(Operation::new("Tj", vec![
        Object::String(cid_bytes, StringFormat::Hexadecimal),
    ]));
}

// ── Reflow PDF 生成：图片提取 + 流式排版 + 自适应分页 ──

/// 从内容流中提取的图片位置信息
pub(super) struct ImagePlacement {
    pub(super) xobject_name: String,
    pub(super) y: f32,           // 原始 y 坐标（用于排序）
    pub(super) display_w: f32,   // 显示宽度 pt
    pub(super) display_h: f32,   // 显示高度 pt
}

/// 2D 仿射矩阵乘法 [a b c d e f] 代表 [[a,b,0],[c,d,0],[e,f,1]]
fn multiply_ctm(m: &[f32; 6], cur: &[f32; 6]) -> [f32; 6] {
    [
        m[0] * cur[0] + m[1] * cur[2],
        m[0] * cur[1] + m[1] * cur[3],
        m[2] * cur[0] + m[3] * cur[2],
        m[2] * cur[1] + m[3] * cur[3],
        m[4] * cur[0] + m[5] * cur[2] + cur[4],
        m[4] * cur[1] + m[5] * cur[3] + cur[5],
    ]
}

/// 从内容流操作中提取所有图片 XObject 的位置和显示尺寸
pub(super) fn extract_image_placements(ops: &[lopdf::content::Operation]) -> Vec<ImagePlacement> {
    let identity: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut ctm_stack: Vec<[f32; 6]> = vec![identity];
    let mut images = Vec::new();

    for op in ops {
        match op.operator.as_str() {
            "q" => { ctm_stack.push(*ctm_stack.last().unwrap_or(&identity)); }
            "Q" => { if ctm_stack.len() > 1 { ctm_stack.pop(); } }
            "cm" => {
                if op.operands.len() >= 6 {
                    let m = [
                        obj_to_f32(&op.operands[0]), obj_to_f32(&op.operands[1]),
                        obj_to_f32(&op.operands[2]), obj_to_f32(&op.operands[3]),
                        obj_to_f32(&op.operands[4]), obj_to_f32(&op.operands[5]),
                    ];
                    let cur = ctm_stack.last_mut().unwrap();
                    *cur = multiply_ctm(&m, cur);
                }
            }
            "Do" => {
                if let Some(lopdf::Object::Name(name)) = op.operands.first() {
                    let ctm = ctm_stack.last().unwrap_or(&identity);
                    let dw = (ctm[0].powi(2) + ctm[1].powi(2)).sqrt();
                    let dh = (ctm[2].powi(2) + ctm[3].powi(2)).sqrt();
                    if dw > 1.0 && dh > 1.0 {
                        images.push(ImagePlacement {
                            xobject_name: String::from_utf8_lossy(name).to_string(),
                            y: ctm[5],
                            display_w: dw,
                            display_h: dh,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    images
}

/// 获取页面 Resources/XObject 字典（解引用间接对象）
pub(super) fn get_page_xobject_dict(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> lopdf::Dictionary {
    use lopdf::{Dictionary, Object};
    let res_opt: Option<Object> = doc.get_object(page_id).ok()
        .and_then(|o| o.as_dict().ok().map(|d| d.clone()))
        .and_then(|d| d.get(b"Resources").ok().map(|o| o.clone()));
    let res_dict: Dictionary = match res_opt {
        Some(Object::Dictionary(d)) => d,
        Some(Object::Reference(r))  => doc.get_object(r)
            .ok().and_then(|o| o.as_dict().ok().map(|d| d.clone()))
            .unwrap_or_else(Dictionary::new),
        _ => Dictionary::new(),
    };
    match res_dict.get(b"XObject") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(r))  => doc.get_object(*r)
            .ok().and_then(|o| o.as_dict().ok().map(|d| d.clone()))
            .unwrap_or_else(Dictionary::new),
        _ => Dictionary::new(),
    }
}

/// 按字符宽度自动换行
fn wrap_text_lines(text: &str, avail_width: f32, fs: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() { lines.push(String::new()); continue; }
        let chars: Vec<char> = line.chars().collect();
        let mut start = 0;
        let mut w = 0.0f32;
        for (i, &ch) in chars.iter().enumerate() {
            let cw = if ch > '\u{2E7F}' { fs * 0.95 } else { fs * 0.55 };
            if w + cw > avail_width && i > start {
                lines.push(chars[start..i].iter().collect());
                start = i;
                w = 0.0;
            }
            w += cw;
        }
        if start < chars.len() { lines.push(chars[start..].iter().collect()); }
    }
    lines
}

/// 渲染后的单页内容
pub(super) struct RenderedPage {
    pub(super) content_bytes: Vec<u8>,
    pub(super) xobject_names: Vec<String>,
}

/// 将翻译文字和图片流式排版到一页或多页
pub(super) fn render_flow_pages(
    translated: &str,
    images: &[ImagePlacement],
    page_w: f32,
    page_h: f32,
    font_size: f32,
    font_data: &[u8],
) -> Vec<RenderedPage> {
    use lopdf::{content::{Content, Operation}, Object};

    let margin = 36.0f32;
    let content_w = page_w - 2.0 * margin;
    let fs = font_size.max(8.0).min(20.0);
    let line_height = fs * 1.5;
    let img_gap = fs; // 图片前后间距

    let wrapped = wrap_text_lines(translated, content_w, fs);
    let total_lines = wrapped.len();

    // 按 y 坐标降序排列图片（页面顶部先出）
    let mut sorted_imgs: Vec<&ImagePlacement> = images.iter().collect();
    sorted_imgs.sort_by(|a, b| b.y.partial_cmp(&a.y).unwrap_or(std::cmp::Ordering::Equal));

    // 计算每张图片在文字流中的插入位置（0.0=最顶，1.0=最底）
    let usable_h = page_h - 2.0 * margin;
    let insert_fracs: Vec<f32> = sorted_imgs.iter().map(|img| {
        ((page_h - margin - img.y) / usable_h).clamp(0.0, 1.0)
    }).collect();

    // ── 渲染状态 ──
    let mut pages: Vec<RenderedPage> = Vec::new();
    let mut ops: Vec<Operation> = Vec::new();
    let mut xobj_names: Vec<String> = Vec::new();
    let mut y = page_h - margin;
    let mut in_text = false;
    let mut first_text_line = true;
    let mut line_idx = 0;
    let mut img_idx = 0;

    // 开始文字块
    fn begin_text(ops: &mut Vec<Operation>, x: f32, y: f32, fs: f32, lh: f32) {
        ops.push(Operation::new("q", vec![]));
        ops.push(Operation::new("rg", vec![Object::Real(0.0), Object::Real(0.0), Object::Real(0.0)]));
        ops.push(Operation::new("BT", vec![]));
        ops.push(Operation::new("Tf", vec![Object::Name(b"ZhF0".to_vec()), Object::Real(fs)]));
        ops.push(Operation::new("TL", vec![Object::Real(lh)]));
        ops.push(Operation::new("Td", vec![Object::Real(x), Object::Real(y)]));
    }
    fn end_text(ops: &mut Vec<Operation>) {
        ops.push(Operation::new("ET", vec![]));
        ops.push(Operation::new("Q", vec![]));
    }

    // 分页：将当前 ops 存为一页，重置状态
    let flush_page = |pages: &mut Vec<RenderedPage>,
                      ops: &mut Vec<Operation>,
                      xobj_names: &mut Vec<String>,
                      in_text: &mut bool,
                      y: &mut f32,
                      first_text_line: &mut bool| {
        if *in_text { end_text(ops); *in_text = false; }
        if !ops.is_empty() {
            let bytes = Content { operations: std::mem::take(ops) }.encode().unwrap_or_default();
            pages.push(RenderedPage { content_bytes: bytes, xobject_names: std::mem::take(xobj_names) });
        }
        *y = page_h - margin;
        *first_text_line = true;
    };

    loop {
        // 检查是否该插入图片
        let text_frac = if total_lines > 0 { line_idx as f32 / total_lines as f32 } else { 1.0 };
        let should_insert_img = img_idx < insert_fracs.len() && text_frac >= insert_fracs[img_idx];

        if should_insert_img {
            let img = sorted_imgs[img_idx];
            let scale = (content_w / img.display_w).min(1.0);
            let img_w = img.display_w * scale;
            let img_h = img.display_h * scale;

            // 图片前间距
            y -= img_gap;

            // 不够放则分页
            if y - img_h < margin {
                flush_page(&mut pages, &mut ops, &mut xobj_names, &mut in_text, &mut y, &mut first_text_line);
            }

            // 关闭文字块
            if in_text { end_text(&mut ops); in_text = false; }

            // 绘制图片（居中）
            let img_x = margin + (content_w - img_w) / 2.0;
            let img_y = y - img_h;
            ops.push(Operation::new("q", vec![]));
            ops.push(Operation::new("cm", vec![
                Object::Real(img_w), Object::Real(0.0),
                Object::Real(0.0),   Object::Real(img_h),
                Object::Real(img_x), Object::Real(img_y),
            ]));
            ops.push(Operation::new("Do", vec![Object::Name(img.xobject_name.as_bytes().to_vec())]));
            ops.push(Operation::new("Q", vec![]));
            xobj_names.push(img.xobject_name.clone());

            y = img_y - img_gap;
            img_idx += 1;
            first_text_line = true; // 图片后重新开始文字块
            continue;
        }

        if line_idx >= wrapped.len() { break; }

        // 不够放一行则分页
        if y - line_height < margin {
            flush_page(&mut pages, &mut ops, &mut xobj_names, &mut in_text, &mut y, &mut first_text_line);
        }

        // 确保文字块已打开
        if !in_text {
            begin_text(&mut ops, margin, y, fs, line_height);
            in_text = true;
            first_text_line = true;
        }

        let line = &wrapped[line_idx];
        if !first_text_line {
            ops.push(Operation::new("T*", vec![]));
        }
        first_text_line = false;

        if !line.is_empty() {
            emit_cid_text(line, font_data, &mut ops);
        }

        y -= line_height;
        line_idx += 1;
    }

    // 插入剩余图片
    while img_idx < sorted_imgs.len() {
        let img = sorted_imgs[img_idx];
        let scale = (content_w / img.display_w).min(1.0);
        let img_w = img.display_w * scale;
        let img_h = img.display_h * scale;
        y -= img_gap;
        if y - img_h < margin {
            flush_page(&mut pages, &mut ops, &mut xobj_names, &mut in_text, &mut y, &mut first_text_line);
        }
        if in_text { end_text(&mut ops); in_text = false; }
        let img_x = margin + (content_w - img_w) / 2.0;
        let img_y = y - img_h;
        ops.push(Operation::new("q", vec![]));
        ops.push(Operation::new("cm", vec![
            Object::Real(img_w), Object::Real(0.0),
            Object::Real(0.0),   Object::Real(img_h),
            Object::Real(img_x), Object::Real(img_y),
        ]));
        ops.push(Operation::new("Do", vec![Object::Name(img.xobject_name.as_bytes().to_vec())]));
        ops.push(Operation::new("Q", vec![]));
        xobj_names.push(img.xobject_name.clone());
        y = img_y - img_gap;
        img_idx += 1;
    }

    // 末页收尾
    if in_text { end_text(&mut ops); }
    if !ops.is_empty() {
        let bytes = Content { operations: ops }.encode().unwrap_or_default();
        pages.push(RenderedPage { content_bytes: bytes, xobject_names: xobj_names });
    }

    // 保底：至少返回一个空页（避免下游 panic）
    if pages.is_empty() {
        pages.push(RenderedPage { content_bytes: vec![], xobject_names: vec![] });
    }
    pages
}
