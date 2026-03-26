# PDF 翻译功能实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 FileDetailSidebar 底部为 PDF 文件添加"翻译 PDF"按钮，生成保留图片和排版的中文译文副本 `{name}_zh.pdf`。

**Architecture:**
- Rust 端：`lopdf` 解析 PDF 内容流，提取文字+位置；用白色矩形遮盖原文本块；在同位置写入中文译文（UTF-16BE hex 字符串，Identity-H 编码）；引用系统字体 Microsoft YaHei（Windows Only，不嵌入字体文件，减少体积）。
- 翻译端：新增 `translate_text_once` 命令（非流式，逐页串行调用 Gemini API），避免与现有 `translate_text_stream` 事件总线冲突。
- 前端：FileDetailSidebar 增加 PDF 翻译区块（按钮 → 逐页进度 → 完成后显示打开按钮）。

**Tech Stack:** `lopdf = "0.34"`, `pdf-extract = "0.7"`, 现有 `reqwest + Gemini API`，Vue 3 + i18n

---

## Task 1: 添加 Cargo 依赖并验证编译

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: 添加依赖**

在 `[dependencies]` 末尾添加：
```toml
lopdf = "0.34"
pdf-extract = "0.7"
```

**Step 2: 验证编译**

```bash
cd src-tauri && cargo check 2>&1 | tail -5
```

预期：`Finished` 无 error（warnings 可接受）

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: add lopdf and pdf-extract dependencies for PDF translation"
```

---

## Task 2: 实现 `extract_pdf_pages_text` 命令

**Files:**
- Modify: `src-tauri/src/commands.rs`（在文件末尾 `translate_text_stream` 函数前添加）

**Step 1: 添加命令函数**

在 `translate_text_stream` 函数（约第 4567 行）之前插入：

```rust
/// 提取 PDF 每页的文字内容，返回 Vec<String>（按页序）
#[tauri::command]
pub fn extract_pdf_pages_text(path: String) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("读取 PDF 失败: {}", e))?;

    let text_all = pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| format!("提取 PDF 文字失败: {}", e))?;

    // pdf-extract 返回全文，按换页符 \x0C 拆分
    let pages: Vec<String> = text_all
        .split('\x0C')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if pages.is_empty() {
        return Err("未检测到可翻译的文字（可能是扫描版 PDF）".to_string());
    }

    Ok(pages)
}
```

**Step 2: 在 lib.rs 注册命令**

在 `src-tauri/src/lib.rs` 的 `invoke_handler` 列表末尾（`commands::set_note,` 后）添加：

```rust
            commands::extract_pdf_pages_text,
```

**Step 3: 验证编译**

```bash
cd src-tauri && cargo check 2>&1 | tail -5
```

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add extract_pdf_pages_text command"
```

---

## Task 3: 实现 `translate_text_once` 命令（非流式翻译，供逐页调用）

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: 在 `extract_pdf_pages_text` 之后插入**

```rust
/// 非流式翻译，供 PDF 逐页翻译使用（避免与 translate_text_stream 事件总线冲突）
/// 固定翻译目标语言为简体中文（pdf 翻译场景固定中文输出）
#[tauri::command]
pub async fn translate_text_once(
    api_key: String,
    model: String,
    text: String,
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

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("X-goog-api-key", &api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let err_text = response.text().await.unwrap_or_default();
        return Err(format!("API 错误 {}: {}", status, err_text));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let translated = json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| "响应格式异常，无法提取翻译文本".to_string())?
        .trim()
        .to_string();

    Ok(translated)
}
```

**Step 2: 在 lib.rs 注册命令**

```rust
            commands::translate_text_once,
```

**Step 3: 验证编译**

```bash
cd src-tauri && cargo check 2>&1 | tail -5
```

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add translate_text_once command for per-page PDF translation"
```

---

## Task 4: 实现 `build_translated_pdf` 命令（核心）

**Files:**
- Modify: `src-tauri/src/commands.rs`（在 `translate_text_once` 之后插入）

**Step 1: 添加辅助函数和主命令**

插入以下完整代码块：

```rust
// ─── PDF 翻译辅助：将字符串编码为 UTF-16BE 十六进制字符串 ─────────────────
fn encode_pdf_utf16be_hex(s: &str) -> String {
    let mut hex = String::from("FEFF"); // BOM
    for c in s.chars() {
        let code = c as u32;
        if code <= 0xFFFF {
            hex.push_str(&format!("{:04X}", code));
        } else {
            // Surrogate pair
            let code = code - 0x10000;
            let high = 0xD800 + (code >> 10);
            let low  = 0xDC00 + (code & 0x3FF);
            hex.push_str(&format!("{:04X}{:04X}", high, low));
        }
    }
    hex
}

/// 生成覆盖文本区域的白色矩形 PDF 操作（PDF path operators）
/// y_pdf: PDF 坐标系中的 y（左下为原点）
fn make_white_rect_ops(x: f32, y: f32, width: f32, height: f32) -> Vec<lopdf::content::Operation> {
    use lopdf::content::Operation;
    use lopdf::Object;
    vec![
        // 保存图形状态
        Operation::new("q", vec![]),
        // 设置填充色为白色 (DeviceRGB 1 1 1)
        Operation::new("rg", vec![Object::Real(1.0), Object::Real(1.0), Object::Real(1.0)]),
        // 矩形路径：x y width height re
        Operation::new("re", vec![
            Object::Real(x - 2.0),
            Object::Real(y - 2.0),
            Object::Real(width + 4.0),
            Object::Real(height + 4.0),
        ]),
        // 填充
        Operation::new("f", vec![]),
        // 恢复图形状态
        Operation::new("Q", vec![]),
    ]
}

/// 生成翻译文本的 PDF BT...ET 块（使用 /ZhF0 字体，Identity-H 编码）
fn make_translated_text_ops(
    x: f32,
    y: f32,
    font_size: f32,
    translated: &str,
    page_width: f32,
) -> Vec<lopdf::content::Operation> {
    use lopdf::content::Operation;
    use lopdf::Object;

    // 按行分割翻译文本，防止单行过长
    let max_chars_per_line = ((page_width / font_size) * 1.5) as usize;
    let max_chars_per_line = max_chars_per_line.max(20).min(80);

    let mut ops = vec![
        Operation::new("q", vec![]),
        // 黑色文字
        Operation::new("rg", vec![Object::Real(0.0), Object::Real(0.0), Object::Real(0.0)]),
        Operation::new("BT", vec![]),
        // 设置字体：/ZhF0 + 原字号
        Operation::new("Tf", vec![
            Object::Name(b"ZhF0".to_vec()),
            Object::Real(font_size),
        ]),
        // 设置行间距
        Operation::new("TL", vec![Object::Real(font_size * 1.4)]),
        // 移动到文字起始位置
        Operation::new("Td", vec![Object::Real(x), Object::Real(y)]),
    ];

    // 将翻译文本按行写入
    let lines: Vec<&str> = translated.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            // 空行：换行
            ops.push(Operation::new("T*", vec![]));
            continue;
        }
        // 分块处理过长行
        let mut remaining = line;
        let mut first_chunk = i == 0;
        while !remaining.is_empty() {
            let take = remaining
                .char_indices()
                .nth(max_chars_per_line)
                .map(|(i, _)| i)
                .unwrap_or(remaining.len());
            let chunk = &remaining[..take];
            remaining = &remaining[take..];

            let hex = encode_pdf_utf16be_hex(chunk);
            let hex_bytes = hex.into_bytes();

            if !first_chunk {
                ops.push(Operation::new("T*", vec![]));
            }
            first_chunk = false;

            ops.push(Operation::new("Tj", vec![
                Object::String(hex_bytes, lopdf::StringFormat::Hexadecimal),
            ]));
        }
        if i + 1 < lines.len() {
            ops.push(Operation::new("T*", vec![]));
        }
    }

    ops.push(Operation::new("ET", vec![]));
    ops.push(Operation::new("Q", vec![]));
    ops
}

/// 向 PDF Document 添加 Microsoft YaHei Type0 字体资源（不嵌入字体文件）
/// 返回字体对象 ID，供各页 /Resources 引用
fn add_yahe_font(doc: &mut lopdf::Document) -> lopdf::ObjectId {
    use lopdf::{Dictionary, Object, Stream};

    // CIDFont (descendant)
    let cid_font = Dictionary::from_iter(vec![
        ("Type",       Object::Name(b"Font".to_vec())),
        ("Subtype",    Object::Name(b"CIDFontType2".to_vec())),
        ("BaseFont",   Object::Name(b"MicrosoftYaHei".to_vec())),
        ("DW",         Object::Integer(1000)),
        ("CIDSystemInfo", Object::Dictionary(Dictionary::from_iter(vec![
            ("Registry",   Object::String(b"Adobe".to_vec(), lopdf::StringFormat::Literal)),
            ("Ordering",   Object::String(b"Identity".to_vec(), lopdf::StringFormat::Literal)),
            ("Supplement", Object::Integer(0)),
        ]))),
    ]);
    let cid_id = doc.add_object(Object::Dictionary(cid_font));

    // ToUnicode CMap stream（Identity mapping: CID == Unicode codepoint）
    let cmap_content = b"/CIDInit /ProcSet findresource begin\n\
        12 dict begin\n\
        begincmap\n\
        /CIDSystemInfo\n\
        << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> def\n\
        /CMapName /Identity-H def\n\
        /CMapType 2 def\n\
        1 begincodespacerange\n\
        <0000> <FFFF>\n\
        endcodespacerange\n\
        1 beginbfrange\n\
        <0000> <FFFF> <0000>\n\
        endbfrange\n\
        endcmap\n\
        CMap currentdict end end\n".to_vec();
    let cmap_stream = Stream::new(Dictionary::new(), cmap_content);
    let cmap_id = doc.add_object(Object::Stream(cmap_stream));

    // Type0 (composite) font
    let type0_font = Dictionary::from_iter(vec![
        ("Type",            Object::Name(b"Font".to_vec())),
        ("Subtype",         Object::Name(b"Type0".to_vec())),
        ("BaseFont",        Object::Name(b"MicrosoftYaHei".to_vec())),
        ("Encoding",        Object::Name(b"Identity-H".to_vec())),
        ("DescendantFonts", Object::Array(vec![Object::Reference(cid_id)])),
        ("ToUnicode",       Object::Reference(cmap_id)),
    ]);
    doc.add_object(Object::Dictionary(type0_font))
}

/// 从 lopdf 内容流操作中提取文字块信息（位置 + 文字）
struct TextBlock {
    x: f32,
    y: f32,
    font_size: f32,
    text: String,
}

fn extract_text_blocks(operations: &[lopdf::content::Operation]) -> Vec<TextBlock> {
    let mut blocks: Vec<TextBlock> = Vec::new();
    let mut in_bt = false;
    let mut cur_x: f32 = 0.0;
    let mut cur_y: f32 = 0.0;
    let mut base_x: f32 = 0.0;
    let mut base_y: f32 = 0.0;
    let mut font_size: f32 = 12.0;
    let mut block_text = String::new();
    let mut block_start_x: f32 = 0.0;
    let mut block_start_y: f32 = 0.0;

    for op in operations {
        match op.operator.as_str() {
            "BT" => {
                in_bt = true;
                block_text.clear();
                cur_x = 0.0; cur_y = 0.0;
                base_x = 0.0; base_y = 0.0;
            }
            "ET" => {
                if in_bt && !block_text.trim().is_empty() {
                    blocks.push(TextBlock {
                        x: block_start_x,
                        y: block_start_y,
                        font_size,
                        text: block_text.trim().to_string(),
                    });
                }
                in_bt = false;
            }
            "Tm" if in_bt => {
                if op.operands.len() >= 6 {
                    cur_x = op.operands[4].as_f64().unwrap_or(0.0) as f32;
                    cur_y = op.operands[5].as_f64().unwrap_or(0.0) as f32;
                    base_x = cur_x; base_y = cur_y;
                    if block_text.is_empty() {
                        block_start_x = cur_x;
                        block_start_y = cur_y;
                    }
                }
            }
            "Td" | "TD" if in_bt => {
                if op.operands.len() >= 2 {
                    let dx = op.operands[0].as_f64().unwrap_or(0.0) as f32;
                    let dy = op.operands[1].as_f64().unwrap_or(0.0) as f32;
                    base_x += dx; base_y += dy;
                    cur_x = base_x; cur_y = base_y;
                    if block_text.is_empty() {
                        block_start_x = cur_x;
                        block_start_y = cur_y;
                    }
                }
            }
            "T*" if in_bt => {
                block_text.push('\n');
            }
            "Tf" if in_bt => {
                if op.operands.len() >= 2 {
                    font_size = op.operands[1].as_f64().unwrap_or(12.0) as f32;
                }
            }
            "Tj" if in_bt => {
                if let Some(lopdf::Object::String(bytes, _)) = op.operands.first() {
                    // 尝试 UTF-16BE 解码，回退 Latin-1
                    let decoded = decode_pdf_string(bytes);
                    block_text.push_str(&decoded);
                }
            }
            "TJ" if in_bt => {
                if let Some(lopdf::Object::Array(arr)) = op.operands.first() {
                    for item in arr {
                        if let lopdf::Object::String(bytes, _) = item {
                            let decoded = decode_pdf_string(bytes);
                            block_text.push_str(&decoded);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    blocks
}

fn decode_pdf_string(bytes: &[u8]) -> String {
    // 检测 UTF-16BE BOM (0xFE 0xFF)
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let utf16: Vec<u16> = bytes[2..]
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&utf16).to_string();
    }
    // 尝试 UTF-8
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    // 回退 Latin-1
    bytes.iter().map(|&b| b as char).collect()
}

/// 构建翻译后的 PDF 并保存为 {原文件名}_zh.pdf
/// translations: 按页序的中文译文
#[tauri::command]
pub fn build_translated_pdf(
    path: String,
    translations: Vec<String>,
) -> Result<String, String> {
    use lopdf::content::Content;

    let mut doc = lopdf::Document::load(&path)
        .map_err(|e| format!("打开 PDF 失败: {}", e))?;

    // 添加 Microsoft YaHei 字体到文档
    let font_id = add_yahe_font(&mut doc);

    let page_ids: Vec<lopdf::ObjectId> = doc.page_iter().collect();
    let total_pages = page_ids.len();

    for (page_idx, &page_id) in page_ids.iter().enumerate() {
        let translation = match translations.get(page_idx) {
            Some(t) if !t.trim().is_empty() => t.clone(),
            _ => continue,
        };

        // 获取页面尺寸（用于自动换行计算）
        let page_width: f32 = doc
            .get_object(page_id)
            .ok()
            .and_then(|o| o.as_dict().ok())
            .and_then(|d| d.get(b"MediaBox").ok())
            .and_then(|o| o.as_array().ok())
            .and_then(|arr| arr.get(2))
            .and_then(|o| o.as_f64().ok())
            .unwrap_or(595.0) as f32; // A4 默认宽

        // 读取内容流
        let content_data = match doc.get_page_content(page_id) {
            Ok(d) => d,
            Err(_) => continue,
        };

        // 解析操作序列
        let content = match Content::decode(&content_data) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // 提取文字块（位置 + 文字）
        let text_blocks = extract_text_blocks(&content.operations);
        if text_blocks.is_empty() {
            continue;
        }

        // 计算整页文字的边界（用于单次覆盖整个文字区域）
        let min_x = text_blocks.iter().map(|b| b.x).fold(f32::MAX, f32::min);
        let max_x = text_blocks.iter().map(|b| b.x).fold(f32::MIN, f32::max);
        let min_y = text_blocks.iter().map(|b| b.y).fold(f32::MAX, f32::min);
        let max_y = text_blocks.iter().map(|b| b.y).fold(f32::MIN, f32::max);

        let first_block = &text_blocks[0];
        let font_size = text_blocks.iter().map(|b| b.font_size).fold(0.0f32, f32::max).max(10.0);

        // 使用更高边距的区域高度（每行高度 * 行数估算）
        let text_width = (max_x - min_x).max(200.0);
        let text_height = (max_y - min_y + font_size * 2.0).max(font_size * 3.0);

        // 构建新的覆盖+翻译操作
        let mut extra_ops: Vec<lopdf::content::Operation> = Vec::new();

        // 白色矩形覆盖原文字区域
        extra_ops.extend(make_white_rect_ops(
            min_x,
            min_y - font_size,
            text_width + page_width * 0.1, // 延伸到右边距
            text_height + font_size * 2.0,
        ));

        // 在原文字起始位置写入翻译文字
        extra_ops.extend(make_translated_text_ops(
            first_block.x,
            max_y, // 从顶部开始向下写
            font_size,
            &translation,
            page_width,
        ));

        // 将新操作追加到原内容流末尾
        let mut all_ops = content.operations;
        all_ops.extend(extra_ops);
        let new_content = Content { operations: all_ops };
        let new_bytes = new_content.encode()
            .map_err(|e| format!("编码内容流失败 (第{}页): {}", page_idx + 1, e))?;

        doc.change_page_content(page_id, new_bytes)
            .map_err(|e| format!("写入内容流失败 (第{}页): {}", page_idx + 1, e))?;

        // 把 ZhF0 字体加入本页 /Resources/Font 字典
        if let Ok(page_obj) = doc.get_object_mut(page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                let fonts_dict = if let Ok(res) = dict.get_mut(b"Resources") {
                    if let Ok(res_dict) = res.as_dict_mut() {
                        if !res_dict.has(b"Font") {
                            res_dict.set("Font", lopdf::Object::Dictionary(lopdf::Dictionary::new()));
                        }
                        res_dict.get_mut(b"Font").ok().and_then(|f| f.as_dict_mut().ok())
                    } else { None }
                } else { None };
                if let Some(fonts) = fonts_dict {
                    fonts.set("ZhF0", lopdf::Object::Reference(font_id));
                }
            }
        }
    }

    // 生成输出路径：{stem}_zh.pdf
    let input_path = std::path::Path::new(&path);
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("translated");
    let output_path = input_path
        .with_file_name(format!("{}_zh.pdf", stem))
        .to_str()
        .ok_or("输出路径生成失败")?
        .to_string();

    doc.save(&output_path)
        .map_err(|e| format!("保存 PDF 失败: {}", e))?;

    Ok(output_path)
}
```

**Step 2: 在 lib.rs 注册命令**

```rust
            commands::build_translated_pdf,
```

**Step 3: 验证编译**

```bash
cd src-tauri && cargo check 2>&1 | grep -E "^error" | head -20
```

预期：无 error。如有类型或方法错误按提示修正。

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add build_translated_pdf command with lopdf content stream manipulation"
```

---

## Task 5: 添加 i18n 字符串

**Files:**
- Modify: `src/locales/zh-CN.ts`
- Modify: `src/locales/en.ts`

**Step 1: zh-CN.ts — 在 `fileDetail` 对象末尾（`confirmDelete` 后）添加**

```typescript
    translatePdf: '翻译 PDF',
    translatePdfProgress: '翻译第 {current}/{total} 页...',
    translatePdfBuilding: '生成译文文件...',
    translatePdfDone: '翻译完成',
    translatePdfOpen: '打开副本',
    translatePdfNoKey: '请先在设置 → 翻译中配置 Gemini API Key',
    translatePdfScanned: '未检测到可翻译文字（扫描版 PDF 暂不支持）',
    translatePdfError: '翻译失败',
```

**Step 2: en.ts — 在 `fileDetail` 对象末尾添加**

```typescript
    translatePdf: 'Translate PDF',
    translatePdfProgress: 'Translating page {current}/{total}...',
    translatePdfBuilding: 'Building translated file...',
    translatePdfDone: 'Translation complete',
    translatePdfOpen: 'Open copy',
    translatePdfNoKey: 'Please configure your Gemini API Key in Settings → Translation',
    translatePdfScanned: 'No translatable text detected (scanned PDFs not supported)',
    translatePdfError: 'Translation failed',
```

**Step 3: Commit**

```bash
git add src/locales/zh-CN.ts src/locales/en.ts
git commit -m "feat: add i18n keys for PDF translation feature"
```

---

## Task 6: 前端 — FileDetailSidebar 接入翻译逻辑和 UI

**Files:**
- Modify: `src/components/FileDetailSidebar.vue`

### Step 1: `<script setup>` 顶部追加 import

在现有 import 块末尾（`import ImageViewer from './ImageViewer.vue'` 之后）添加：

```typescript
import { useSettings } from '../composables/useSettings'
```

### Step 2: 在 `const { t } = useI18n()` 之后添加 PDF 翻译状态和逻辑

```typescript
// ─── PDF 翻译 ────────────────────────────────────────

const { loadSettings } = useSettings()

type PdfTranslateState = 'idle' | 'extracting' | 'translating' | 'building' | 'done' | 'error'
const pdfTranslateState = ref<PdfTranslateState>('idle')
const pdfTranslateProgress = ref({ current: 0, total: 0 })
const pdfTranslateError = ref('')
const pdfOutputPath = ref('')

// 切换文件时重置翻译状态
watch(() => props.file, () => {
  pdfTranslateState.value = 'idle'
  pdfTranslateError.value = ''
  pdfOutputPath.value = ''
  pdfTranslateProgress.value = { current: 0, total: 0 }
})

async function handleTranslatePdf() {
  if (!props.file || pdfTranslateState.value !== 'idle') return

  const settings = await loadSettings()
  if (!settings?.translation?.apiKey) {
    pdfTranslateState.value = 'error'
    pdfTranslateError.value = t('fileDetail.translatePdfNoKey')
    return
  }

  const { apiKey, model } = settings.translation

  try {
    // Step 1: 提取文字
    pdfTranslateState.value = 'extracting'
    const pages = await invoke<string[]>('extract_pdf_pages_text', { path: props.file.path })

    pdfTranslateProgress.value = { current: 0, total: pages.length }

    // Step 2: 逐页翻译
    pdfTranslateState.value = 'translating'
    const translations: string[] = []

    for (let i = 0; i < pages.length; i++) {
      pdfTranslateProgress.value.current = i + 1
      const translated = await invoke<string>('translate_text_once', {
        apiKey,
        model,
        text: pages[i],
      })
      translations.push(translated)
    }

    // Step 3: 生成 PDF
    pdfTranslateState.value = 'building'
    const outputPath = await invoke<string>('build_translated_pdf', {
      path: props.file.path,
      translations,
    })

    pdfOutputPath.value = outputPath
    pdfTranslateState.value = 'done'
  } catch (e: any) {
    pdfTranslateState.value = 'error'
    pdfTranslateError.value = String(e).includes('扫描版')
      ? t('fileDetail.translatePdfScanned')
      : `${t('fileDetail.translatePdfError')}: ${e}`
    console.error('PDF 翻译失败:', e)
  }
}

async function openPdfCopy() {
  if (!pdfOutputPath.value) return
  try {
    await invoke('open_file', { path: pdfOutputPath.value })
  } catch (e) {
    console.error('打开副本失败:', e)
  }
}
```

### Step 3: 在 template 中 PDF 预览区块下方添加翻译 section

在 `<!-- 基本信息（文本类不显示） -->` 那行（约第 539 行）之前，PDF 的 `</div>` 后面插入：

```html
          <!-- PDF 翻译区块 -->
          <div v-if="fileType === 'pdf'" class="sidebar-section pdf-translate-section">
            <!-- idle：显示翻译按钮 -->
            <template v-if="pdfTranslateState === 'idle'">
              <button class="pdf-translate-btn" @click="handleTranslatePdf">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <circle cx="12" cy="12" r="10"/>
                  <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
                  <line x1="2" y1="12" x2="22" y2="12"/>
                </svg>
                {{ $t('fileDetail.translatePdf') }}
              </button>
            </template>

            <!-- 进行中：提取 / 翻译 / 生成 -->
            <template v-else-if="['extracting', 'translating', 'building'].includes(pdfTranslateState)">
              <div class="pdf-translate-progress">
                <span class="pdf-translate-spinner"/>
                <span v-if="pdfTranslateState === 'extracting'">{{ $t('fileDetail.loading') }}</span>
                <span v-else-if="pdfTranslateState === 'translating'">
                  {{ $t('fileDetail.translatePdfProgress', pdfTranslateProgress) }}
                </span>
                <span v-else>{{ $t('fileDetail.translatePdfBuilding') }}</span>
              </div>
            </template>

            <!-- 完成 -->
            <template v-else-if="pdfTranslateState === 'done'">
              <div class="pdf-translate-done">
                <span class="pdf-translate-done-label">{{ $t('fileDetail.translatePdfDone') }}</span>
                <button class="pdf-translate-open-btn" @click="openPdfCopy">
                  {{ $t('fileDetail.translatePdfOpen') }}
                </button>
                <button class="pdf-translate-reset-btn" @click="pdfTranslateState = 'idle'">↺</button>
              </div>
            </template>

            <!-- 出错 -->
            <template v-else-if="pdfTranslateState === 'error'">
              <div class="pdf-translate-error">
                <span class="pdf-translate-error-msg">{{ pdfTranslateError }}</span>
                <button class="pdf-translate-reset-btn" @click="pdfTranslateState = 'idle'">↺</button>
              </div>
            </template>
          </div>
```

### Step 4: 在 `<style>` 块中添加 PDF 翻译样式

在 `/* ─── PDF 预览 ─── */` 样式块（约 872 行）之后插入：

```css
/* ─── PDF 翻译区块 ─── */
.pdf-translate-section {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-3);
  border-top: var(--glass-border);
}

.pdf-translate-btn {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  width: 100%;
  padding: var(--spacing-2) var(--spacing-3);
  background: transparent;
  border: 1px solid var(--border-light);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
  cursor: pointer;
  transition: background var(--duration-fast) var(--ease-out),
              color var(--duration-fast) var(--ease-out),
              border-color var(--duration-fast) var(--ease-out);
}

.pdf-translate-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
  border-color: var(--border-medium);
}

.pdf-translate-progress {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
}

.pdf-translate-spinner {
  display: inline-block;
  width: 12px;
  height: 12px;
  border: 2px solid var(--border-medium);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  flex-shrink: 0;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.pdf-translate-done {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  font-size: var(--font-size-sm);
}

.pdf-translate-done-label {
  color: var(--color-success);
  flex: 1;
}

.pdf-translate-open-btn {
  padding: var(--spacing-1) var(--spacing-3);
  background: var(--color-primary);
  color: white;
  border: none;
  border-radius: var(--radius-sm);
  font-size: var(--font-size-sm);
  cursor: pointer;
  transition: opacity var(--duration-fast);
}

.pdf-translate-open-btn:hover { opacity: 0.85; }

.pdf-translate-reset-btn {
  padding: var(--spacing-1) var(--spacing-2);
  background: transparent;
  border: 1px solid var(--border-light);
  border-radius: var(--radius-sm);
  color: var(--text-tertiary);
  font-size: var(--font-size-sm);
  cursor: pointer;
  transition: color var(--duration-fast);
}

.pdf-translate-reset-btn:hover { color: var(--text-primary); }

.pdf-translate-error {
  display: flex;
  align-items: flex-start;
  gap: var(--spacing-2);
  font-size: var(--font-size-sm);
}

.pdf-translate-error-msg {
  color: var(--color-danger);
  flex: 1;
  line-height: 1.4;
}
```

**Step 5: 验证前端类型检查**

```bash
cd "C:/work/PG Butler/PGB1" && npx vue-tsc --noEmit 2>&1 | head -20
```

预期：无 error

**Step 6: Commit**

```bash
git add src/components/FileDetailSidebar.vue
git commit -m "feat: add PDF translation UI in FileDetailSidebar"
```

---

## Task 7: 联调验证

**Step 1: 启动开发模式**

```bash
cd "C:/work/PG Butler/PGB1" && npm run tauri dev
```

**Step 2: 功能验证清单**

- [ ] 打开一个有文字的 PDF 文件，侧边栏底部显示"翻译 PDF"按钮
- [ ] 未配置 API Key 时：点击按钮 → 显示红色错误提示 → ↺ 重置回 idle
- [ ] 配置 API Key 后：点击 → 显示进度 → 翻译第 N/总页 → 生成中 → 完成
- [ ] 完成后点击"打开副本" → 用默认 PDF 查看器打开 `{name}_zh.pdf`
- [ ] 切换到其他文件 → 翻译状态重置为 idle
- [ ] 扫描版 PDF（无文字层）→ 显示对应错误提示
- [ ] `{name}_zh.pdf` 内容验证：图片完整保留，文字区域为中文译文

**Step 3: 最终 Commit**

```bash
git add -A
git commit -m "feat: PDF translation - translate text while preserving images, save as _zh.pdf"
```

---

## 已知局限（供产品总监知悉）

1. **扫描版 PDF**：仅有图片层无文字层的 PDF，`pdf-extract` 无法提取文字，会友好提示
2. **字体体积**：引用系统 Microsoft YaHei，输出 PDF 须在 Windows 环境下查看（已 YaHei 安装）；跨平台查看中文字形可能回退到系统默认字体
3. **文字覆盖范围**：白色矩形基于文字块包围框计算，极端情况（文字分布散乱的页面）可能覆盖不完整
4. **大文档速度**：每页独立调用 Gemini API，50 页文档约需 2-5 分钟（受 API 速度影响）
