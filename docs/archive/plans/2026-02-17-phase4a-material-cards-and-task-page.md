# Phase 4a: 素材卡片 + 任务页双视图 + 侧边栏基础

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 任务页展示真实素材文件（素材卡片），支持树形分类和名称排序双视图切换，点击卡片打开侧边栏基础详情面板。

**Scope 界定:**
- **本 Phase 做的**：Rust 素材扫描（含进度判定）、MaterialCard 组件（静帧预览）、任务页双视图框架、小标题栏、侧边栏基础面板（图片预览+基本信息）
- **Phase 4b 再做**：Canvas 序列帧动画预览、LRU 缓存、侧边栏"其他版本"区域、Prototype 子分类完整处理

**Architecture:**
- Rust 后端新增 `scan_materials` 命令，扫描 `00_original/` 并关联各工作流目录判定进度
- 前端新增 `useMaterials` composable + `MaterialCard` 组件
- 任务页改造：小标题栏 + 视图切换 + 卡片网格
- 侧边栏：独立组件 `DetailSidebar.vue`，从右侧滑入

**前置条件:** Phase 3b 已完成。

**测试数据:** `D:\work\pgsoft\exp\217_RedDevil\03_Render_VFX\VFX\Export\Ambient\`

**设计文档参考:**
- 素材卡片: `design/卡片设计.md` L104-180
- 任务页双视图: `design/界面设计.md` L296-649
- 侧边栏: `design/界面设计.md` L1536-1770, `design/DesignSystem.md` 侧边栏 Token
- 素材进度: `design/文件命名与组织规则.md` L1460-1690
- Prototype: `design/Prototype特例规则.md`

---

## 总体路线图

| 阶段 | 内容 | 状态 |
|------|------|------|
| **Phase 1** | 脚手架 + 设计系统 + 主窗口框架 | ✅ |
| **Phase 2** | 三级页面导航 + Vue Router + 页面骨架 | ✅ |
| **Phase 3a** | 项目卡片 + 任务卡片 + Rust 文件系统后端 | ✅ |
| **Phase 3b** | 普通卡片 + 辅助页面文件扫描 + 打开文件夹 | ✅ |
| **Phase 4a**（本文档） | 素材卡片 + 任务页双视图 + 侧边栏基础 | ✅ |
| **Phase 4b** | 序列帧动画预览 + 版本追踪 + Prototype 完整处理 | ⬜ |
| **Phase 5** | 文件拖拽 + 任务管理系统 + 工作流功能 | ⬜ |
| **Phase 6** | 日报打卡 + 翻译 + 转换进度悬浮窗 + 设置页 | ⬜ |

---

## Task 1: Rust 素材数据模型 + 扫描命令

**Files:**
- Modify: `src-tauri/src/models.rs`（新增 MaterialInfo）
- Modify: `src-tauri/src/commands.rs`（新增 scan_materials）
- Modify: `src-tauri/src/lib.rs`（注册命令）

**Step 1: 新增 MaterialInfo 数据模型**

在 `models.rs` 末尾添加：

```rust
/// 素材文件类型
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MaterialType {
    /// 静帧图片（单个 png/jpg）
    Image,
    /// 序列帧动画（文件夹包含多帧）
    Sequence,
    /// 视频文件
    Video,
    /// 其他文件
    Other,
}

/// 素材进度状态
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MaterialProgress {
    /// 未开始（不在任何工作流目录）
    None,
    /// 原始文件（仅在 00_original）
    Original,
    /// 已缩放（存在于 01_scale，仅静帧）
    Scaled,
    /// 已完成（存在于 02_done）
    Done,
    /// 已上传（存在于 nextcloud）
    Uploaded,
}

/// 返回给前端的素材信息
#[derive(Debug, Serialize, Clone)]
pub struct MaterialInfo {
    /// 素材基础名（去掉扩展名和 _01 后缀）
    pub name: String,
    /// 完整文件名或目录名（在 00_original 中）
    pub file_name: String,
    /// 00_original 中的完整路径
    pub path: String,
    /// 素材类型
    pub material_type: MaterialType,
    /// 进度状态
    pub progress: MaterialProgress,
    /// 文件大小（字节），序列帧为整个目录大小
    pub size_bytes: u64,
    /// 序列帧帧数（非序列帧为 0）
    pub frame_count: u32,
    /// 文件扩展名（小写）
    pub extension: String,
    /// 预览图路径（静帧=文件本身路径，序列帧=首帧路径）
    pub preview_path: Option<String>,
}
```

**Step 2: 新增 scan_materials 命令**

在 `commands.rs` 添加扫描素材的命令：

```rust
/// 扫描任务的素材列表（从 00_original 读取，关联各目录判定进度）
#[tauri::command]
pub fn scan_materials(task_path: String) -> Result<Vec<MaterialInfo>, String> {
    let task_dir = Path::new(&task_path);
    let original_dir = task_dir.join("00_original");

    if !original_dir.exists() {
        return Ok(Vec::new());
    }

    let scale_dir = task_dir.join("01_scale");
    let done_dir = task_dir.join("02_done");

    // 获取 nextcloud 路径：从 task_path 向上推导
    // task_path = .../Export/{TaskName}
    // nextcloud = .../nextcloud/{TaskName}
    let nextcloud_dir = task_dir
        .parent() // Export/
        .and_then(|p| p.parent()) // VFX/
        .map(|vfx| vfx.join("nextcloud").join(task_dir.file_name().unwrap_or_default()));

    let mut materials = Vec::new();

    let entries = fs::read_dir(&original_dir)
        .map_err(|e| format!("无法读取 00_original: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取失败: {}", e))?;
        let path = entry.path();

        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // 跳过隐藏文件
        if file_name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            // 序列帧目录
            let frame_count = count_frames(&path);
            let first_frame = find_first_frame(&path);
            let size_bytes = calc_dir_size(&path);
            let base_name = file_name.clone();

            let progress = determine_progress_sequence(
                &base_name, &done_dir, &nextcloud_dir,
            );

            materials.push(MaterialInfo {
                name: base_name,
                file_name,
                path: path.to_string_lossy().to_string(),
                material_type: MaterialType::Sequence,
                progress,
                size_bytes,
                frame_count,
                extension: "seq".to_string(),
                preview_path: first_frame,
            });
        } else {
            // 单个文件
            let ext = path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let size_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);

            // 提取基础名（去掉扩展名，如有 _01 后缀也去掉）
            let stem = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            let base_name = stem.strip_suffix("_01").unwrap_or(stem).to_string();

            let material_type = match ext.as_str() {
                "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" => MaterialType::Image,
                "mp4" | "mov" | "avi" | "webm" => MaterialType::Video,
                _ => MaterialType::Other,
            };

            let progress = if material_type == MaterialType::Image {
                determine_progress_image(&base_name, &scale_dir, &done_dir, &nextcloud_dir)
            } else {
                MaterialProgress::Original
            };

            materials.push(MaterialInfo {
                name: base_name,
                file_name,
                path: path.to_string_lossy().to_string(),
                material_type,
                progress,
                size_bytes,
                frame_count: 0,
                extension: ext,
                preview_path: Some(path.to_string_lossy().to_string()),
            });
        }
    }

    // 按名称排序
    materials.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(materials)
}
```

辅助函数：

```rust
/// 静帧进度判定
fn determine_progress_image(
    base_name: &str,
    scale_dir: &Path,
    done_dir: &Path,
    nextcloud_dir: &Option<std::path::PathBuf>,
) -> MaterialProgress {
    // 检查 nextcloud（已上传）
    if let Some(nc) = nextcloud_dir {
        if nc.exists() && find_file_in_dir(nc, base_name) {
            return MaterialProgress::Uploaded;
        }
    }
    // 检查 02_done（已完成）
    if done_dir.exists() && find_file_in_subdirs(done_dir, base_name, "img") {
        return MaterialProgress::Done;
    }
    // 检查 01_scale（已缩放）
    if scale_dir.exists() && find_file_in_subdirs(scale_dir, base_name, "") {
        return MaterialProgress::Scaled;
    }
    MaterialProgress::Original
}

/// 序列帧进度判定（跳过 01_scale）
fn determine_progress_sequence(
    base_name: &str,
    done_dir: &Path,
    nextcloud_dir: &Option<std::path::PathBuf>,
) -> MaterialProgress {
    if let Some(nc) = nextcloud_dir {
        if nc.exists() && find_file_in_dir(nc, base_name) {
            return MaterialProgress::Uploaded;
        }
    }
    if done_dir.exists() && find_file_in_subdirs(done_dir, base_name, "an") {
        return MaterialProgress::Done;
    }
    MaterialProgress::Original
}

/// 在目录下查找含指定 base_name 的文件
fn find_file_in_dir(dir: &Path, base_name: &str) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(base_name) {
                return true;
            }
        }
    }
    false
}

/// 在子目录（[img-XX] 或 [an-XX-YY]）中查找文件
fn find_file_in_subdirs(dir: &Path, base_name: &str, prefix: &str) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() { continue; }

            let dir_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // 匹配前缀（如 [img-XX] 或 [an-XX-YY]，或空前缀匹配所有如 [100]）
            if prefix.is_empty() || dir_name.starts_with(&format!("[{}-", prefix)) {
                if find_file_in_dir(&path, base_name) {
                    return true;
                }
            }
        }
    }
    false
}

/// 计算序列帧目录中的帧数
fn count_frames(dir: &Path) -> u32 {
    fs::read_dir(dir)
        .map(|entries| entries
            .flatten()
            .filter(|e| e.path().is_file())
            .count() as u32
        )
        .unwrap_or(0)
}

/// 找到序列帧目录中的第一帧
fn find_first_frame(dir: &Path) -> Option<String> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_file())
        .collect();

    files.sort_by_key(|e| e.file_name());
    files.first().map(|e| e.path().to_string_lossy().to_string())
}
```

**Step 3: 注册命令**

lib.rs 添加 `commands::scan_materials`。

**Step 4: cargo check**

---

## Task 2: 前端数据层 + 素材类型定义

**Files:**
- Create: `src/composables/useMaterials.ts`

```typescript
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export type MaterialType = 'image' | 'sequence' | 'video' | 'other'
export type MaterialProgress = 'none' | 'original' | 'scaled' | 'done' | 'uploaded'

export interface MaterialInfo {
  name: string
  file_name: string
  path: string
  material_type: MaterialType
  progress: MaterialProgress
  size_bytes: number
  frame_count: number
  extension: string
  preview_path: string | null
}

export function useMaterials() {
  const materials = ref<MaterialInfo[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function loadMaterials(taskPath: string) {
    loading.value = true
    error.value = null
    try {
      materials.value = await invoke<MaterialInfo[]>('scan_materials', { taskPath })
    } catch (e) {
      error.value = String(e)
      console.error('扫描素材失败:', e)
    } finally {
      loading.value = false
    }
  }

  return { materials, loading, error, loadMaterials }
}
```

---

## Task 3: MaterialCard 组件

**Files:**
- Create: `src/components/MaterialCard.vue`

设计规范：220×280px，预览区 165px（3:4），底部文件名+进度标签+大小。

Phase 4a 预览实现：静帧直接用 `<img>` 加载 `preview_path`，序列帧用首帧静图占位（Phase 4b 改为 Canvas 动画）。

进度标签颜色映射：
- none → `--tag-status-pending-bg`
- original → `--tag-progress-original-bg`
- scaled → `--tag-progress-scaled-bg`
- done → `--tag-progress-done-bg`
- uploaded → `--tag-progress-uploaded-bg`

---

## Task 4: 任务页改造 — 小标题栏 + 双视图

**Files:**
- Modify: `src/views/TaskPage.vue`

**小标题栏按钮**（主功能区顶部）：
- 素材列表（文字标题）
- 树形视图 / 名称视图（切换按钮，当前激活高亮）
- 刷新按钮

**树形视图**：按文件类型分组（静帧 / 序列帧 / 视频），每组下面是卡片网格。

**名称视图**：所有素材按名称排序的平铺卡片网格。

需要：
1. 通过 projectId 找到项目路径，拼接 taskPath
2. `useMaterials(taskPath)` 加载素材
3. 视图模式切换（ref<'tree' | 'name'>）
4. 点击卡片 → 打开侧边栏（selectedMaterial）

---

## Task 5: DetailSidebar 组件

**Files:**
- Create: `src/components/DetailSidebar.vue`

**触发**：点击素材卡片时，将选中素材传入侧边栏，侧边栏从右侧滑入。

**布局**：
1. 标题区：「详情」+ 关闭按钮
2. 预览区：图片直接 `<img>` 加载（序列帧用首帧，Phase 4b 改为高质量 Canvas）
3. 基本信息区：文件名、类型、大小、帧数（序列帧）、进度状态

**样式**：
- 宽度 30%（默认）
- 毛玻璃 glass-strong
- 从右侧滑入 300ms ease-out / 滑出 200ms ease-in

---

## Task 6: 侧边栏集成到布局

**Files:**
- Modify: `src/layouts/MainLayout.vue` 或 直接在 TaskPage 中集成

侧边栏应该在主内容区右侧展开，挤压卡片区域（不是 overlay）。

选择方案：在 TaskPage 内管理侧边栏状态（selectedMaterial），作为同级元素渲染。GameIntroPage 和 MaterialsPage 也需要侧边栏，但 Phase 4a 先只做 TaskPage，后续复用。

---

## Task 7: 构建验证与收尾

**Step 1: TypeScript 检查**
```bash
npx vue-tsc --noEmit
```

**Step 2: Vite 构建**
```bash
npm run build
```

**Step 3: Cargo 编译**
```bash
cargo build
```

**Step 4: 验证清单**
- [ ] 任务页显示小标题栏（素材列表 + 视图切换 + 刷新）
- [ ] 树形视图：素材按类型分组（静帧 / 序列帧）
- [ ] 名称视图：所有素材按名称平铺
- [ ] 素材卡片显示：预览图 + 文件名 + 进度标签 + 大小
- [ ] 静帧预览图正常加载（从 00_original）
- [ ] 序列帧卡片显示首帧
- [ ] 进度标签颜色正确（original / scaled / done / uploaded）
- [ ] 点击卡片 → 侧边栏滑入（显示详情）
- [ ] 关闭侧边栏正常
- [ ] 视图切换流畅
- [ ] 明暗主题正常

**Step 5: 更新计划文档**

---

## Phase 4a 实施记录

**完成时间:** 2026-02-17

### 已完成

- Task 1: Rust 素材数据模型 + `scan_materials` 命令（MaterialType/MaterialProgress/MaterialInfo + 进度判定逻辑）
- Task 2: `useMaterials.ts` composable
- Task 3: `MaterialCard.vue` 组件（预览图 + 进度标签 + 帧数角标）
- Task 4: TaskPage.vue 改造 — 小标题栏 + 树形/名称双视图切换 + 刷新
- Task 5+6: 侧边栏基础版直接内联在 TaskPage 中（预览 + 基本信息，滑入动画）
- Task 7: 构建验证全通过（vue-tsc + vite build + cargo build）

### 设计决策

- 侧边栏暂未提取为独立组件，直接在 TaskPage 内管理状态。后续其他页面需要侧边栏时再提取复用。
- 树形视图简化为按类型分组（静帧/序列帧/视频/其他），不做多层级展开折叠（Phase 4b 可增强）。

### 遗留问题

- MaterialProgress::None variant 未使用（warning），后续添加全新素材时会用到
- 序列帧预览暂为首帧静图，Phase 4b 改为 Canvas 动画
- 多选模式留到 Phase 5
