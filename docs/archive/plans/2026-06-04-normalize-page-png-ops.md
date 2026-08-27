# 规范化独立页面 + PNG 静帧两项新操作

> 状态：✅ 已实现（2026-06-04，备份默认 ON）
> 日期：2026-06-04
> 决策锁定：独立页面（方案 B） / add·screen 用「下划线分段含该词」 / 两项新操作仅限静帧 PNG

---

## 一、需求

1. **自适应画布**：勾选后，删除 PNG 静帧内多余的透明区域（裁到非透明像素包围盒）。
2. **添加黑底**：勾选后，对「下划线分段含 `add` 或 `screen`」的 PNG 静帧合成纯黑底（变成不透明）。
3. **命名规范化**：作为默认勾选项；页面默认显示**全部素材**（含已命名的），用户自选。
4. **序列帧合并显示**：现状逐帧展开（60 帧=60 行），改为每个素材一行。
5. **弹窗 → 独立页面**：仿 ScalePage/ConvertPage。

---

## 二、现状根因（代码定位）

- 触发：`TaskPage.vue:504` → `NormalizationDialog.vue`（460px 弹窗）。
- 扫描：`conversion.rs:581 scan_and_group_files`。
  - **序列帧逐帧 push**（`conversion.rs:629`）→ 全展开问题根因。
  - **只收集带 `_NN` 后缀的文件**（`conversion.rs:601`）→ 已命名素材不进列表，无法补做新操作。
- `image` crate 已是依赖（`conversion.rs:501`），两项新操作零新依赖。

---

## 三、后端改造（Rust）

### 3.1 新模型（models.rs）

```rust
pub struct NormalizeItem {
    pub base_name: String,        // 已去 _NN 的基础名
    pub material_type: String,    // "static" | "sequence"
    pub ext: String,              // 小写扩展名
    pub frame_count: u32,         // 静帧=1
    pub needs_rename: bool,       // 命名是否需规范化
    pub is_png: bool,
    pub is_add_or_screen: bool,   // base_name 按 '_' 切分，任一段 == "add"||"screen"
    pub thumbnail_path: String,   // 静帧=本体 / 序列帧=首帧（绝对路径）
    pub paths: Vec<String>,       // 静帧1 / 序列帧N
    pub target_name: String,      // 静帧=base.ext / 序列帧=base（文件夹名）
}

pub struct NormalizeRequest {
    pub paths: Vec<String>,
    pub material_type: String,
    pub base_name: String,
    pub ext: String,
    pub target_name: String,
    pub do_rename: bool,
    pub do_trim: bool,
    pub do_black_bg: bool,
}
```

### 3.2 新命令 `scan_normalize_items(task_path) -> Vec<NormalizeItem>`

全量盘点 `00_original/`（Prototype 走子分类，沿用现有逻辑）：

| 输入形态 | 判定 | needs_rename |
|----------|------|--------------|
| 带 `_NN` 松散文件，同基础名仅 1 个 | static | true（去后缀） |
| 带 `_NN` 松散文件，同基础名多个 | sequence | true（移入文件夹） |
| 不带 `_NN` 松散文件 | static（已命名） | false |
| 子目录（已规范序列帧夹） | sequence（已命名） | false，frame_count=夹内帧数 |

每项计算 `is_png` / `is_add_or_screen` / `thumbnail_path`。

### 3.3 新命令 `execute_normalize_v2(app_handle, requests)`

每个 request 执行顺序（保证一致性）：
1. **内容操作**（仅 static PNG）：若 `do_trim` 先裁透明包围盒；若 `do_black_bg` 再合成黑底（`out_rgb = src_rgb × src_alpha / 255`，输出不透明 PNG）。原地写回当前路径。
2. **命名操作**：`do_rename` 时——static 原地改名为 `target_name`；sequence 建 `target_name/` 文件夹并移入全部帧。
3. emit `normalize-progress`（current/total/name）。

辅助函数：
- `trim_transparent(img) -> DynamicImage`：求 alpha>0 的 min/max x·y，crop。全透明时跳过。
- `composite_on_black(img) -> DynamicImage`：黑底 alpha 合成。

### 3.4 破坏性写入与备份

两项操作对 00_original 原件**不可逆覆盖**。默认开启轻量备份：执行前把被改文件复制到 `00_original/.normalize_backup/`（隐藏，扫描时已忽略 `.` 开头）。页面提供「执行前备份原件」开关，默认 ON。

---

## 四、前端改造（Vue）

### 4.1 路由（router/index.ts）
新增 `/project/:projectId/task/:taskId/normalize` → `NormalizePage.vue`。

### 4.2 NormalizePage.vue（新建，仿 ScalePage）
- **顶部全局选项条**：命名规范化（默认 ON）/ 自适应画布（默认 OFF）/ 添加黑底（默认 OFF）/ 执行前备份（默认 ON）。
- **素材列表**：每素材一行（缩略图 + 名称 + 类型角标「静帧 / 序列帧 ×N 帧」）。
  - 命名 checkbox：needs_rename 才可选，否则灰显「已规范」。
  - 自适应画布 checkbox：仅 `is_png && static` 可选。
  - 添加黑底 checkbox：仅 `is_png && static && is_add_or_screen` 可选。
  - 全局开关驱动每行默认值，可逐行覆盖。
- **底部**：执行按钮 + 进度反馈（监听 `normalize-progress`）。

### 4.3 TaskPage.vue
`normalize` action 由 `showNormalizeDialog=true` 改为 `router.push({name:'normalize', ...})`（对齐 convert 的跳页方式，`TaskPage.vue:506`）。`hasNormalizeWork` 检测改用新命令或保留轻量判断。

### 4.4 i18n（zh-CN + en）
新增 `normalize.*`：页面标题、三个选项标签、类型角标、已规范态、进度文案。

### 4.5 清理
移除 `NormalizationDialog.vue` 与旧命令 `preview_normalize` / `execute_normalize`（页面接通后）。

---

## 五、爆炸半径

- `TaskPage.vue:424` 用 `preview_normalize` 算 `hasNormalizeWork` → 必须改。
- Prototype 特例扫描需沿用（已覆盖）。
- 删除旧弹窗/旧命令前确认无其他引用（已 grep：仅 TaskPage + Dialog 自身）。
- 文档维护：CODE_INDEX.md（+NormalizePage / -NormalizationDialog）、INDEX.md（页面结构）、`design/文件命名与组织规则.md` 步骤 2（补两项新选项）。

---

## 六、实施顺序

1. 后端：模型 + `scan_normalize_items` + `execute_normalize_v2` + 两个图像辅助函数 + 注册命令（lib.rs）。
2. 前端：路由 + NormalizePage + TaskPage 跳页改造 + i18n。
3. 自测：静帧裁切/黑底/改名、序列帧合并显示与移动、已命名素材补操作、Prototype。
4. 清理旧弹窗/旧命令。
5. 文档更新。
