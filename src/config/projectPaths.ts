/** 项目目录结构 SSOT（与 Rust 侧 src-tauri/src/commands/workflow_paths.rs 对齐）
 *
 *  <project>/03_Render_VFX/VFX/
 *    ├── Export/<Task>/{00_original, 01_scale, 02_done, 03_preview}
 *    ├── nextcloud/<Task>/（preview/ + preview/breakdown/）
 *    ├── AE/
 *    └── PSD/
 */
export const VFX_SUBPATH = '03_Render_VFX\\VFX'

/** 任务工作目录：<project>\03_Render_VFX\VFX\Export\<Task> */
export function taskFolderPath(projectPath: string, taskId: string): string {
  return `${projectPath}\\${VFX_SUBPATH}\\Export\\${taskId}`
}

/** 任务上传标记目录：<project>\03_Render_VFX\VFX\nextcloud\<Task> */
export function nextcloudTaskPath(projectPath: string, taskId: string): string {
  return `${projectPath}\\${VFX_SUBPATH}\\nextcloud\\${taskId}`
}

/** 预览视频上传目录：<project>\03_Render_VFX\VFX\nextcloud\preview */
export function nextcloudPreviewPath(projectPath: string): string {
  return `${projectPath}\\${VFX_SUBPATH}\\nextcloud\\preview`
}

/** AE 工程目录（正斜杠形式，供 convertFileSrc / 拼 URL 场景使用） */
export function aeDirPath(projectPath: string): string {
  return projectPath.replace(/\\/g, '/') + '/' + VFX_SUBPATH.replace(/\\/g, '/') + '/AE'
}

/** PSD 素材库子路径（MaterialsPage 分组用，相对项目根） */
export const PSD_SUBPATH = `${VFX_SUBPATH}\\PSD`
