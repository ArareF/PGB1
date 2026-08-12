import { clearSequenceCache } from './useFrameCache'
import { clearPsdThumbnailCache } from './usePsdThumbnail'

/**
 * 手动刷新时清空前端媒体缓存的 SSOT。
 *
 * 背景：序列帧帧图（`useFrameCache`）与 PSD 缩略图（`usePsdThumbnail`）都是模块级常驻缓存，
 * 跨组件卸载/页面切换都不失效。素材在同路径原地更新（重转同尺寸 / 规范化 / 改 .tps 等）后，
 * 若不清缓存，预览仍显示旧内容。静帧 `<img>` 已由后端 `?v=preview_version`（mtime）自动破除，
 * 无需在此处理。
 *
 * 仅在**用户手动点刷新按钮**时调用，不挂到失焦/自动刷新——否则每次切回窗口都要重解码上百帧图片。
 */
export function clearMediaCaches(): void {
  clearSequenceCache()
  clearPsdThumbnailCache()
}
