# 静帧转换流程重构 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将静帧转换的监控目标从 `02_done/` 改为 `01_scale/`，用户在 Imagine 里直接保存到源文件旁边，程序自动将新生成的 `.webp` 移动到 `02_done/[img-XX]/`，同时清理废弃代码（预热 Explorer、剪贴板 TODO）。

**Architecture:** 修改 `conversion.rs` 的 `handle_file_event` 函数签名和逻辑——接收 `scale_dir`（`01_scale/`）和 `done_path` 两个路径，从事件中的文件父目录名解析比例值，目标目录改为 `done_path/[img-XX]/`。`start_conversion` 改为递归监控 `01_scale/`，删除 Explorer 预热和剪贴板 TODO。同时清理 `ConversionSession` 中已成死代码的 `image_map` 和 `watcher` 字段（它们在 session 里存着但从不读出，实际数据通过闭包捕获）。

**Tech Stack:** Rust, Tauri 2.x, notify crate（已引入）

---

## 背景：当前代码问题

| 位置 | 问题 |
|------|------|
| `conversion.rs:40` | `path.parent() != Some(done_path)` — 过滤逻辑基于 `done_path`，整个函数签名依赖错误的监控目标 |
| `conversion.rs:53` | `image_map.get(stem)` — 靠预先构建的 stem→scale 映射，新逻辑直接从路径解析不需要映射 |
| `commands.rs:1191` | `watcher.watch(&done_path, NonRecursive)` — 监控目标和模式都要改 |
| `commands.rs:1199` | `explorer done_path` — 废弃，删除 |
| `commands.rs:1202` | `// TODO: Clipboard` — 废弃，删除 |
| `conversion.rs:15` | `image_map` 字段存在 session 里但从不读出（Rust warning） |
| `conversion.rs:19` | `watcher` 字段存在 session 里但从不读出（Rust warning） |

---

## Task 1：重写 `handle_file_event`

**文件：**
- 修改：`src-tauri/src/conversion.rs:28-79`

**当前签名：**
```rust
pub fn handle_file_event<R: Runtime>(
    event: Event,
    done_path: &Path,
    image_map: &HashMap<String, u32>,
    app_handle: &AppHandle<R>,
)
```

**新逻辑说明：**
- 不再需要 `image_map` 参数（比例从路径解析）
- 需要 `scale_dir: &Path`（`01_scale/` 路径，用于验证文件确实在其下）
- `done_path: &Path` 保留（移动目标的根目录）
- 检测逻辑：事件路径的父目录必须是 `scale_dir/[数字]/` 形式
- 比例解析：从父目录名 `[XX]` 中提取数字

**Step 1：替换函数体**

将 `conversion.rs` 中 `handle_file_event` 整体替换为：

```rust
/// 处理文件监控事件的逻辑
/// scale_dir: 01_scale/ 目录路径
/// done_path: 02_done/ 目录路径
pub fn handle_file_event<R: Runtime>(
    event: Event,
    scale_dir: &Path,
    done_path: &Path,
    app_handle: &AppHandle<R>,
) {
    if !event.kind.is_create() && !event.kind.is_modify() {
        return;
    }

    for path in event.paths {
        if !path.is_file() { continue; }

        // 只处理 .webp 文件（imagine 输出格式）
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext != "webp" { continue; }

        // 父目录必须是 scale_dir/[XX]/ 形式
        let parent = match path.parent() {
            Some(p) => p,
            None => continue,
        };
        if parent.parent() != Some(scale_dir) { continue; }

        // 从父目录名 "[XX]" 解析比例数字
        let dir_name = parent.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !dir_name.starts_with('[') || !dir_name.ends_with(']') { continue; }
        let scale_str = &dir_name[1..dir_name.len() - 1];
        let scale: u32 = match scale_str.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        let target_dir = done_path.join(format!("[img-{}]", scale));
        let app_handle_clone = app_handle.clone();
        let source = path.clone();
        let dest = target_dir.join(&file_name);

        tauri::async_runtime::spawn(async move {
            // 等待文件写入完成（防抖）
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if !target_dir.exists() {
                let _ = fs::create_dir_all(&target_dir);
            }
            if let Err(e) = fs::rename(&source, &dest) {
                eprintln!("自动整理失败 ({}): {}", stem, e);
            } else {
                let _ = app_handle_clone.emit("conversion-organized", stem);
            }
        });
    }
}
```

**Step 2：cargo check**

```bash
cd src-tauri && cargo check 2>&1
```

期望：编译错误，提示 `handle_file_event` 调用方参数不匹配（`start_conversion` 里的调用还没改）。这是正常的，说明改对了。

---

## Task 2：清理 `ConversionSession` 死代码字段

**文件：**
- 修改：`src-tauri/src/conversion.rs:11-23`

`image_map` 和 `watcher` 字段实际上在 session 中存储后从不读取（数据通过闭包捕获传递），是 Rust warning 的来源。删掉它们，让 session 只保存真正需要的字段。

**Step 1：修改 ConversionSession 结构体**

```rust
/// 转换会话状态
pub struct ConversionSession {
    pub imagine_pid: Option<u32>,
    pub done_path: PathBuf,
    /// 序列帧名称 -> 帧率
    pub sequence_fps_map: HashMap<String, u32>,
    /// 外部工具路径
    pub texture_packer_cli: PathBuf,
    pub texture_packer_gui: PathBuf,
}
```

**Step 2：清理 imports**

`conversion.rs` 顶部的 import 行：
```rust
use notify::{Watcher, RecursiveMode, Config, RecommendedWatcher, Event};
```
改为：
```rust
use notify::{Event};
```

（`Watcher`、`RecursiveMode`、`Config`、`RecommendedWatcher` 都移到 `commands.rs` 使用处，`conversion.rs` 只负责处理事件。）

**Step 3：cargo check**

```bash
cd src-tauri && cargo check 2>&1
```

期望：仍然有 `start_conversion` 相关的错误（session 字段变了），下一个 Task 修复。

---

## Task 3：重写 `start_conversion` 中的监控逻辑

**文件：**
- 修改：`src-tauri/src/commands.rs:1138-1248`

这是改动最大的部分。

**Step 1：替换 `start_conversion` 函数体**

找到 `start_conversion` 函数体（约 L1138-L1248），替换为：

```rust
pub fn start_conversion<R: Runtime>(
    app_handle: AppHandle<R>,
    state: State<'_, ConversionState>,
    request: StartConversionRequest,
) -> Result<(), String> {
    let task_dir = Path::new(&request.task_path);
    let done_path = task_dir.join("02_done");
    let scale_dir = task_dir.join("01_scale");

    if !done_path.exists() {
        fs::create_dir_all(&done_path).map_err(|e| e.to_string())?;
    }

    // 1. 序列帧映射 (前端已传入)
    let mut sequence_fps_map = HashMap::new();
    for seq in request.sequences {
        sequence_fps_map.insert(seq.name, seq.fps);
    }

    // 2. 开启 notify 递归监控 01_scale/
    let done_path_clone = done_path.clone();
    let scale_dir_clone = scale_dir.clone();
    let app_handle_inner = app_handle.clone();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        match res {
            Ok(event) => handle_file_event(event, &scale_dir_clone, &done_path_clone, &app_handle_inner),
            Err(e) => eprintln!("watch error: {:?}", e),
        }
    }).map_err(|e| e.to_string())?;

    watcher.watch(&scale_dir, RecursiveMode::Recursive).map_err(|e| e.to_string())?;

    // 3. 启动 Imagine (如果选了静帧)
    let mut imagine_pid = None;
    if !request.images.is_empty() {
        let imagine_path = Path::new(&request.imagine_path);
        if imagine_path.exists() {
            // 构造参数列表：所有选中的静帧原始文件路径
            let mut args: Vec<String> = Vec::new();
            for (name, _) in &request.images {
                // 在 01_scale/[XX]/ 下查找该文件（遍历各比例目录）
                if let Ok(entries) = fs::read_dir(&scale_dir) {
                    for entry in entries.flatten() {
                        let dir_path = entry.path();
                        if !dir_path.is_dir() { continue; }
                        let dir_name = dir_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if !dir_name.starts_with('[') || !dir_name.ends_with(']') { continue; }
                        for ext in &["png", "jpg", "jpeg"] {
                            let p = dir_path.join(format!("{}.{}", name, ext));
                            if p.exists() {
                                args.push(p.to_string_lossy().to_string());
                                break;
                            }
                        }
                    }
                }
            }

            if let Ok(child) = std::process::Command::new(imagine_path).args(&args).spawn() {
                let pid = child.id();
                imagine_pid = Some(pid);
                let pid_clone = pid;
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    bring_window_to_front(pid_clone);
                });
            }
        }
    }

    // 4. 保存状态（watcher 必须存活，否则监控立即停止）
    let mut state_lock = state.lock().map_err(|e| e.to_string())?;
    *state_lock = Some(ConversionSession {
        imagine_pid,
        done_path,
        sequence_fps_map,
        _watcher: watcher,
        texture_packer_cli: PathBuf::from(request.texture_packer_cli_path),
        texture_packer_gui: PathBuf::from(request.texture_packer_gui_path),
    });

    Ok(())
}
```

> **注意**：`watcher` 必须存活才能继续监控（drop 掉就停了），所以 session 里要保留它，但用 `_watcher` 命名避免 dead_code warning，同时在 struct 里也改名为 `_watcher`。

**Step 2：同步修改 ConversionSession 结构体**（回到 `conversion.rs`）

把 `_watcher` 字段加回来：

```rust
pub struct ConversionSession {
    pub imagine_pid: Option<u32>,
    pub done_path: PathBuf,
    /// 序列帧名称 -> 帧率
    pub sequence_fps_map: HashMap<String, u32>,
    /// 监控器句柄（必须存活，drop 即停止监控）
    pub _watcher: RecommendedWatcher,
    /// 外部工具路径
    pub texture_packer_cli: PathBuf,
    pub texture_packer_gui: PathBuf,
}
```

同时恢复 `conversion.rs` 的 import（需要 `RecommendedWatcher`）：

```rust
use notify::{RecommendedWatcher, Event};
```

（`Watcher`、`RecursiveMode` 在 `commands.rs` 里用，不需要在这里 import。）

**Step 3：cargo check**

```bash
cd src-tauri && cargo check 2>&1
```

期望：`warning: unused import` 数量明显减少，无 error。

---

## Task 4：清理 commands.rs 的废弃 import

**文件：**
- 修改：`src-tauri/src/commands.rs` 顶部 import 区域

**Step 1：确认 commands.rs 顶部 import**

```bash
head -20 src-tauri/src/commands.rs
```

找到 conversion 相关 import，确认 `Watcher`、`RecursiveMode` 在 `commands.rs` 里已有或需要补上：

`commands.rs` 里用到 `notify::recommended_watcher`、`RecursiveMode::Recursive`、`Event`，需要：

```rust
use notify::{RecursiveMode, Event};
```

（`recommended_watcher` 是自由函数，不需要 import trait。）

**Step 2：最终 cargo check**

```bash
cd src-tauri && cargo check 2>&1
```

期望：0 error，warnings 数量比之前少（至少清掉 `unused import` 和 `fields never read` 的 warning）。

---

## Task 5：验证前端不需要改动

**文件：**
- 只读：`src/views/TaskPage.vue:308-348`
- 只读：`src/components/ConversionDialog.vue:70-94`

**Step 1：确认前端调用点**

前端调用 `start_conversion` 传入的 `request.images` 是 `Record<string, number>`（`name -> 0`，比例由后端识别）。新逻辑同样在后端扫描 `01_scale/` 确定文件路径，前端接口不变，无需修改。

`conversion-organized` 事件 payload 仍然是 `stem`（文件名无后缀），前端只做计数，无需变更。

**Step 2：确认编译正常**

```bash
cd src-tauri && cargo check 2>&1
```

期望：0 error。

---

## 变更总结

| 文件 | 变更内容 |
|------|---------|
| `conversion.rs` | `handle_file_event` 签名去掉 `image_map`，加 `scale_dir`；从路径解析比例；监控目标从 `done_path/` 改为 `scale_dir/[XX]/` |
| `conversion.rs` | `ConversionSession` 删除 `image_map` 字段，`watcher` 改名 `_watcher` 并改为非 Option |
| `conversion.rs` | imports 精简为只需要 `RecommendedWatcher, Event` |
| `commands.rs` | `start_conversion` 删除 Explorer 预热、剪贴板 TODO；监控改为 `RecursiveMode::Recursive` 监控 `scale_dir`；删除 `image_map` 构建逻辑 |
| `commands.rs` | imports 补充 `notify::{RecursiveMode, Event}` |
