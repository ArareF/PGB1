import { invoke } from '@tauri-apps/api/core'

// 模块级缓存：`${path}@${maxSize}@${mtime}` → dataURI（null 表示解析失败）
const cache = new Map<string, string | null>()
// 进行中的请求：同一 key 并发时只发一个 invoke
const pending = new Map<string, Promise<string | null>>()

export async function getPsdThumbnail(path: string, maxSize: number): Promise<string | null> {
  // 先拿 mtime，构造带版本的 key，文件修改后自动失效
  let mtime = 0
  try {
    mtime = await invoke<number>('get_file_mtime', { path })
  } catch {
    // stat 失败时退化为不带 mtime 的 key（依然能用，只是不会自动失效）
  }

  const key = `${path}@${maxSize}@${mtime}`

  if (cache.has(key)) return cache.get(key)!

  if (pending.has(key)) return pending.get(key)!

  const promise = invoke<string | null>('extract_psd_thumbnail', { path, maxSize })
    .then(result => {
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
