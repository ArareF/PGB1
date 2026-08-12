import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'

// 模块级缓存：`${path}@${maxSize}` → asset URL（null 表示解析失败）
// mtime 失效已由 Rust 磁盘缓存处理，前端只缓存 invoke 结果避免重复调用
const cache = new Map<string, string | null>()
// 进行中的请求：同一 key 并发时只发一个 invoke
const pending = new Map<string, Promise<string | null>>()

/** 清除指定文件的 JS 缓存，用于 mtime 变化后强制重新生成 */
export function invalidatePsdCache(path: string, maxSize: number): void {
  const key = `${path}@${maxSize}`
  cache.delete(key)
  // pending 不清除：若有进行中的请求，完成后会以最新 Rust 结果覆盖缓存
}

/**
 * 清空整个 PSD 缩略图 JS 缓存——手动刷新时调用。
 * JS Map 只按 path+maxSize 缓存不含 mtime，清掉后会重新 invoke；Rust 侧按 mtime 决定复用/重生，
 * 缓存文件名含 mtime 哈希，故素材更新后拿到的是新路径新图，不会再吃旧缓存。
 * pending 同 invalidatePsdCache：不清除，进行中请求完成后以最新 Rust 结果覆盖。
 */
export function clearPsdThumbnailCache(): void {
  cache.clear()
}

export async function getPsdThumbnail(path: string, maxSize: number): Promise<string | null> {
  const key = `${path}@${maxSize}`

  if (cache.has(key)) return cache.get(key)!

  if (pending.has(key)) return pending.get(key)!

  const promise = invoke<string | null>('extract_psd_thumbnail', { path, maxSize })
    .then(cachePath => {
      // Rust 返回磁盘缓存文件路径，用 convertFileSrc 转为 asset URL
      const result = cachePath ? convertFileSrc(cachePath) : null
      cache.set(key, result)
      pending.delete(key)
      return result
    })
    .catch(e => {
      console.error('提取 PSD 缩略图失败:', e)
      cache.set(key, null)
      pending.delete(key)
      return null
    })

  pending.set(key, promise)
  return promise
}
