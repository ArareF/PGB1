import { ref, type Ref, unref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

/** 悬停预览截取字符数 */
export const NOTE_PREVIEW_LIMIT = 39

/** 剥离 markdown 语法，提取纯可见文本（用于进度条计数） */
export function stripMarkdown(text: string): string {
  return text
    .split('\n')
    .map(line => {
      // 移除行首 checklist 前缀
      let stripped = line.replace(/^- \[[x ]\] /i, '')
      // 命名链接 [text](url) → text
      stripped = stripped.replace(/\[([^\]]+)\]\(https?:\/\/[^)]+\)/g, '$1')
      // 裸 URL → 不计入（渲染为可点击链接，不占文本预算）
      stripped = stripped.replace(/https?:\/\/\S+/g, '')
      // 移除粗体标记 **...**
      stripped = stripped.replace(/\*\*(.+?)\*\*/g, '$1')
      // 移除斜体标记 *...*（非 **）
      stripped = stripped.replace(/(?<!\*)\*(?!\*)(.+?)(?<!\*)\*(?!\*)/g, '$1')
      return stripped
    })
    .join('\n')
}

/** 切换指定行的 checkbox 状态，返回新文本 */
export function toggleCheckbox(text: string, lineIndex: number): string {
  const lines = text.split('\n')
  if (lineIndex < 0 || lineIndex >= lines.length) return text
  const line = lines[lineIndex]
  if (line.startsWith('- [ ] ')) {
    lines[lineIndex] = '- [x] ' + line.slice(6)
  } else if (line.startsWith('- [x] ')) {
    lines[lineIndex] = '- [ ] ' + line.slice(6)
  }
  return lines.join('\n')
}

export function useNotes(dirPath: Ref<string> | string) {
  const notes = ref<Record<string, string>>({})
  const loading = ref(false)

  async function loadNotes() {
    const dir = unref(dirPath)
    if (!dir) return
    loading.value = true
    try {
      notes.value = await invoke<Record<string, string>>('get_notes', { dirPath: dir })
    } catch (e) {
      console.error('读取笔记失败:', e)
      notes.value = {}
    } finally {
      loading.value = false
    }
  }

  function getNote(key: string): string | null {
    return notes.value[key] ?? null
  }

  function hasNote(key: string): boolean {
    return !!notes.value[key]
  }

  /** 返回前 39 字符截断预览（剥离 markdown 语法后截取） */
  function hoverPreview(key: string): string {
    const text = notes.value[key]
    if (!text) return ''
    const plain = stripMarkdown(text)
    if (plain.length <= NOTE_PREVIEW_LIMIT) return plain
    return plain.slice(0, NOTE_PREVIEW_LIMIT) + '...'
  }

  /** 进度条状态：当前字符数 / 39 */
  function previewProgress(text: string) {
    const current = text.length
    return {
      current: Math.min(current, NOTE_PREVIEW_LIMIT),
      max: NOTE_PREVIEW_LIMIT,
      percent: Math.min((current / NOTE_PREVIEW_LIMIT) * 100, 100),
    }
  }

  /** 保存笔记并同步本地缓存（乐观更新） */
  async function saveNote(key: string, text: string) {
    const dir = unref(dirPath)
    if (!dir) return
    // 乐观更新
    if (text) {
      notes.value[key] = text
    } else {
      delete notes.value[key]
    }
    try {
      await invoke('set_note', {
        dirPath: dir,
        key,
        note: text || null,
      })
    } catch (e) {
      console.error('保存笔记失败:', e)
      // 回滚：重新读取
      await loadNotes()
    }
  }

  return {
    notes,
    loading,
    loadNotes,
    getNote,
    hasNote,
    hoverPreview,
    previewProgress,
    saveNote,
  }
}
