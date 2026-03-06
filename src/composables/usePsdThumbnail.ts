import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'

// 模块级缓存：`${path}@${maxSize}` → asset URL（null 表示解析失败）
// mtime 失效已由 Rust 磁盘缓存处理，前端只缓存 invoke 结果避免重复调用
const cache = new Map<string, string | null>()
// 进行中的请求：同一 key 并发时只发一个 invoke
const pending = new Map<string, Promise<string | null>>()

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
