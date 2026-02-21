# 翻译功能 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现全局快捷键呼出的翻译窗口，调用 Gemini API 完成中英日双语自动检测翻译。

**Architecture:**
- Rust 后端注册 Windows 全局热键（`RegisterHotKey`），收到热键消息后 toggle 翻译窗口 show/hide
- 翻译窗口是独立的 Tauri WebviewWindow（400×500，always_on_top），指向路由 `/translator`
- Gemini API 调用在 Rust 后端通过 `reqwest` 发起（防止前端暴露 API Key）
- 设置存在 AppSettings.translation 中（已有骨架），补充 langA/langB 字段

**Tech Stack:** Tauri 2.x, Vue 3, Rust, Windows API (RegisterHotKey/VK_LAUNCH_APP2), reqwest, Gemini REST API

---

## Task 1：补充 TranslationSettings 数据模型（langA / langB）

**Files:**
- Modify: `src-tauri/src/models.rs`（TranslationSettings struct + Default）
- Modify: `src/composables/useSettings.ts`（TranslationSettings interface）
- Modify: `src/views/SettingsPage.vue`（翻译 Tab 补充语言对选择器 UI + model 选项更新）

**背景：** 设计文档要求 translation 配置含 `langA`/`langB`，当前 struct 缺失。

### Step 1：修改 models.rs，TranslationSettings 新增 lang_a / lang_b

在 `src-tauri/src/models.rs` 的 `TranslationSettings` struct 末尾添加两个字段：

```rust
pub struct TranslationSettings {
    pub api_key: String,
    pub model: String,
    pub shortcut: String,
    pub use_calculator_key: bool,
    pub lang_a: String,  // 新增
    pub lang_b: String,  // 新增
}
```

同时修改 `Default for AppSettings` 中对应部分：

```rust
translation: TranslationSettings {
    api_key: String::new(),
    model: "gemini-2.0-flash".to_string(),  // 同时更新默认模型
    shortcut: "Ctrl+Shift+T".to_string(),
    use_calculator_key: false,
    lang_a: "zh-CN".to_string(),  // 新增
    lang_b: "en".to_string(),     // 新增
},
```

### Step 2：修改 useSettings.ts，同步 TypeScript interface

```typescript
export interface TranslationSettings {
  apiKey: string
  model: string
  shortcut: string
  useCalculatorKey: boolean
  langA: string   // 新增
  langB: string   // 新增
}
```

### Step 3：修改 SettingsPage.vue 翻译 Tab

在翻译 Tab 的 form-group 列表末尾添加默认语言对选择器，同时更新 model 选项与设计文档对齐（gemini-2.5-flash-lite / gemini-2.0-flash / gemini-2.5-pro）：

```html
<!-- 模型选项更新 -->
<select v-model="editSettings.translation.model" class="form-select">
  <option value="gemini-2.5-flash-lite">Gemini 2.5 Flash Lite (推荐)</option>
  <option value="gemini-2.0-flash">Gemini 2.0 Flash</option>
  <option value="gemini-2.5-pro">Gemini 2.5 Pro</option>
</select>

<!-- 新增：默认语言对 -->
<div class="form-group">
  <label class="form-label">默认语言对</label>
  <div class="lang-pair-row">
    <select v-model="editSettings.translation.langA" class="form-select lang-select">
      <option value="zh-CN">中文</option>
      <option value="en">English</option>
      <option value="ja">日本語</option>
    </select>
    <span class="lang-separator">↔</span>
    <select v-model="editSettings.translation.langB" class="form-select lang-select">
      <option value="zh-CN">中文</option>
      <option value="en">English</option>
      <option value="ja">日本語</option>
    </select>
  </div>
</div>
```

在 SettingsPage.vue 的 `<style scoped>` 末尾添加 lang-pair 样式：

```css
.lang-pair-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
.lang-select {
  flex: 1;
}
.lang-separator {
  color: var(--text-secondary);
  font-size: var(--text-lg);
  flex-shrink: 0;
}
```

### Step 4：验证编译

```bash
cd D:/work/pgsoft/PGB1
npm run tauri dev
```

期望：设置页翻译 Tab 出现语言对选择器，模型选项更新，无编译错误。

---

## Task 2：添加 reqwest 依赖 + Gemini 翻译命令

**Files:**
- Modify: `src-tauri/Cargo.toml`（添加 reqwest）
- Modify: `src-tauri/src/commands.rs`（新增 `translate_text` 命令）
- Modify: `src-tauri/src/lib.rs`（注册命令）

**背景：** Gemini API 调用在 Rust 后端，防止前端暴露 API Key。HTTP POST 到 Google 生成式语言 API。

### Step 1：Cargo.toml 添加 reqwest

在 `[dependencies]` 末尾添加：

```toml
reqwest = { version = "0.12", features = ["json"] }
```

### Step 2：commands.rs 末尾添加 translate_text 命令

```rust
#[tauri::command]
pub async fn translate_text(
    api_key: String,
    model: String,
    lang_a: String,
    lang_b: String,
    text: String,
) -> Result<String, String> {
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

    let prompt = format!(
        "You are a translator between {} and {}.\nDetect the input language and translate to the other one.\nTone: concise, friendly, natural — like casual coworker chat. No fluff.\nOnly output the translation, nothing else.\n\n\"{}\"",
        lang_a_display, lang_b_display, text
    );

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model
    );

    let body = serde_json::json!({
        "contents": [{
            "parts": [{ "text": prompt }]
        }]
    });

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("X-goog-api-key", &api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("网络错误：{}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let err_text = response.text().await.unwrap_or_default();
        return Err(format!("API 错误 {}: {}", status, err_text));
    }

    let resp_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败：{}", e))?;

    let translated = resp_json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| "响应格式异常，无法提取翻译结果".to_string())?
        .trim()
        .to_string();

    Ok(translated)
}
```

### Step 3：lib.rs 注册命令

在 `invoke_handler` 的命令列表末尾添加：

```rust
commands::translate_text,
```

### Step 4：验证编译

```bash
cd D:/work/pgsoft/PGB1/src-tauri
cargo build 2>&1 | tail -5
```

期望：编译成功，无错误。首次可能需要下载 reqwest，耗时约 1~2 分钟。

---

## Task 3：创建翻译窗口页面 TranslatorPage.vue

**Files:**
- Create: `src/views/TranslatorPage.vue`
- Modify: `src/router/index.ts`（添加 `/translator` 路由）

**背景：** 翻译窗口是独立的 400×500 WebviewWindow，加载 `/translator` 路由。布局：标题栏 + 输入区 + 翻译按钮 + 输出区 + 底部语言对选择器。

### Step 1：创建 TranslatorPage.vue

完整文件内容：

```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useSettings } from '../composables/useSettings'

const { loadSettings } = useSettings()

const inputText = ref('')
const outputText = ref('')
const isTranslating = ref(false)
const copyFeedback = ref(false)
const langA = ref('zh-CN')
const langB = ref('en')

const LANG_LABELS: Record<string, string> = {
  'zh-CN': '中文',
  'en': 'English',
  'ja': '日本語',
}

onMounted(async () => {
  // 加载设置中的默认语言对
  const settings = await loadSettings()
  if (settings) {
    langA.value = settings.translation.langA || 'zh-CN'
    langB.value = settings.translation.langB || 'en'
  }
})

async function handleTranslate() {
  if (isTranslating.value || !inputText.value.trim()) return

  const settings = await loadSettings()
  if (!settings) {
    outputText.value = '加载设置失败，请重试'
    return
  }

  isTranslating.value = true
  outputText.value = '翻译中...'

  try {
    const result = await invoke<string>('translate_text', {
      apiKey: settings.translation.apiKey,
      model: settings.translation.model,
      langA: langA.value,
      langB: langB.value,
      text: inputText.value,
    })
    outputText.value = result
  } catch (e) {
    outputText.value = String(e)
  } finally {
    isTranslating.value = false
  }
}

async function handlePaste() {
  try {
    const text = await readText()
    if (text) inputText.value = text
  } catch (e) {
    console.error('读取剪贴板失败:', e)
  }
}

async function handleCopy() {
  if (!outputText.value) return
  try {
    await writeText(outputText.value)
    copyFeedback.value = true
    setTimeout(() => { copyFeedback.value = false }, 1500)
  } catch (e) {
    console.error('复制失败:', e)
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.ctrlKey && e.key === 'Enter') {
    e.preventDefault()
    handleTranslate()
  }
}

async function hideWindow() {
  const win = getCurrentWindow()
  await win.hide()
}
</script>

<template>
  <div class="translator-window" data-tauri-drag-region>
    <!-- 标题栏 -->
    <div class="translator-titlebar" data-tauri-drag-region>
      <span class="translator-title" data-tauri-drag-region>翻译</span>
      <div class="translator-controls">
        <button class="win-btn" @click="hideWindow" title="隐藏">_</button>
        <button class="win-btn win-btn-close" @click="hideWindow" title="关闭">×</button>
      </div>
    </div>

    <!-- 主内容 -->
    <div class="translator-body">
      <!-- 输入区 -->
      <div class="translator-box">
        <textarea
          v-model="inputText"
          class="translator-textarea"
          placeholder="请输入文本..."
          @keydown="handleKeydown"
        />
        <button class="box-action-btn" @click="handlePaste" title="粘贴剪贴板内容">📋</button>
      </div>

      <!-- 翻译按钮 -->
      <div class="translator-action-row">
        <button
          class="translate-btn"
          :disabled="isTranslating || !inputText.trim()"
          @click="handleTranslate"
        >
          {{ isTranslating ? '翻译中...' : '▶ 翻译' }}
        </button>
      </div>

      <!-- 输出区 -->
      <div class="translator-box">
        <textarea
          v-model="outputText"
          class="translator-textarea translator-output"
          placeholder="译文..."
          readonly
        />
        <button class="box-action-btn" @click="handleCopy" title="复制译文">
          {{ copyFeedback ? '✓' : '📄' }}
        </button>
      </div>
    </div>

    <!-- 底部语言对 -->
    <div class="translator-footer">
      <select v-model="langA" class="lang-select-sm">
        <option value="zh-CN">中文</option>
        <option value="en">English</option>
        <option value="ja">日本語</option>
      </select>
      <span class="lang-sep-icon">↔</span>
      <select v-model="langB" class="lang-select-sm">
        <option value="zh-CN">中文</option>
        <option value="en">English</option>
        <option value="ja">日本語</option>
      </select>
    </div>
  </div>
</template>

<style scoped>
.translator-window {
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--glass-bg-strong);
  backdrop-filter: blur(24px) saturate(1.8);
  -webkit-backdrop-filter: blur(24px) saturate(1.8);
  border: 1px solid var(--glass-border);
  border-radius: var(--radius-lg);
  overflow: hidden;
  color: var(--text-primary);
  font-family: var(--font-base);
  user-select: none;
}

/* 标题栏 */
.translator-titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 var(--space-3);
  height: 40px;
  background: var(--glass-bg-medium);
  border-bottom: 1px solid var(--glass-border);
  flex-shrink: 0;
}
.translator-title {
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--text-primary);
}
.translator-controls {
  display: flex;
  gap: var(--space-1);
}
.win-btn {
  width: 28px;
  height: 22px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  font-size: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background var(--transition-fast);
}
.win-btn:hover {
  background: var(--glass-bg-strong);
  color: var(--text-primary);
}
.win-btn-close:hover {
  background: var(--color-danger);
  color: white;
}

/* 主体 */
.translator-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: var(--space-3);
  gap: var(--space-2);
  overflow: hidden;
  min-height: 0;
}

.translator-box {
  flex: 1;
  position: relative;
  min-height: 0;
}
.translator-textarea {
  width: 100%;
  height: 100%;
  resize: none;
  border: 1px solid var(--glass-border);
  border-radius: var(--radius-md);
  background: var(--glass-bg-subtle);
  color: var(--text-primary);
  font-family: var(--font-base);
  font-size: var(--text-sm);
  padding: var(--space-2) var(--space-3);
  padding-right: 36px;
  box-sizing: border-box;
  outline: none;
  transition: border-color var(--transition-fast);
  user-select: text;
}
.translator-textarea:focus {
  border-color: var(--color-primary);
}
.translator-output {
  color: var(--text-secondary);
}
.box-action-btn {
  position: absolute;
  top: var(--space-2);
  right: var(--space-2);
  width: 28px;
  height: 28px;
  border: none;
  border-radius: var(--radius-sm);
  background: var(--glass-bg-medium);
  cursor: pointer;
  font-size: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background var(--transition-fast);
}
.box-action-btn:hover {
  background: var(--glass-bg-strong);
}

/* 翻译按钮行 */
.translator-action-row {
  display: flex;
  justify-content: center;
  flex-shrink: 0;
}
.translate-btn {
  padding: var(--space-1) var(--space-5);
  border: none;
  border-radius: var(--radius-md);
  background: var(--color-primary);
  color: white;
  font-family: var(--font-base);
  font-size: var(--text-sm);
  font-weight: 600;
  cursor: pointer;
  transition: opacity var(--transition-fast), background var(--transition-fast);
}
.translate-btn:hover:not(:disabled) {
  opacity: 0.9;
}
.translate-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

/* 底部语言选择 */
.translator-footer {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-top: 1px solid var(--glass-border);
  background: var(--glass-bg-medium);
  flex-shrink: 0;
}
.lang-select-sm {
  flex: 1;
  height: 30px;
  border: 1px solid var(--glass-border);
  border-radius: var(--radius-sm);
  background: var(--glass-bg-subtle);
  color: var(--text-primary);
  font-family: var(--font-base);
  font-size: var(--text-xs);
  padding: 0 var(--space-2);
  outline: none;
  cursor: pointer;
}
.lang-sep-icon {
  color: var(--text-tertiary);
  font-size: var(--text-base);
  flex-shrink: 0;
}
</style>
```

### Step 2：router/index.ts 添加路由

在路由列表末尾（settings 路由后）添加：

```typescript
{
  path: '/translator',
  name: 'translator',
  component: () => import('../views/TranslatorPage.vue'),
},
```

### Step 3：安装 clipboard-manager 插件

翻译窗口需要读写剪贴板，使用 Tauri 官方插件：

```bash
cd D:/work/pgsoft/PGB1
npm install @tauri-apps/plugin-clipboard-manager
```

然后在 `src-tauri/Cargo.toml` 的 `[dependencies]` 末尾添加：

```toml
tauri-plugin-clipboard-manager = "2"
```

在 `src-tauri/src/lib.rs` 的 `.plugin(...)` 链中添加：

```rust
.plugin(tauri_plugin_clipboard_manager::init())
```

---

## Task 4：Rust 后端实现全局快捷键 + 翻译窗口管理

**Files:**
- Create: `src-tauri/src/hotkey.rs`（全局快捷键模块）
- Modify: `src-tauri/src/lib.rs`（mod hotkey + 注册命令 + setup 初始化）
- Modify: `src-tauri/src/commands.rs`（新增 `toggle_translator_window` 命令，供前端手动呼出备用）

**背景：**
- Windows `RegisterHotKey` API 要求在同一线程消息循环中处理 `WM_HOTKEY`
- 使用独立线程运行消息循环，收到热键事件后通过 Tauri `AppHandle` 操作窗口
- 翻译窗口（label: `"translator"`）通过 `WebviewWindowBuilder` 动态创建，首次创建，后续 show/hide
- tauri.conf.json 不预定义翻译窗口（动态创建）

### Step 1：在 Cargo.toml windows features 中补充 Win32_UI_Shell_Common

修改 `windows` 依赖，添加 `Win32_UI_Shell` 相关特性（RegisterHotKey 需要）：

```toml
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_UI_WindowsAndMessaging",
  "Win32_System_Threading",
] }
```

**注意：** `RegisterHotKey` 已在 `Win32_UI_WindowsAndMessaging` 中，无需额外特性，保持现有 features 不变。

### Step 2：创建 src-tauri/src/hotkey.rs

```rust
use std::sync::OnceLock;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::UI::WindowsAndMessaging::{
    RegisterHotKey, UnregisterHotKey, GetMessageW, MSG,
    MOD_CONTROL, MOD_SHIFT, WM_HOTKEY,
    VK_LAUNCH_APP2,
};
use windows::Win32::Foundation::HWND;

const HOTKEY_ID_SHORTCUT: i32 = 1001;
const HOTKEY_ID_CALCULATOR: i32 = 1002;
const TRANSLATOR_WINDOW_LABEL: &str = "translator";

/// 在独立线程中运行全局快捷键消息循环
/// settings 在 setup 时传入（快捷键字符串 + 是否使用计算器键）
pub fn start_hotkey_listener(app_handle: AppHandle, shortcut: String, use_calculator_key: bool) {
    std::thread::spawn(move || {
        unsafe {
            // 注册热键
            if use_calculator_key {
                // VK_LAUNCH_APP2 = 0xB7（计算器键），无修饰键
                let _ = RegisterHotKey(HWND(0), HOTKEY_ID_CALCULATOR, Default::default(), 0xB7);
            } else {
                // 解析 shortcut 字符串，支持 "Ctrl+Shift+T" 格式
                if let Some((modifiers, vk)) = parse_shortcut(&shortcut) {
                    let _ = RegisterHotKey(HWND(0), HOTKEY_ID_SHORTCUT, modifiers, vk as u32);
                }
            }

            // 消息循环
            let mut msg = MSG::default();
            loop {
                let result = GetMessageW(&mut msg, HWND(0), 0, 0);
                if result.0 == 0 || result.0 == -1 {
                    break;
                }
                if msg.message == WM_HOTKEY {
                    toggle_translator_window(&app_handle);
                }
            }

            // 清理
            let _ = UnregisterHotKey(HWND(0), HOTKEY_ID_SHORTCUT);
            let _ = UnregisterHotKey(HWND(0), HOTKEY_ID_CALCULATOR);
        }
    });
}

fn toggle_translator_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(TRANSLATOR_WINDOW_LABEL) {
        // 已存在：toggle show/hide
        let is_visible = win.is_visible().unwrap_or(false);
        if is_visible {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.set_focus();
        }
    } else {
        // 首次创建翻译窗口
        let dev_url = "http://localhost:1420/translator";
        let prod_url = "index.html";

        // 判断是开发模式还是生产模式
        #[cfg(debug_assertions)]
        let url_str = dev_url;
        #[cfg(not(debug_assertions))]
        let url_str = prod_url;

        let url = WebviewUrl::App(format!("{}", url_str).into());

        if let Ok(win) = WebviewWindowBuilder::new(app, TRANSLATOR_WINDOW_LABEL, url)
            .title("翻译")
            .inner_size(400.0, 500.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .center()
            .build()
        {
            #[cfg(target_os = "windows")]
            {
                use window_vibrancy::apply_acrylic;
                let _ = apply_acrylic(&win, Some((0, 0, 0, 1)));
            }
            let _ = win.show();
        }
    }
}

/// 解析快捷键字符串，返回 (HOTMODIFIERS, 虚拟键码)
/// 支持格式：Ctrl+Shift+T、Control+Shift+T、Ctrl+Alt+T
fn parse_shortcut(s: &str) -> Option<(windows::Win32::UI::WindowsAndMessaging::HOT_KEY_MODIFIERS, u16)> {
    use windows::Win32::UI::WindowsAndMessaging::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, HOT_KEY_MODIFIERS};

    let parts: Vec<&str> = s.split('+').collect();
    if parts.len() < 2 {
        return None;
    }

    let key_char = parts.last()?.trim();
    let modifiers_parts = &parts[..parts.len() - 1];

    let mut modifiers = HOT_KEY_MODIFIERS(0);
    for part in modifiers_parts {
        match part.trim().to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "shift" => modifiers |= MOD_SHIFT,
            "alt" => modifiers |= MOD_ALT,
            _ => {}
        }
    }

    // 字符转虚拟键码（仅支持 A-Z）
    let vk = if key_char.len() == 1 {
        let c = key_char.chars().next()?.to_ascii_uppercase();
        if c.is_ascii_alphabetic() {
            c as u16
        } else {
            return None;
        }
    } else {
        return None;
    };

    Some((modifiers, vk))
}
```

### Step 3：lib.rs 集成 hotkey 模块

在 `lib.rs` 顶部添加：
```rust
mod hotkey;
```

在 `invoke_handler` 添加命令（Task 2 已处理 translate_text，这里补充 toggle 命令）：
```rust
commands::toggle_translator_window,
```

在 `.setup(|app| {` 闭包末尾（`Ok(())` 之前）添加热键初始化：

```rust
// 初始化全局快捷键（翻译窗口）
let hotkey_app = app.handle().clone();
tauri::async_runtime::spawn(async move {
    // 加载设置获取快捷键配置
    let settings_path = match hotkey_app.path().app_config_dir() {
        Ok(dir) => dir.join("app_settings.json"),
        Err(_) => return,
    };
    let settings: models::AppSettings = if settings_path.exists() {
        std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        models::AppSettings::default()
    };
    hotkey::start_hotkey_listener(
        hotkey_app,
        settings.translation.shortcut.clone(),
        settings.translation.use_calculator_key,
    );
});
```

### Step 4：commands.rs 添加 toggle_translator_window 命令（前端手动触发备用）

```rust
#[tauri::command]
pub async fn toggle_translator_window(app: tauri::AppHandle) -> Result<(), String> {
    // 复用 hotkey.rs 中的逻辑，但 hotkey.rs 是 pub(crate)
    // 这里直接内联窗口 toggle 逻辑
    let label = "translator";
    if let Some(win) = app.get_webview_window(label) {
        let is_visible = win.is_visible().map_err(|e| e.to_string())?;
        if is_visible {
            win.hide().map_err(|e| e.to_string())?;
        } else {
            win.show().map_err(|e| e.to_string())?;
            win.set_focus().map_err(|e| e.to_string())?;
        }
    }
    // 如果窗口不存在，不在这里创建（热键线程负责创建）
    Ok(())
}
```

### Step 5：验证编译

```bash
cd D:/work/pgsoft/PGB1/src-tauri
cargo build 2>&1 | tail -10
```

期望：编译成功。

---

## Task 5：翻译窗口开发模式 URL 修正 + 端到端测试

**Files:**
- Modify: `src-tauri/src/hotkey.rs`（URL 构造修正，使用 Tauri 的 devUrl 配置）

**背景：** 开发模式下 Tauri 使用 `devUrl = http://localhost:1420`，翻译页面路由是 `/translator`，需要正确拼接。生产模式用 `WebviewUrl::App("index.html".into())`，由前端路由处理。

### Step 1：修正 hotkey.rs 中的窗口创建 URL

将 hotkey.rs 的 `toggle_translator_window` 函数中 URL 构建部分替换为：

```rust
#[cfg(debug_assertions)]
let url = WebviewUrl::External("http://localhost:1420/translator".parse().unwrap());
#[cfg(not(debug_assertions))]
let url = WebviewUrl::App("index.html".into());
```

### Step 2：端到端手动测试

启动 dev 模式：
```bash
cd D:/work/pgsoft/PGB1
npm run tauri dev
```

测试清单：
1. **快捷键唤起**：按 `Ctrl+Shift+T` → 翻译窗口弹出（400×500，毛玻璃，置顶）
2. **再次按快捷键**：窗口隐藏
3. **再次按快捷键**：窗口重新显示，保留上次内容
4. **点击 [×] 或 [_]**：窗口隐藏（不退出程序）
5. **粘贴功能**：点击 📋 → 剪贴板内容填入输入框
6. **翻译功能**：输入中文 → 点击翻译 → 输出英文（需 API Key 配置）
7. **复制功能**：点击 📄 → 输出内容复制到剪贴板，按钮变 ✓
8. **Ctrl+Enter**：在输入框按 Ctrl+Enter → 触发翻译
9. **语言切换**：底部切换语言对 → 翻译方向变化
10. **API Key 未设置**：输出框显示"请先在设置中配置 Gemini API Key"

---

## Task 6：更新 CODE_INDEX.md

**Files:**
- Modify: `CODE_INDEX.md`

更新内容：
- 文件统计（views +1，src-tauri/src +1）
- 新增 `TranslatorPage.vue` 描述
- 新增 `hotkey.rs` 描述
- 更新 commands.rs 行数和命令列表（translate_text, toggle_translator_window）
- 更新 lib.rs 行数
- 更新 models.rs（TranslationSettings 新增 langA/langB）
- 更新 Cargo.toml（reqwest, tauri-plugin-clipboard-manager）

---

## 注意事项

### 关键架构约束

1. **AppSettings 文件路径**：在 commands.rs 的 `load_settings` 里确认实际路径（通常是 `app_config_dir/app_settings.json`），hotkey.rs 初始化时用同一路径读取。

2. **热键线程生命周期**：`start_hotkey_listener` 启动的线程运行整个应用生命周期，无需手动关闭（随进程退出）。

3. **设置保存后需重启生效**：快捷键变更后需重启 PGB1 才生效（不实现运行时动态重注册，避免过度工程化）。可在设置保存成功提示里补充"快捷键变更后重启生效"说明。

4. **计算器键排他**：当 `useCalculatorKey = true` 时，只注册 VK_LAUNCH_APP2，忽略 shortcut 字符串。

5. **TranslatorPage 使用 useSettings 单例**：页面加载时从 useSettings 读取 langA/langB，窗口内的语言切换是临时状态，不自动保存到设置。

### 潜在问题预警

- `RegisterHotKey` 失败时（快捷键已被其他程序占用）：静默失败，不影响主程序启动。
- Gemini API Key 含特殊字符：reqwest 的 header 会安全处理，无问题。
- `tauri-plugin-clipboard-manager` 需要在 `tauri.conf.json` 的 permissions 中授权（Tauri 2.x）：需要添加 `"clipboard-manager:allow-read-text"` 和 `"clipboard-manager:allow-write-text"`。
