import { convertFileSrc } from '@tauri-apps/api/core'

interface CachedSequence {
  key: string
  frames: HTMLImageElement[]
  lastUsed: number
}

const MAX_CACHED = 10
const MAX_FRAMES = 120
const cache: CachedSequence[] = []

/**
 * 媒体缓存代次：手动刷新时 +1，作为 `?v=` 追加到帧图 asset URL，破除 webview 对同路径旧图的缓存
 * （与静帧预览的 `?v=preview_version` 同理；序列帧无后端 mtime 版本号，故用刷新代次兜底）。
 */
let mediaVersion = 0

/** 清空序列帧 LRU 缓存并递增代次——手动刷新时调用，强制重新解码 + 破浏览器缓存 */
export function clearSequenceCache(): void {
  cache.length = 0
  mediaVersion++
}

/** 加载序列帧图片（带 LRU 缓存） */
export async function loadSequenceFrames(
  folderPath: string,
  framePaths: string[],
  maxWidth: number,
): Promise<HTMLImageElement[]> {
  const key = `${folderPath}:${maxWidth}`

  // 命中缓存
  const existing = cache.find(c => c.key === key)
  if (existing) {
    existing.lastUsed = Date.now()
    return existing.frames
  }

  // 降采样：超过 MAX_FRAMES 则均匀取样
  let paths = framePaths
  if (paths.length > MAX_FRAMES) {
    const step = paths.length / MAX_FRAMES
    paths = Array.from({ length: MAX_FRAMES }, (_, i) => framePaths[Math.floor(i * step)])
  }

  // 并行加载图片（追加刷新代次破 webview 缓存，避免同路径素材更新后仍显示旧帧）
  const frames = await Promise.all(
    paths.map(p => loadImage(`${convertFileSrc(p)}?v=${mediaVersion}`))
  )

  // LRU 淘汰
  if (cache.length >= MAX_CACHED) {
    cache.sort((a, b) => a.lastUsed - b.lastUsed)
    cache.shift()
  }

  cache.push({ key, frames, lastUsed: Date.now() })
  return frames
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.onload = () => resolve(img)
    img.onerror = reject
    img.src = src
  })
}
