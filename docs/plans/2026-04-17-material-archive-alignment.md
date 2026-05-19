# 素材删除对齐三段式归档

**日期**：2026-04-17
**状态**：已落地，待归档

## 背景

删除行为的顶层设计三分家本已成型：

| 场景 | 旧行为 | 归属 |
|------|--------|------|
| 项目删除（HomePage） | Windows 回收站 | 保持 |
| 项目素材 / 游戏介绍（普通文件） | Windows 回收站 | 保持 |
| 任务删除（取消勾选） | 归档 `.archived_tasks/` + 60 天 GC | 保持 |
| **任务内单个素材删除（TaskPage 侧边栏）** | ❌ `fs::remove_*` 永久删除 | **本次对齐** |

素材删除是删除体系的破窗：直接物理抹除 `00_original` / `01_scale/*` / `02_done/*` / `nextcloud` 四个目录的命中文件，不可恢复。与任务归档的三段式（归档 → 60 天 GC → 手动清理）不一致，且用户易误删。

## 设计决策（产品总监拍板）

1. **时光机入口**：ProjectPage 快捷功能区新增「时光机」按钮，独立页面承接；TaskListPage 的「时光机」Tab 下线
2. **归档保留期**：60 天（与任务归档对齐）
3. **nextcloud 处理**：不归档，直接删除（nextcloud 仅作本地上传标记，非云端本体）
4. **恢复冲突策略**：保持拒绝式（目标位置已存在同名文件直接报错，用户需先删再恢复）

## 归档目录结构

```
<project>/.archived_materials/<TaskName>/<BaseName>/timestamp_YYYY-MM-DD_HH-MM/
  ├─ 00_original/<命中文件>
  ├─ 01_scale/<分辨率>/<命中文件>
  └─ 02_done/<规格>/<命中目录或文件>
```

`nextcloud/` 命中的副本在删除时同步物理删除，不进归档。

## 实施清单

### 后端（Rust）

| 文件 | 改动 |
|------|------|
| `src-tauri/src/models.rs` | 新增 `ArchivedMaterialVersion` DTO（task_name / base_name / material_type / timestamp / display_time / path / size_bytes / stages） |
| `src-tauri/src/commands/files.rs` | `delete_material` 改写为归档模式：`00/01/02_*` 的命中项 move 到归档，`nextcloud` 命中项直接删除 |
| `src-tauri/src/commands/files.rs` | 新增 `list_archived_materials`（含 60 天懒 GC）、`restore_archived_material`（拒绝式冲突）、`delete_archived_material_version` |
| `src-tauri/src/commands/files.rs` | 新增内部 helper：`infer_archived_material_type` / `scan_archive_content` / `compute_path_size` / `collect_restore_conflicts` / `restore_stage_dir` |
| `src-tauri/src/lib.rs` | 注册 3 个新命令 |

### 前端（Vue）

| 文件 | 改动 |
|------|------|
| `src/types/task.ts` | 新增 `ArchivedMaterialVersion` 接口 |
| `src/composables/useArchivedMaterials.ts` | 新建：load / restore / remove 封装 |
| `src/views/TimeMachinePage.vue` | 新建：双 Tab（任务归档 / 素材归档）独立页面 |
| `src/router/index.ts` | 新增 `/project/:projectId/time-machine` 路由 |
| `src/views/ProjectPage.vue` | `buildNavActions` 在「任务列表」右侧新增「时光机」入口 |
| `src/views/TaskListPage.vue` | 移除「时光机」Tab 及所有相关 state / logic / template / 样式 |
| `src/locales/zh-CN.ts` + `src/locales/en.ts` | 新增 `project.timeMachine` 及整个 `timeMachine.*` 命名空间；修改 `task.deleteMaterialDesc` 文案；调整 `onboarding.taskListEnable` 和 `onboarding.projectShortcuts` |

## 验证闭环

- `cargo check` — 通过
- `npm run build`（含 i18n parity 检查 + Vite 构建）— 通过，545 键对齐

## 回归测试清单（手测）

- [ ] 任务页删素材 → `<project>/.archived_materials/` 出现归档 → 时光机「素材归档」Tab 能看到
- [ ] 时光机素材归档 → 点「恢复」→ 文件回到原位
- [ ] 恢复素材时若原位已有同名文件 → 后端拒绝并返回冲突清单
- [ ] 时光机素材归档 → 点「删除」→ 归档物理消失
- [ ] 任务页删素材时的 nextcloud 副本 → 被物理删除（未进归档）
- [ ] 任务归档（取消勾选任务）+ 时光机「任务归档」Tab → 行为不变
- [ ] 60 天以上的归档 → 调用 `list_archived_materials` 时自动清理
