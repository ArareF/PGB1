# Rust 后端防御性加固实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 消除 Rust 后端所有裸 `.unwrap()` panic 炸弹，并将 `eprintln!` 升级为语义化日志宏

**Architecture:**
- `.unwrap()` 按风险分级处理：用户数据路径操作用 `ok_or_else + ?` 传播错误；初始化不变式用 `.expect("reason")` 自文档化；逻辑保证单元素的 `else` 分支改 `if let Some`
- 日志升级：添加 `log = "0.4"` 显式依赖（已是 `tauri` 的传递依赖，zero cost），4 处 `eprintln!` 改为 `log::warn!` / `log::error!`，前向兼容 `tauri-plugin-log`

**Tech Stack:** Rust, Tauri 2, `log = "0.4"`

---

## Task 1: lib.rs — 初始化不变式 `.expect()`

**Files:**
- Modify: `src-tauri/src/lib.rs:91,99`

**Step 1: 修改 lib.rs:91**

```rust
// 改前
let window = app.get_webview_window("main").unwrap();
// 改后
let window = app.get_webview_window("main").expect("main 窗口必须在 tauri.conf.json 中声明");
```

**Step 2: 修改 lib.rs:99**

```rust
// 改前
.icon(app.default_window_icon().unwrap().clone())
// 改后
.icon(app.default_window_icon().expect("tauri.conf.json 必须配置 windows.icon").clone())
```

**Step 3: 验证编译**

```bash
cd src-tauri && cargo check 2>&1
```
期望：零错误

**Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix(rust): lib.rs unwrap 改为 expect 自文档化初始化不变式"
```

---

## Task 2: commands.rs:641 — else 分支空集防御

**Files:**
- Modify: `src-tauri/src/commands.rs:639-648`

**背景:** `else` 分支在 `files.len() > 1` 为假时执行，理论上含 `len == 0` 死角（虽然极不可能，因为 files 由文件系统遍历填充）。改为 `if let Some` 安全跳过。

**Step 1: 修改 else 分支**

```rust
// 改前 (line 639-648)
        } else {
            // 单文件（如 _01 的静帧）→ 移入独立文件列表
            let path = files.into_iter().next().unwrap();
            let fname = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            standalone_files.push((path, fname));
        }

// 改后
        } else if let Some(path) = files.into_iter().next() {
            // 单文件（如 _01 的静帧）→ 移入独立文件列表
            let fname = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            standalone_files.push((path, fname));
        }
```

**Step 2: 验证编译**

```bash
cd src-tauri && cargo check 2>&1
```

**Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "fix(rust): scan_materials else 分支空集防御，unwrap → if let Some"
```

---

## Task 3: commands.rs:1948/1954/1962 — execute_normalize 路径操作

**Files:**
- Modify: `src-tauri/src/commands.rs:1946-1964`

**背景:** `execute_normalize` 已返回 `Result<(), String>`，可以直接用 `ok_or_else + ?` 传播错误给前端显示。3 处 `.unwrap()` 全部在用户提供的文件路径上操作，路径异常时必须安全回报而不是崩溃。

**Step 1: 修改 Rename 分支 (line 1948)**

```rust
// 改前
let new_path = old_path.parent().unwrap().join(&item.target_name);

// 改后
let new_path = old_path
    .parent()
    .ok_or_else(|| format!("无法获取父目录: {}", item.original_path))?
    .join(&item.target_name);
```

**Step 2: 修改 MoveToFolder 分支 (line 1954)**

```rust
// 改前
let parent = old_path.parent().unwrap();

// 改后
let parent = old_path
    .parent()
    .ok_or_else(|| format!("无法获取父目录: {}", item.original_path))?;
```

**Step 3: 修改 file_name (line 1962)**

```rust
// 改前
let dest_path = target_dir.join(old_path.file_name().unwrap());

// 改后
let dest_path = target_dir.join(
    old_path
        .file_name()
        .ok_or_else(|| format!("无法获取文件名: {}", item.original_path))?,
);
```

**Step 4: 验证编译**

```bash
cd src-tauri && cargo check 2>&1
```

**Step 5: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "fix(rust): execute_normalize 路径 unwrap 改 ok_or_else+? 安全传播"
```

---

## Task 4: commands.rs:5011 — rename_material 帧文件父路径

**Files:**
- Modify: `src-tauri/src/commands.rs:5011`

**背景:** `fpath` 来自 `fs::read_dir` 遍历，必定是某目录内的文件，父路径存在是结构性保证。用 `.expect()` 自文档化即可（也符合整行已被 `let _ = ` 忽略返回值的模式）。

**Step 1: 修改 line 5011**

```rust
// 改前
let _ = fs::rename(&fpath, fpath.parent().unwrap().join(&new_fname));

// 改后
let _ = fs::rename(
    &fpath,
    fpath.parent().expect("read_dir 帧文件必有父目录").join(&new_fname),
);
```

**Step 2: 验证编译**

```bash
cd src-tauri && cargo check 2>&1
```

**Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "fix(rust): rename_material 帧文件路径 unwrap 改 expect 自文档化"
```

---

## Task 5: 添加 log 依赖 + 替换全部 eprintln!

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/hotkey.rs:105`
- Modify: `src-tauri/src/conversion.rs:107`
- Modify: `src-tauri/src/commands.rs:1643`
- Modify: `src-tauri/src/commands.rs:5535`

**背景:** `log` 已是 `tauri` 的传递依赖，显式声明无额外开销。Tauri 2 在 debug 构建自动初始化 stderr logger；release 构建可后续通过 `tauri-plugin-log` 升级为文件日志，无需再改代码。

**Step 1: 在 Cargo.toml 添加 log 依赖**

在 `[dependencies]` 末尾追加：
```toml
log = "0.4"
```

**Step 2: hotkey.rs:105**

```rust
// 改前
eprintln!("[hotkey] 创建翻译窗口失败: {}", e);
// 改后
log::error!("[hotkey] 创建翻译窗口失败: {}", e);
```

**Step 3: conversion.rs:107**

```rust
// 改前
eprintln!("自动整理失败 ({}): {}", stem, e);
// 改后
log::warn!("自动整理失败 ({}): {}", stem, e);
```

**Step 4: commands.rs:1643**

```rust
// 改前
Err(e) => eprintln!("watch error: {:?}", e),
// 改后
Err(e) => log::error!("watch error: {:?}", e),
```

**Step 5: commands.rs:5535**

```rust
// 改前
eprintln!("PSD 解析失败 {}: {}", path, e);
// 改后
log::warn!("PSD 解析失败 {}: {}", path, e);
```

**Step 6: 验证编译（含 Cargo.lock 更新）**

```bash
cd src-tauri && cargo check 2>&1
```

期望：零错误，Cargo.lock 中 `log` 版本确认锁定

**Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/hotkey.rs src-tauri/src/conversion.rs src-tauri/src/commands.rs
git commit -m "fix(rust): eprintln! → log::warn!/error!，添加 log 0.4 显式依赖"
```

---

## Task 6: 最终验证

**Step 1: 完整构建检查**

```bash
cd src-tauri && cargo check 2>&1
```

期望：0 errors，0 warnings（关于 unwrap）

**Step 2: 启动 tauri dev，验证关键路径**

```bash
npm run tauri dev
```

手动验证：
- [ ] 应用正常启动（lib.rs 初始化路径）
- [ ] TaskPage 正常加载素材（scan_materials）
- [ ] NormalizationDialog 预览+执行规范化（execute_normalize）
- [ ] 重命名素材（rename_material 帧文件路径）

**Step 3: 确认无新 warning**

开发控制台（F12）检查无 JS 错误，Tauri stderr 检查无 panic 输出。
