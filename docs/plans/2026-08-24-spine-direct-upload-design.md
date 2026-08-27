# Spine 原件直传设计

## 目标

素材详情侧边栏对处于 `original` 状态的静帧和序列帧统一显示「Spine」按钮。点击后将 `00_original` 原件复制到 nextcloud 的隔离目录，并由文件扫描结果自然判定为“已上传”，不引入额外状态文件。

## 数据与目录

- 普通静帧：保持现状，复制到 `nextcloud/{任务}/original/{文件}`。
- 普通序列帧：递归复制到 `nextcloud/{任务}/original/{素材名}/{帧文件}`。
- Prototype 静帧：保持现状，复制到 `nextcloud/Prototype/{子分类}/original/{文件}`。
- Prototype 序列帧：递归复制到 `nextcloud/Prototype/{子分类}/original/{素材名}/{帧文件}`。

目录名是素材标识，保留帧目录可防止不同序列之间的帧重名和污染。复制仍复用 `copy_to_nextcloud`，普通 `02_done` 交付上传路径不变。

## 状态与生命周期

序列帧进度判定新增合法分支：`nextcloud` 根层命中仍要求存在 `02_done` webp；`original/{素材名}/` 命中则直接视为 Spine 原件已上传，不判链条断裂。

“更新/删除”必须递归删除 Spine 序列目录；“重命名”必须同步处理 `nextcloud/original` 中的静帧文件或序列目录。复制或扫描失败保持侧边栏打开并写入错误日志，允许用户重试。

## UI

按钮复用现有 `sidebar-action-btn`，不增加颜色、尺寸或新样式。按钮显示条件统一为：素材类型是 `image` 或 `sequence`，且 `progress === 'original'`。忙碌期间禁用，避免重复复制。

## 验证

Rust 单元测试覆盖序列目录递归复制和 `original/{素材名}/` 上传判定；前端生产构建覆盖模板类型与 i18n 对称性；完整 Rust 测试与静态检查作为交付门禁。
