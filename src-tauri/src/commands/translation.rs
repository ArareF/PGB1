use tauri::Emitter;

#[tauri::command]
pub async fn translate_text_stream(
    app_handle: tauri::AppHandle,
    api_key: String,
    model: String,
    lang_a: String,
    lang_b: String,
    text: String,
) -> Result<(), String> {
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

    // 中日对需要额外的语言判别提示（共享汉字导致自动检测频繁误判）
    let cjk_hint = if (lang_a == "zh-CN" && lang_b == "ja") || (lang_a == "ja" && lang_b == "zh-CN") {
        "\nIMPORTANT: If the text contains any hiragana (あ-ん) or katakana (ア-ン), it is Japanese — translate to Chinese. If it contains no kana and uses simplified Chinese characters or Chinese grammar patterns, it is Chinese — translate to Japanese. The output MUST be in a different language than the input."
    } else {
        ""
    };

    let prompt = format!(
        "You are a translator between {} and {}.\nDetect the input language and translate to the other one.{}\nTone: concise, friendly, natural — like casual coworker chat. No fluff.\nOnly output the translation, nothing else.\n\n\"{}\"",
        lang_a_display, lang_b_display, cjk_hint, text
    );

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse",
        model
    );

    let body = serde_json::json!({
        "contents": [{
            "parts": [{ "text": prompt }]
        }]
    });

    // spawn 异步任务，命令立即返回（避免长连接阻塞 IPC）
    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::new();
        let response = match client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-goog-api-key", &api_key)
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let _ = app_handle.emit("translate-error", format!("网络错误：{}", e));
                return;
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().await.unwrap_or_default();
            let _ = app_handle.emit("translate-error", format!("API 错误 {}: {}", status, err_text));
            return;
        }

        // SSE 流式读取：TCP chunk 边界与 SSE 事件边界不对齐，需 buffer 累积
        let mut buffer = String::new();
        let mut response = response;
        while let Ok(Some(chunk)) = response.chunk().await {
            let chunk_str = match std::str::from_utf8(&chunk) {
                Ok(s) => s,
                Err(_) => continue,
            };
            // 统一换行符：Google API 返回 \r\n，需规范为 \n 才能正确分割 SSE 事件
            buffer.push_str(&chunk_str.replace('\r', ""));

            // 按 \n\n 分割完整的 SSE 事件
            while let Some(pos) = buffer.find("\n\n") {
                let event_block = buffer[..pos].to_string();
                buffer = buffer[pos + 2..].to_string();

                // 提取 data: 行
                for line in event_block.lines() {
                    let line = line.trim();
                    if let Some(json_str) = line.strip_prefix("data: ") {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                            if let Some(text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                                let _ = app_handle.emit("translate-chunk", text.to_string());
                            }
                        }
                    }
                }
            }
        }

        // 流结束后处理 buffer 中残留的最后一个事件（可能没有尾部 \n\n）
        for line in buffer.lines() {
            let line = line.trim();
            if let Some(json_str) = line.strip_prefix("data: ") {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                        let _ = app_handle.emit("translate-chunk", text.to_string());
                    }
                }
            }
        }

        let _ = app_handle.emit("translate-done", ());
    });

    Ok(())
}

#[tauri::command]
pub async fn toggle_translator_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::hotkey::do_toggle_window(&app);
    Ok(())
}

#[tauri::command]
pub async fn extract_pdf_pages_text(path: String) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || extract_pdf_pages_text_inner(&path))
        .await
        .map_err(|e| format!("提取任务失败: {}", e))?
}

fn extract_pdf_pages_text_inner(path: &str) -> Result<Vec<String>, String> {
    // ① 用 lopdf 获取准确页数
    let doc = lopdf::Document::load(&path)
        .map_err(|e| format!("读取 PDF 失败: {}", e))?;
    let page_count = doc.page_iter().count();
    if page_count == 0 {
        return Err("PDF 无页面".to_string());
    }

    // ② 用 pdf_extract 提取全文（正确处理字体 encoding）
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("读取文件字节失败: {}", e))?;
    let text_all = pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| format!("提取文字失败: {}", e))?;

    // ③ 优先按换页符 \x0C 分页；分段数不足时按字符数均分
    let ff_segments: Vec<&str> = text_all.split('\x0C').collect();

    let raw_pages: Vec<String> = if ff_segments.len() >= page_count {
        // 换页符分页足够——直接取前 page_count 段
        ff_segments.iter()
            .take(page_count)
            .map(|s| s.trim().to_string())
            .collect()
    } else {
        // 换页符不足——按段落（双换行）边界分配，避免截断在单词中间
        let paragraphs: Vec<&str> = text_all.split("\n\n")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if paragraphs.is_empty() {
            vec![text_all.trim().to_string()]
        } else {
            let para_per_page = (paragraphs.len() + page_count - 1) / page_count;
            let mut result: Vec<String> = paragraphs
                .chunks(para_per_page.max(1))
                .map(|chunk| chunk.join("\n\n"))
                .collect();
            while result.len() < page_count { result.push(String::new()); }
            result.truncate(page_count);
            result
        }
    };

    // ④ 打印调试日志（不再截断，让 Gemini 收到完整页面文字）
    let pages: Vec<String> = raw_pages.into_iter().enumerate().map(|(i, text)| {
        eprintln!("[PDF] page {} extracted {} chars", i + 1, text.chars().count());
        text
    }).collect();

    let has_text = pages.iter().any(|p| !p.trim().is_empty());
    if !has_text {
        return Err("未检测到可翻译的文字（可能是扫描版 PDF）".to_string());
    }

    Ok(pages)
}

/// 非流式翻译，供 PDF 逐页调用（避免与 translate_text_stream 事件总线冲突）
/// 固定翻译为简体中文输出
#[tauri::command]
pub async fn translate_text_once(
    app_handle: tauri::AppHandle,
    api_key: String,
    model: String,
    text: String,
    page_index: Option<u32>,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("请先在设置中配置 Gemini API Key".to_string());
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    let prompt = format!(
        "You are a professional document translator. Translate the following text to Simplified Chinese.\n\
         Rules:\n\
         - Preserve paragraph structure and line breaks\n\
         - Only output the translation, nothing else\n\
         - Do not add any explanation or commentary\n\n\
         Text to translate:\n{}",
        trimmed
    );

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model
    );

    let body = serde_json::json!({
        "contents": [{ "parts": [{ "text": prompt }] }],
        "generationConfig": { "temperature": 0.1 }
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("构建 HTTP 客户端失败: {}", e))?;

    // 遇到 503/429 时自动重试，最多 6 次，指数退避 10s→20s→40s→80s→120s→120s（cap 120s）
    const MAX_RETRIES: u32 = 6;
    const MAX_WAIT_SECS: u64 = 120;
    let mut last_err = String::new();
    let mut translated = String::new();
    let mut success = false;

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let wait_secs = (10u64 * 2u64.pow(attempt - 1)).min(MAX_WAIT_SECS);
            eprintln!("[translate] attempt {} waiting {}s...", attempt, wait_secs);
            let _ = app_handle.emit("pdf-translate-retry", serde_json::json!({
                "page": page_index.unwrap_or(0),
                "attempt": attempt,
                "maxRetries": MAX_RETRIES,
                "waitSecs": wait_secs,
                "error": &last_err,
            }));
            tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
        }

        let preview: String = trimmed.chars().take(120).collect();
        eprintln!("[translate] attempt {} sending ({} chars): {:?}", attempt, trimmed.len(), preview);
        let response = match client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-goog-api-key", &api_key)
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("网络错误: {}", e);
                eprintln!("[translate] attempt {} send error: {}", attempt, last_err);
                continue;
            }
        };

        let status = response.status();
        eprintln!("[translate] attempt {} status: {}", attempt, status);
        if status.as_u16() == 503 || status.as_u16() == 429 {
            let err_text = response.text().await.unwrap_or_default();
            last_err = format!("API 错误 {}: {}", status, err_text);
            continue; // 重试
        }
        if !status.is_success() {
            let err_text = response.text().await.unwrap_or_default();
            return Err(format!("API 错误 {}: {}", status, err_text));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("解析响应失败: {}", e))?;

        translated = json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| "响应格式异常，无法提取翻译文本".to_string())?
            .trim()
            .to_string();
        success = true;
        break;
    }

    if !success {
        return Err(format!("翻译失败（重试 {} 次后）: {}", MAX_RETRIES, last_err));
    }

    Ok(translated)
}

// ─── PDF 翻译辅助函数 ──────────────────────────────────────────

/// lopdf::Object → f32（Integer 和 Real 两种变体，其余返回 0.0）
#[inline]
fn obj_to_f32(obj: &lopdf::Object) -> f32 {
    match obj {
        lopdf::Object::Integer(i) => *i as f32,
        lopdf::Object::Real(f)    => *f as f32,
        _ => 0.0,
    }
}

/// 加载系统中文字体文件（msyh.ttc 优先，fallback simhei.ttf）
fn load_cjk_font_bytes() -> Option<Vec<u8>> {
    let candidates = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyhbd.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
    ];
    for path in &candidates {
        if let Ok(data) = std::fs::read(path) {
            return Some(data);
        }
    }
    None
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

/// TTC（字体集合）→ 单独 TTF 字节
/// TTC 内各表的 offset 是相对整个 TTC 文件的绝对地址，需要重新计算后输出独立 TTF
fn extract_single_ttf_from_data(data: &[u8]) -> Vec<u8> {
    // 不是 TTC（没有 "ttcf" 魔术头）→ 直接返回原数据
    if data.len() < 12 || &data[0..4] != b"ttcf" {
        return data.to_vec();
    }
    if data.len() < 16 { return data.to_vec(); }

    // 取第 0 个字体的偏移
    let face0_off = u32::from_be_bytes([data[12], data[13], data[14], data[15]]) as usize;
    if face0_off + 12 > data.len() { return data.to_vec(); }

    // numTables 在字体头偏移 4
    let num_tables = u16::from_be_bytes([data[face0_off + 4], data[face0_off + 5]]) as usize;
    let tbl_dir = face0_off + 12; // 表目录起点
    if tbl_dir + num_tables * 16 > data.len() { return data.to_vec(); }

    // 读取所有表记录（tag / checksum / abs_offset / length）
    struct Rec { tag: [u8;4], chk: [u8;4], abs_off: usize, len: usize }
    let recs: Vec<Rec> = (0..num_tables).map(|i| {
        let p = tbl_dir + i * 16;
        Rec {
            tag:     [data[p], data[p+1], data[p+2], data[p+3]],
            chk:     [data[p+4], data[p+5], data[p+6], data[p+7]],
            abs_off: u32::from_be_bytes([data[p+8],  data[p+9],  data[p+10], data[p+11]]) as usize,
            len:     u32::from_be_bytes([data[p+12], data[p+13], data[p+14], data[p+15]]) as usize,
        }
    }).collect();

    // 新 TTF 布局：字体头(12) + 表目录(N*16) + 表数据（各自 4 字节对齐）
    let header_sz = 12 + num_tables * 16;
    let mut new_offs: Vec<usize> = Vec::with_capacity(num_tables);
    let mut cur = header_sz;
    for rec in &recs {
        if cur % 4 != 0 { cur += 4 - cur % 4; }
        new_offs.push(cur);
        cur += rec.len;
    }

    let mut out = vec![0u8; cur + 4]; // 末尾留少量 padding

    // 写字体头（sfVersion + numTables + searchRange + entrySelector + rangeShift）
    out[..12].copy_from_slice(&data[face0_off..face0_off + 12]);

    // 写表目录（新 offset）
    for (i, (rec, &new_off)) in recs.iter().zip(new_offs.iter()).enumerate() {
        let p = 12 + i * 16;
        out[p..p+4].copy_from_slice(&rec.tag);
        out[p+4..p+8].copy_from_slice(&rec.chk);
        out[p+8..p+12].copy_from_slice(&(new_off as u32).to_be_bytes());
        out[p+12..p+16].copy_from_slice(&(rec.len as u32).to_be_bytes());
    }

    // 写表数据
    for (rec, &new_off) in recs.iter().zip(new_offs.iter()) {
        let src_end = (rec.abs_off + rec.len).min(data.len());
        let copy_len = src_end.saturating_sub(rec.abs_off);
        if copy_len > 0 && new_off + copy_len <= out.len() {
            out[new_off..new_off + copy_len].copy_from_slice(&data[rec.abs_off..src_end]);
        }
    }

    out
}

/// 向文档添加嵌入 CJK 字体（Identity-H + FontFile2），返回 Type0 字体 ObjectId
/// 嵌入字体文件保证 PDF 查看器能正确渲染中文，不依赖系统字体
/// `used_chars`: 翻译中实际使用的字符集，用于生成正确的 ToUnicode CMap
fn add_yahe_font(
    doc: &mut lopdf::Document,
    raw_font_data: &[u8],
    used_chars: &std::collections::BTreeSet<char>,
) -> lopdf::ObjectId {
    use lopdf::{Dictionary, Object, Stream, StringFormat};

    // 从 TTC 提取单独 TTF（若已是 TTF 则直接使用）
    let ttf_bytes = extract_single_ttf_from_data(raw_font_data);
    let ttf_len = ttf_bytes.len() as i64;

    // 从 ttf-parser 读取真实字体度量值
    let (ascent, descent, cap_height, bbox) = {
        use ttf_parser::Face;
        match Face::parse(&ttf_bytes, 0) {
            Ok(face) => {
                let em = face.units_per_em() as f32;
                let scale = 1000.0 / em;
                let a = (face.ascender() as f32 * scale) as i64;
                let d = (face.descender() as f32 * scale) as i64;
                let ch = face.capital_height().map(|h| (h as f32 * scale) as i64).unwrap_or(a);
                let bb = face.global_bounding_box();
                let bbox_arr = [
                    (bb.x_min as f32 * scale) as i64,
                    (bb.y_min as f32 * scale) as i64,
                    (bb.x_max as f32 * scale) as i64,
                    (bb.y_max as f32 * scale) as i64,
                ];
                (a, d, ch, bbox_arr)
            }
            Err(_) => (859, -141, 731, [-30, -141, 1030, 859]),
        }
    };

    // FontFile2 流
    let mut ffs_dict = Dictionary::new();
    ffs_dict.set("Length1", Object::Integer(ttf_len));
    let font_file_id = doc.add_object(Object::Stream(Stream::new(ffs_dict, ttf_bytes)));

    // FontDescriptor（使用真实字体度量）
    let descriptor = Dictionary::from_iter(vec![
        ("Type",        Object::Name(b"FontDescriptor".to_vec())),
        ("FontName",    Object::Name(b"ZhCJKFont".to_vec())),
        ("Flags",       Object::Integer(4)),
        ("FontBBox",    Object::Array(vec![
            Object::Integer(bbox[0]), Object::Integer(bbox[1]),
            Object::Integer(bbox[2]), Object::Integer(bbox[3]),
        ])),
        ("ItalicAngle", Object::Integer(0)),
        ("Ascent",      Object::Integer(ascent)),
        ("Descent",     Object::Integer(descent)),
        ("CapHeight",   Object::Integer(cap_height)),
        ("StemV",       Object::Integer(80)),
        ("FontFile2",   Object::Reference(font_file_id)),
    ]);
    let descriptor_id = doc.add_object(Object::Dictionary(descriptor));

    // CIDFont（descendant）
    let cid_font = Dictionary::from_iter(vec![
        ("Type",            Object::Name(b"Font".to_vec())),
        ("Subtype",         Object::Name(b"CIDFontType2".to_vec())),
        ("BaseFont",        Object::Name(b"ZhCJKFont".to_vec())),
        ("DW",              Object::Integer(1000)),
        ("CIDSystemInfo",   Object::Dictionary(Dictionary::from_iter(vec![
            ("Registry",   Object::String(b"Adobe".to_vec(), StringFormat::Literal)),
            ("Ordering",   Object::String(b"Identity".to_vec(), StringFormat::Literal)),
            ("Supplement", Object::Integer(0)),
        ]))),
        ("FontDescriptor",  Object::Reference(descriptor_id)),
    ]);
    let cid_id = doc.add_object(Object::Dictionary(cid_font));

    // ToUnicode CMap — 逐字符 GlyphID → Unicode 映射（保证翻译后文本可复制）
    let cmap_content = {
        use ttf_parser::Face;
        let mut bfchar_lines = Vec::new();
        if let Ok(face) = Face::parse(raw_font_data, 0) {
            for &ch in used_chars {
                if let Some(gid) = face.glyph_index(ch) {
                    bfchar_lines.push(format!("<{:04X}> <{:04X}>", gid.0, ch as u32));
                }
            }
        }
        // CMap 规范：每个 beginbfchar 最多 100 条
        let mut cmap = String::from(
            "/CIDInit /ProcSet findresource begin\n\
             12 dict begin\n\
             begincmap\n\
             /CIDSystemInfo\n\
             << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> def\n\
             /CMapName /Identity-H def\n\
             /CMapType 2 def\n\
             1 begincodespacerange\n\
             <0000> <FFFF>\n\
             endcodespacerange\n"
        );
        for chunk in bfchar_lines.chunks(100) {
            cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
            for line in chunk {
                cmap.push_str(line);
                cmap.push('\n');
            }
            cmap.push_str("endbfchar\n");
        }
        cmap.push_str("endcmap\nCMap currentdict end end\n");
        cmap.into_bytes()
    };
    let cmap_id = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), cmap_content)));

    // Type0（composite）font
    let type0_font = Dictionary::from_iter(vec![
        ("Type",            Object::Name(b"Font".to_vec())),
        ("Subtype",         Object::Name(b"Type0".to_vec())),
        ("BaseFont",        Object::Name(b"ZhCJKFont".to_vec())),
        ("Encoding",        Object::Name(b"Identity-H".to_vec())),
        ("DescendantFonts", Object::Array(vec![Object::Reference(cid_id)])),
        ("ToUnicode",       Object::Reference(cmap_id)),
    ]);
    doc.add_object(Object::Dictionary(type0_font))
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
struct ImagePlacement {
    xobject_name: String,
    y: f32,           // 原始 y 坐标（用于排序）
    display_w: f32,   // 显示宽度 pt
    display_h: f32,   // 显示高度 pt
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
fn extract_image_placements(ops: &[lopdf::content::Operation]) -> Vec<ImagePlacement> {
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
fn get_page_xobject_dict(
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
struct RenderedPage {
    content_bytes: Vec<u8>,
    xobject_names: Vec<String>,
}

/// 将翻译文字和图片流式排版到一页或多页
fn render_flow_pages(
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
                    let blocks = extract_text_blocks_from_ops(&c.operations);
                    let fs = if blocks.is_empty() { 11.0 } else {
                        (blocks.iter().map(|b| b.font_size).sum::<f32>() / blocks.len() as f32).max(8.0)
                    };
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
