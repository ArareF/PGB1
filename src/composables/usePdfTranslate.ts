import { ref, computed, watch, type ComputedRef, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import i18n from '../i18n'
import { useSettings } from './useSettings'

// ─── 类型 ────────────────────────────────────────────

export type PdfTranslateState = 'idle' | 'extracting' | 'translating' | 'building' | 'done' | 'error'

interface RetryInfo {
  page: number
  attempt: number
  maxRetries: number
  waitSecs: number
  error: string
}

interface PdfTranslateSession {
  state: Ref<PdfTranslateState>
  progress: Ref<{ current: number; total: number }>
  error: Ref<string>
  outputPath: Ref<string>
  showingTranslated: Ref<boolean>
  retryInfo: Ref<RetryInfo | null>
}

// ─── 模块级全局状态 ──────────────────────────────────

/** 以原始 PDF 路径为 key 管理翻译会话 */
const sessions = new Map<string, PdfTranslateSession>()

/** 当前正在翻译的文件路径（用于路由重试事件） */
let activeFilePath = ''

// 模块级事件监听（只注册一次）
let retryListenerReady = false

function ensureRetryListener() {
  if (retryListenerReady) return
  retryListenerReady = true
  listen<RetryInfo>('pdf-translate-retry', (event) => {
    if (!activeFilePath) return
    const session = sessions.get(activeFilePath)
    if (session) {
      session.retryInfo.value = event.payload
    }
  })
}

// ─── Session 管理 ────────────────────────────────────

function getOrCreateSession(filePath: string): PdfTranslateSession {
  const existing = sessions.get(filePath)
  if (existing) return existing

  const session: PdfTranslateSession = {
    state: ref<PdfTranslateState>('idle'),
    progress: ref({ current: 0, total: 0 }),
    error: ref(''),
    outputPath: ref(''),
    showingTranslated: ref(false),
    retryInfo: ref(null),
  }
  sessions.set(filePath, session)
  return session
}

// ─── 已有译文检测 ────────────────────────────────────

/** 检测 _zh.pdf 是否存在，存在则直接设为 done 态 */
async function checkExisting(filePath: string, session: PdfTranslateSession): Promise<void> {
  // 已在翻译中或已完成的不重新检测
  if (session.state.value !== 'idle') return
  try {
    const existingPath = await invoke<string | null>('check_translated_pdf_exists', { path: filePath })
    // 检测期间状态可能已被用户操作改变
    if (existingPath && session.state.value === 'idle') {
      session.outputPath.value = existingPath
      session.state.value = 'done'
    }
  } catch {
    // 检测失败静默忽略，保持 idle
  }
}

// 已检测过的路径集合，避免重复 invoke
const checkedPaths = new Set<string>()

// ─── 翻译核心逻辑 ────────────────────────────────────

async function startTranslation(
  filePath: string,
  session: PdfTranslateSession,
): Promise<void> {
  const t = i18n.global.t

  if (session.state.value !== 'idle') return

  // 加载设置（通过 useSettings 单例获取，与 FileDetailSidebar 共享缓存）
  const { loadSettings } = useSettings()
  const settings = await loadSettings()
  const apiKey = settings?.translation?.apiKey
  const model = settings?.translation?.model

  if (!apiKey) {
    session.state.value = 'error'
    session.error.value = t('fileDetail.translatePdfNoKey')
    return
  }

  activeFilePath = filePath
  ensureRetryListener()

  try {
    // 提取文字
    session.state.value = 'extracting'
    session.retryInfo.value = null
    const pages = await invoke<string[]>('extract_pdf_pages_text', { path: filePath })

    session.progress.value = { current: 0, total: pages.length }

    // 逐页翻译（跳过空白页）
    session.state.value = 'translating'
    const translations: string[] = []

    for (let i = 0; i < pages.length; i++) {
      session.progress.value = { current: i + 1, total: pages.length }
      session.retryInfo.value = null

      if (!pages[i].trim()) {
        translations.push('')
        continue
      }

      const translated = await invoke<string>('translate_text_once', {
        apiKey,
        model,
        text: pages[i],
        pageIndex: i,
      })
      translations.push(translated)
    }

    // 生成 PDF
    session.state.value = 'building'
    session.retryInfo.value = null
    const outputPath = await invoke<string>('build_translated_pdf', {
      path: filePath,
      translations,
    })

    session.outputPath.value = outputPath
    session.showingTranslated.value = true
    session.state.value = 'done'
  } catch (e: any) {
    session.state.value = 'error'
    session.retryInfo.value = null
    const msg = String(e)
    if (msg.includes('扫描版')) {
      session.error.value = t('fileDetail.translatePdfScanned')
    } else {
      session.error.value = `${t('fileDetail.translatePdfError')}: ${msg}`
    }
    console.error('PDF 翻译失败:', e)
  } finally {
    if (activeFilePath === filePath) {
      activeFilePath = ''
    }
  }
}

// ─── Composable 入口 ────────────────────────────────

export function usePdfTranslate(filePath: ComputedRef<string>) {
  // 当前文件对应的 session（响应式切换）
  const currentSession = computed(() => {
    const fp = filePath.value
    if (!fp) return null
    return getOrCreateSession(fp)
  })

  // 文件路径变化时检测已有译文
  watch(filePath, (fp) => {
    if (fp && !checkedPaths.has(fp)) {
      checkedPaths.add(fp)
      const session = getOrCreateSession(fp)
      checkExisting(fp, session)
    }
  }, { immediate: true })

  // 代理当前 session 的 ref（无 session 时返回默认值）
  const state = computed(() => currentSession.value?.state.value ?? 'idle')
  const progress = computed(() => currentSession.value?.progress.value ?? { current: 0, total: 0 })
  const error = computed(() => currentSession.value?.error.value ?? '')
  const outputPath = computed(() => currentSession.value?.outputPath.value ?? '')
  const showingTranslated = computed(() => currentSession.value?.showingTranslated.value ?? false)
  const retryInfo = computed(() => currentSession.value?.retryInfo.value ?? null)

  const activePdfSrc = computed(() => {
    if (showingTranslated.value && outputPath.value) {
      return convertFileSrc(outputPath.value)
    }
    const fp = filePath.value
    return fp ? convertFileSrc(fp) : ''
  })

  function start() {
    const fp = filePath.value
    if (!fp) return
    const session = getOrCreateSession(fp)
    startTranslation(fp, session)
  }

  function toggleView() {
    const session = currentSession.value
    if (session) {
      session.showingTranslated.value = !session.showingTranslated.value
    }
  }

  function reset() {
    const session = currentSession.value
    if (session) {
      session.state.value = 'idle'
      session.showingTranslated.value = false
      session.error.value = ''
      session.retryInfo.value = null
    }
  }

  return {
    state,
    progress,
    error,
    outputPath,
    showingTranslated,
    retryInfo,
    activePdfSrc,
    start,
    toggleView,
    reset,
  }
}
