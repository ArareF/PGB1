# 快捷方式栏（第一轮）实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现左侧快捷方式栏的核心功能：添加/编辑/删除快捷方式，单击启动，长按编辑，数据持久化。图标提取和拖拽排序留后续。

**Architecture:**
- 后端：`models.rs` 新增 Shortcut 数据模型，`commands.rs` 新增 3 个命令（load/save/launch），数据存 `shortcuts.json`
- 前端：改造 `Sidebar.vue`（渲染列表+长按交互），新增 `ShortcutDialog.vue`（添加/编辑弹窗）
- 无新依赖，复用 `tauri-plugin-dialog`（文件选择）和 `tauri-plugin-opener`（打开URL）

**Tech Stack:** Tauri 2.x, Rust, Vue 3, 现有插件

---

## Task 1: 后端 — 数据模型

**Files:**
- Modify: `src-tauri/src/models.rs`

**改动：** 在文件末尾（`use std::collections::HashMap;` 之前）追加：

```rust
// ─── 快捷方式栏 ─────────────────────────────────────────────

/// 快捷方式类型
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutType {
    /// 应用程序（.exe）
    App,
    /// 文件夹
    Folder,
    /// 网页 URL
    Web,
}

/// 单个快捷方式
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Shortcut {
    /// 唯一 ID（UUID）
    pub id: String,
    /// 类型
    pub shortcut_type: ShortcutType,
    /// 用户自定义名称
    pub name: String,
    /// 路径（应用/文件夹）或 URL（网页）
    pub path: String,
    /// 图标缓存路径（相对于 app_config_dir/shortcut_icons/），null 表示使用默认图标
    pub icon_cache: Option<String>,
    /// 排序序号
    pub order: u32,
}

/// 快捷方式配置文件结构
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ShortcutsConfig {
    pub shortcuts: Vec<Shortcut>,
}
```

---

## Task 2: 后端 — 命令实现

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1：** 在 `commands.rs` 的 import 区域追加 `Shortcut, ShortcutsConfig, ShortcutType`。

**Step 2：** 在 `commands.rs` 末尾追加 3 个命令：

```rust
// ─── 快捷方式栏 ─────────────────────────────────────────────

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
```

**Step 3：** 在 `lib.rs` 的 `invoke_handler` 中注册 3 个命令：
```
commands::load_shortcuts,
commands::save_shortcuts,
commands::launch_shortcut,
```

---

## Task 3: 前端 — ShortcutDialog.vue（添加/编辑弹窗）

**Files:**
- Create: `src/components/ShortcutDialog.vue`

**功能：**
- Props: `shortcut?`（传入时为编辑模式，不传为添加模式）
- Emits: `save(shortcut)`, `delete(id)`, `cancel`
- 表单：类型单选（应用/文件夹/网页）→ 路径或URL → 名称
- 应用/文件夹：显示「浏览」按钮，调用 `@tauri-apps/plugin-dialog` 的 `open()`
- 网页：隐藏「浏览」按钮，手动输入 URL
- 名称自动填充（应用=exe文件名去后缀，文件夹=文件夹名）
- 编辑模式下底部显示「删除」按钮（红色文字）
- 样式复用 CreateProjectDialog 的 `.dialog-*` 模式

**组件结构：**
```
<Teleport to="body">
  <div class="dialog-overlay">
    <div class="dialog-content glass-strong">
      <p class="dialog-title">{{ isEditing ? '编辑快捷方式' : '添加快捷方式' }}</p>

      <!-- 类型选择 -->
      <div class="type-selector">
        <button>应用</button>
        <button>文件夹</button>
        <button>网页</button>
      </div>

      <!-- 路径/URL -->
      <div class="path-row">
        <input v-model="path" />
        <button v-if="type !== 'web'" @click="browse">浏览...</button>
      </div>

      <!-- 名称 -->
      <input v-model="name" />

      <!-- 操作按钮 -->
      <div class="dialog-actions">
        <button v-if="isEditing" class="delete-btn" @click="handleDelete">删除</button>
        <button class="dialog-btn-primary" @click="handleSave">{{ isEditing ? '保存' : '添加' }}</button>
        <button class="dialog-btn-secondary" @click="cancel">取消</button>
      </div>
    </div>
  </div>
</Teleport>
```

---

## Task 4: 前端 — Sidebar.vue 改造

**Files:**
- Modify: `src/components/Sidebar.vue`

**改动：**
1. 加载逻辑：`onMounted` 调用 `invoke('load_shortcuts')` 获取列表
2. 渲染列表：每个快捷方式显示默认图标（根据类型）+ 名称（截断+tooltip）
3. 单击处理：调用 `invoke('launch_shortcut', { shortcutType, path })`
4. 长按处理：setTimeout 检测（500ms），触发后打开 ShortcutDialog 编辑模式
5. [+] 按钮：打开 ShortcutDialog 添加模式
6. 弹窗回调：保存/删除后重新 `invoke('save_shortcuts')` 并刷新列表

**默认图标（SVG inline）：**
- 应用：齿轮图标
- 文件夹：文件夹图标
- 网页：地球图标

**长按交互：**
```typescript
let pressTimer: number | null = null
let isLongPress = false

function onPointerDown(shortcut: Shortcut) {
  isLongPress = false
  pressTimer = window.setTimeout(() => {
    isLongPress = true
    openEditDialog(shortcut)
  }, 500)
}

function onPointerUp(shortcut: Shortcut) {
  if (pressTimer) clearTimeout(pressTimer)
  if (!isLongPress) {
    launchShortcut(shortcut)
  }
}
```

---

## 执行顺序

Task 1 → Task 2 → Task 3 → Task 4（严格顺序，后端先行）

每完成一个 Task 后编译验证再继续下一个。
