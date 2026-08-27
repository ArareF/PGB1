//! 工作流目录命名 SSOT（与前端 src/config/projectPaths.ts 对齐）
//!
//! 项目结构：
//! ```text
//! <project>/03_Render_VFX/VFX/
//!   ├── Export/<Task>/{00_original, 01_scale, 02_done, 03_preview}
//!   └── nextcloud/<Task>/          ← 上传标记区（preview/ + preview/breakdown/）
//! ```
//! 02_done 子目录命名：静帧 `[img-<scale>]`，序列帧 `[an-<scale>-<fps>]`

use std::path::{Path, PathBuf};

pub(crate) const DIR_RENDER_VFX: &str = "03_Render_VFX";
pub(crate) const DIR_VFX: &str = "VFX";
pub(crate) const DIR_EXPORT: &str = "Export";
pub(crate) const DIR_NEXTCLOUD: &str = "nextcloud";
pub(crate) const DIR_AE: &str = "AE";
pub(crate) const DIR_PSD: &str = "PSD";

pub(crate) const DIR_ORIGINAL: &str = "00_original";
pub(crate) const DIR_SCALE: &str = "01_scale";
pub(crate) const DIR_DONE: &str = "02_done";
pub(crate) const DIR_PREVIEW: &str = "03_preview";

/// nextcloud 下的预览视频目录及其 breakdown 子目录
pub(crate) const DIR_NC_PREVIEW: &str = "preview";
pub(crate) const DIR_NC_BREAKDOWN: &str = "breakdown";
/// nextcloud/<Task>/ 下的 Spine 原件直传落点（与 webp 交付物隔离）
pub(crate) const DIR_NC_ORIGINAL: &str = "original";

/// 02_done 子目录前缀：序列帧 [an-...] / 静帧 [img-...]
pub(crate) const STAGE_PREFIX_ANIM: &str = "an";
pub(crate) const STAGE_PREFIX_IMG: &str = "img";

/// <project>/03_Render_VFX/VFX/
pub(crate) fn vfx_dir(project_dir: &Path) -> PathBuf {
    project_dir.join(DIR_RENDER_VFX).join(DIR_VFX)
}

/// <project>/03_Render_VFX/VFX/Export/
pub(crate) fn export_dir(project_dir: &Path) -> PathBuf {
    vfx_dir(project_dir).join(DIR_EXPORT)
}

/// <project>/03_Render_VFX/VFX/nextcloud/
pub(crate) fn nextcloud_dir(project_dir: &Path) -> PathBuf {
    vfx_dir(project_dir).join(DIR_NEXTCLOUD)
}

/// 从任务目录（.../Export/<Task>）推导同名 nextcloud 目录（.../nextcloud/<Task>）。
/// task_dir 不足两级父目录时返回 None（防御：调用方传入异常路径）
pub(crate) fn nextcloud_task_dir(task_dir: &Path) -> Option<PathBuf> {
    task_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|vfx| vfx.join(DIR_NEXTCLOUD).join(task_dir.file_name().unwrap_or_default()))
}

/// 阶段子目录名前缀（用于 starts_with 匹配）：`[an-` / `[img-`
pub(crate) fn stage_dir_prefix(kind: &str) -> String {
    format!("[{}-", kind)
}

/// 静帧成品子目录名：`[img-<scale>]`
pub(crate) fn img_dir_name(scale: u32) -> String {
    format!("[{}-{}]", STAGE_PREFIX_IMG, scale)
}

/// 序列帧成品子目录名：`[an-<scale>-<fps>]`
pub(crate) fn an_dir_name(scale: u32, fps: u32) -> String {
    format!("[{}-{}-{}]", STAGE_PREFIX_ANIM, scale, fps)
}
