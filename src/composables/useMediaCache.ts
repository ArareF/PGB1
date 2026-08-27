import { ref, readonly } from 'vue'
import { clearSequenceCache } from './useFrameCache'
import { clearPsdThumbnailCache } from './usePsdThumbnail'

/**
 * 媒体刷新代次（手动刷新 SSOT）。
 *
 * 三个作用，缺一不可——只清缓存不动代次，屏幕上的旧图不会变：
 *   1. **破 WebView HTTP 缓存**：拼进 asset URL 的 `?v=`，让同路径新内容重新走一次请求；
 *   2. **破组件内缓存**：媒体组件（SequencePreview / NormalCard / usePreviewVideos）watch 它，
 *      丢弃自己 ref 里的解码结果并重新生成——这些缓存活在组件实例里，模块级清函数够不着；
 *   3. **破模块级缓存**：见下方 clearMediaCaches。
 *
 * 只读导出：改代次必须走 clearMediaCaches，避免出现"涨了代次但没清缓存"的半吊子状态。
 */
const version = ref(0)
export const mediaVersion = readonly(version)

/**
 * 手动刷新时清空前端媒体缓存的 SSOT。
 *
 * 背景：序列帧帧图（`useFrameCache`）与 PSD 缩略图（`usePsdThumbnail`）都是模块级常驻缓存，
 * 跨组件卸载/页面切换都不失效。素材在同路径原地更新（重转同尺寸 / 规范化 / 改 .tps 等）后，
 * 若不清缓存，预览仍显示旧内容。静帧 `<img>` 由后端 `?v=preview_version`（mtime）+ 本代次共同破除。
 *
 * 仅在**用户手动点刷新按钮 / 执行了原地改文件的操作后**调用，不挂到失焦自动刷新——
 * 否则每次切回窗口都要重解码上百帧图片。
 */
export function clearMediaCaches(): void {
  clearSequenceCache()
  clearPsdThumbnailCache()
  // 必须放在清理之后：组件 watch 到代次变化会立刻重新取图，此时缓存须已是空的
  version.value++
}
