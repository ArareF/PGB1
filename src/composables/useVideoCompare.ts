import { ref, computed, watch, onBeforeUnmount, type Ref } from 'vue'
import { VIDEO_SYNC_DRIFT_TOLERANCE_SEC, VIDEO_SYNC_CHECK_INTERVAL_MS } from '../config/video'

/** 对比候选（组内一个版本） */
export interface VideoCompareCandidate {
  path: string
  /** 展示标签（「版本 2」/「最新版本」），由侧栏按版本列表规则统一生成 */
  label: string
}

export type VideoCompareLayout = 'side' | 'wipe'

export interface UseVideoCompareOptions {
  /** A 路（主时钟）文件路径 */
  src: Ref<string>
  /** 组内候选版本，旧→新；undefined 或少于 2 个时对比不可用 */
  candidates: Ref<VideoCompareCandidate[] | undefined>
  videoA: Ref<HTMLVideoElement | null>
  videoB: Ref<HTMLVideoElement | null>
}

/**
 * 视频对比播放：模式 / 目标版本 / 偏移 / A→B 同步引擎。
 *
 * A 是主时钟，B 只被动对齐：`B 时间 = A 时间 + offsetSec`。
 * 播放中用定时巡检软对齐（超漂移容差才 seek），显式事件（play/playing/pause/seeked/偏移变化）硬对齐。
 * 不碰 DOM 结构，布局（并排/滑动）与拖线是 VideoPlayer 的展示层职责。
 */
export function useVideoCompare(opts: UseVideoCompareOptions) {
  /** 对比开关（「打开对比 / 关闭对比」按钮） */
  const enabled = ref(false)
  const layout = ref<VideoCompareLayout>('side')
  /** 用户在版本菜单里明确选的 B；null = 用默认（上一版） */
  const pickedPath = ref<string | null>(null)
  const offsetSec = ref(0)

  // ─── 候选解析 ─────────────────────────────────────

  const candidates = computed(() => opts.candidates.value ?? [])
  /** 组内至少两个版本才有对比的意义 */
  const available = computed(() => candidates.value.length >= 2)
  const currentIndex = computed(() => candidates.value.findIndex(c => c.path === opts.src.value))
  const currentLabel = computed(() => candidates.value[currentIndex.value]?.label ?? '')
  /** 默认 B：上一版；A 已是最旧版时退而取下一版（available 保证组里至少还有一个） */
  const defaultCandidate = computed<VideoCompareCandidate | null>(() => {
    const i = currentIndex.value
    if (i < 0) return null
    return candidates.value[i - 1] ?? candidates.value[i + 1] ?? null
  })
  /** 版本菜单的可选项：组内除 A 以外的版本 */
  const pickCandidates = computed(() => candidates.value.filter(c => c.path !== opts.src.value))

  const target = computed<VideoCompareCandidate | null>(() => {
    if (!available.value || !enabled.value) return null
    const picked = pickCandidates.value.find(c => c.path === pickedPath.value)
    return picked ?? defaultCandidate.value
  })
  const isComparing = computed(() => target.value !== null)

  // ─── 状态规则 ─────────────────────────────────────

  function reset() {
    enabled.value = false
    pickedPath.value = null
  }

  // A 换版本：对比保持打开、B 重新解析；用户选的 B 撞上新 A 就退回默认（上一版）
  watch(() => opts.src.value, () => {
    if (pickedPath.value === opts.src.value) pickedPath.value = null
  })

  // 候选列表消失（侧栏换组 / 关闭）→ 关闭对比
  watch(available, (ok) => { if (!ok) reset() })

  // 配对（A 或 B 路径）变化 → 偏移归零
  watch(() => [opts.src.value, target.value?.path], () => {
    offsetSec.value = 0
  })

  function toggle() {
    if (!available.value) return
    enabled.value = !enabled.value
  }

  function pick(path: string) {
    if (!pickCandidates.value.some(c => c.path === path)) return
    pickedPath.value = path
  }

  function toggleLayout() {
    layout.value = layout.value === 'side' ? 'wipe' : 'side'
  }

  // ─── 偏移 ─────────────────────────────────────────

  function nudgeOffset(deltaSec: number) {
    offsetSec.value += deltaSec
    alignB(true)
  }

  function resetOffset() {
    offsetSec.value = 0
    alignB(true)
  }

  // ─── 同步引擎 ─────────────────────────────────────

  let timerId: number | null = null
  /**
   * B.play() 上一次被浏览器拒绝（典型：页面隐藏时 Chromium 主动暂停静音视频「to save power」）。
   * 拒绝后巡检不再重试，否则每 100ms seek + play 一次，B 在原地抽搐；用户的下一次显式动作（硬对齐）会再试。
   */
  let playRejected = false

  function stopLoop() {
    if (timerId !== null) {
      window.clearInterval(timerId)
      timerId = null
    }
  }

  function startLoop() {
    stopLoop()
    timerId = window.setInterval(() => alignB(false), VIDEO_SYNC_CHECK_INTERVAL_MS)
  }

  /**
   * 把 B 对齐到 A。`hard`：无条件 seek；否则只在漂移超容差时 seek（播放中每帧 seek 会卡）。
   * 目标时间越出 B 的时长 → B 暂停并定格在最近的边界帧，等 A 走回范围内再续播。
   */
  function alignB(hard: boolean) {
    const a = opts.videoA.value
    const b = opts.videoB.value
    if (!a || !b) return
    if (!Number.isFinite(b.duration) || b.duration <= 0) return   // metadata 未就绪，loadedmetadata 时再来

    const t = a.currentTime + offsetSec.value
    if (t < 0 || t > b.duration) {
      if (!b.paused) b.pause()
      const edge = t < 0 ? 0 : b.duration
      if (Math.abs(b.currentTime - edge) > VIDEO_SYNC_DRIFT_TOLERANCE_SEC) b.currentTime = edge
      return
    }

    if (hard || Math.abs(b.currentTime - t) > VIDEO_SYNC_DRIFT_TOLERANCE_SEC) b.currentTime = t
    if (!a.paused && !a.ended && b.paused && (hard || !playRejected)) {
      b.play().then(
        () => { playRejected = false },
        (e: unknown) => {
          if (!playRejected) console.warn('[useVideoCompare] B 路 play() 被拒，暂停重试直到下次显式操作:', e)
          playRejected = true
        },
      )
    }
  }

  function onPlayA() {
    alignB(true)
    startLoop()
  }

  /** A 真正出画（缓冲完）时再硬对齐一次：play 到 playing 之间的起播延迟会让先起跑的 B 领先一截 */
  function onPlayingA() {
    alignB(true)
  }

  function onPauseA() {
    stopLoop()
    opts.videoB.value?.pause()
    alignB(true)
  }

  function onSeekedA() {
    alignB(true)
  }

  function onLoadedMetadataB() {
    alignB(true)
    // A 已经在播的时候才开的对比：play 事件早过去了，循环要在这里补开
    const a = opts.videoA.value
    if (a && !a.paused && !a.ended) startLoop()
  }

  /** B 路加载失败：打日志 + 关闭对比，不留一个黑框在那 */
  function onErrorB(e: Event) {
    const el = e.target as HTMLVideoElement | null
    console.error(
      `[useVideoCompare] B 路加载失败 code=${el?.error?.code ?? '?'} msg="${el?.error?.message ?? ''}" ${target.value?.path ?? ''}`,
    )
    reset()
  }

  watch(isComparing, (on) => { if (!on) stopLoop() })
  onBeforeUnmount(stopLoop)

  return {
    // 状态
    enabled,
    layout,
    offsetSec,
    // 派生
    available,
    currentLabel,
    pickCandidates,
    target,
    isComparing,
    // 操作
    toggle,
    pick,
    toggleLayout,
    nudgeOffset,
    resetOffset,
    // A/B 事件
    onPlayA,
    onPlayingA,
    onPauseA,
    onSeekedA,
    onLoadedMetadataB,
    onErrorB,
  }
}
