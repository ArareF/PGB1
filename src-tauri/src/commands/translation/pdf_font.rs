//! PDF 翻译 · CJK 字体处理
//!
//! 职责边界：字体字节加载、TTC→TTF 拆解、Type0+Identity-H+FontFile2 内嵌。
//! 不负责：内容流排版、命令入口、IPC。

use std::collections::BTreeSet;

/// 加载系统中文字体文件（msyh.ttc 优先，fallback simhei.ttf）
pub(super) fn load_cjk_font_bytes() -> Option<Vec<u8>> {
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

/// TTC（字体集合）→ 单独 TTF 字节
/// TTC 内各表的 offset 是相对整个 TTC 文件的绝对地址，需要重新计算后输出独立 TTF
pub(super) fn extract_single_ttf_from_data(data: &[u8]) -> Vec<u8> {
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
pub(super) fn add_yahe_font(
    doc: &mut lopdf::Document,
    raw_font_data: &[u8],
    used_chars: &BTreeSet<char>,
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
