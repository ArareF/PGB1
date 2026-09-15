<script setup lang="ts">
import { ref, computed, watch, toRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { convertFileSrc } from '@tauri-apps/api/core'
import { VIDEO_FRAME_STEP_SEC, VIDEO_OFFSET_STEP_SEC } from '../config/video'
import { useVideoCompare, type VideoCompareCandidate } from '../composables/useVideoCompare'
import SidebarActionMenu, { type SidebarActionItem } from './SidebarActionMenu.vue'

const props = defineProps<{
  /** 视频文件路径（原始路径，内部 convertFileSrc） */
  src: string
  /** 是否全屏模式 */
  isFullscreen?: boolean
  /**
   * 对比候选（同组版本，旧→新）。不传或少于 2 个时不渲染对比工具栏。
   * 只有任务页预览视频侧栏传这个；项目素材页 / 游戏介绍页保持单路播放。
   */
  compareCandidates?: VideoCompareCandidate[]
}>()

const emit = defineEmits<{
  'toggle-fullscreen': []
  /** 进入对比模式时请求页内全屏（侧栏默认宽度下并排看不清），由侧栏决定是否响应 */
  'request-fullscreen': []
}>()

const { t } = useI18n()

// ─── 视频播放控制 ─────────────────────────────────────

const videoRef = ref<HTMLVideoElement | null>(null)
const isPlaying = ref(false)
const currentTime = ref(0)
const duration = ref(0)
const isSeeking = ref(false)

// 切换视频源时重置播放状态
watch(() => props.src, () => {
  isPlaying.value = false
  currentTime.value = 0
  duration.value = 0
})

function onVideoTimeUpdate() {
  if (!isSeeking.value && videoRef.value) {
    currentTime.value = videoRef.value.currentTime
  }
}

function onVideoLoaded() {
  if (videoRef.value) {
    duration.value = videoRef.value.duration || 0
    currentTime.value = 0
    isPlaying.value = false
  }
}

function onVideoEnded() {
  isPlaying.value = false
}

function togglePlay() {
  const v = videoRef.value
  if (!v) return
  if (v.paused) {
    v.play()
    isPlaying.value = true
  } else {
    v.pause()
    isPlaying.value = false
  }
}

function seekTo(seconds: number) {
  const v = videoRef.value
  if (!v || !duration.value) return
  v.currentTime = Math.max(0, Math.min(duration.value, seconds))
  currentTime.value = v.currentTime
}

function onProgressMouseDown(e: MouseEvent) {
  isSeeking.value = true
  const bar = e.currentTarget as HTMLElement
  doSeekFromBar(e.clientX, bar)

  function onMove(ev: MouseEvent) { doSeekFromBar(ev.clientX, bar) }
  function onUp() {
    isSeeking.value = false
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

function doSeekFromBar(clientX: number, bar: HTMLElement) {
  if (!duration.value) return
  const rect = bar.getBoundingClientRect()
  const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width))
  seekTo(ratio * duration.value)
}

function onVideoKeydown(e: KeyboardEvent) {
  const v = videoRef.value
  if (!v) return
  if (e.key === ' ' || e.code === 'Space') {
    e.preventDefault()
    togglePlay()
  } else if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
    e.preventDefault()
    const dir = e.key === 'ArrowLeft' ? -1 : 1
    // Shift+←/→：对比模式下调 B 路偏移 ±1 帧；其余情况是 A 路步进（Ctrl 逐帧 / 默认 1 秒）
    if (e.shiftKey && compare.isComparing.value) {
      compare.nudgeOffset(dir * VIDEO_FRAME_STEP_SEC)
    } else {
      seekTo(v.currentTime + dir * (e.ctrlKey ? VIDEO_FRAME_STEP_SEC : 1))
    }
  }
}

function formatTime(s: number): string {
  if (!isFinite(s)) return '0:00'
  const m = Math.floor(s / 60)
  const sec = Math.floor(s % 60)
  return `${m}:${sec.toString().padStart(2, '0')}`
}

const progressPercent = computed(() =>
  duration.value > 0 ? (currentTime.value / duration.value) * 100 : 0
)

// ─── 对比播放 ─────────────────────────────────────────

const videoBRef = ref<HTMLVideoElement | null>(null)
const stageRef = ref<HTMLElement | null>(null)

const compare = useVideoCompare({
  src: toRef(props, 'src'),
  candidates: toRef(props, 'compareCandidates'),
  videoA: videoRef,
  videoB: videoBRef,
})

/** 偏移微调组默认收起成一个「对齐」按钮，点开才展示步进按钮 */
const offsetExpanded = ref(false)

// 进入对比 → 请求页内全屏；退出对比不动全屏，但把偏移组收起来
watch(compare.isComparing, (on) => {
  if (on && !props.isFullscreen) emit('request-fullscreen')
  if (!on) offsetExpanded.value = false
})

/** 版本菜单：组内除 A 以外的版本，当前 B 打勾 */
const pickItems = computed<SidebarActionItem[]>(() =>
  compare.pickCandidates.value.map(c => ({
    id: c.path,
    label: c.path === compare.target.value?.path ? `✓ ${c.label}` : c.label,
  }))
)

function formatOffset(s: number): string {
  const sign = s < 0 ? '−' : '+'
  return `${sign}${Math.abs(s).toFixed(2)}s`
}

/** 「对齐」按钮文案：收起且偏移非零时把偏移量带在按钮上，用户不点开也知道当前有偏移 */
const alignLabel = computed(() =>
  !offsetExpanded.value && compare.offsetSec.value !== 0
    ? `${t('videoCompare.align')} ${formatOffset(compare.offsetSec.value)}`
    : t('videoCompare.align')
)

// 滑动布局：分割线位置（0~1，相对画框宽度）
const wipeRatio = ref(0.5)
/** 分割线不允许拖到 100%：B 的宽度按 `槽宽 / (1 − r)` 反推画框宽，r = 1 会除零 */
const WIPE_RATIO_MAX = 0.995
const wipeStyle = computed(() => ({
  '--wipe-x': `${wipeRatio.value * 100}%`,
  '--wipe-r': String(wipeRatio.value),
}))

function onWipeMouseDown(e: MouseEvent) {
  e.preventDefault()
  e.stopPropagation()
  const stage = stageRef.value
  if (!stage) return

  function onMove(ev: MouseEvent) {
    const rect = stage!.getBoundingClientRect()
    if (rect.width <= 0) return
    wipeRatio.value = Math.max(0, Math.min(WIPE_RATIO_MAX, (ev.clientX - rect.left) / rect.width))
  }
  function onUp() {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
  onMove(e)
}
</script>

<template>
  <div
    class="preview-video-wrap"
    tabindex="0"
    @keydown="onVideoKeydown"
  >
    <!-- 画框：单路 / 并排 / 滑动 三种布局都在这一层切换 -->
    <div
      ref="stageRef"
      class="video-stage"
      :class="{
        'layout-side': compare.isComparing.value && compare.layout.value === 'side',
        'layout-wipe': compare.isComparing.value && compare.layout.value === 'wipe',
      }"
      :style="wipeStyle"
    >
      <video
        ref="videoRef"
        :key="src"
        :src="convertFileSrc(src)"
        class="preview-video is-a"
        preload="metadata"
        loop
        @timeupdate="onVideoTimeUpdate"
        @loadedmetadata="onVideoLoaded"
        @ended="onVideoEnded"
        @play="compare.onPlayA"
        @playing="compare.onPlayingA"
        @pause="compare.onPauseA"
        @seeked="compare.onSeekedA"
        @click="togglePlay"
      />
      <template v-if="compare.target.value">
        <!-- B 槽位：并排时 display: contents（B 直接当 flex 项）；滑动时变成右侧裁剪窗，用 overflow 裁而不是给 <video> 上 clip-path
             B 故意不加 loop：它只跟 A，A 循环回到开头会触发 seeked → B 硬对齐一起回去；B 自己循环会和 A 脱钩 -->
        <div class="video-b-slot">
          <video
            ref="videoBRef"
            :key="compare.target.value.path"
            :src="convertFileSrc(compare.target.value.path)"
            class="preview-video is-b"
            preload="auto"
            muted
            @loadedmetadata="compare.onLoadedMetadataB"
            @error="compare.onErrorB"
            @click="togglePlay"
          />
        </div>
        <span class="video-ab-badge is-a">{{ $t('videoCompare.badgeA', { label: compare.currentLabel.value }) }}</span>
        <span class="video-ab-badge is-b">{{ $t('videoCompare.badgeB', { label: compare.target.value.label }) }}</span>
        <div
          v-if="compare.layout.value === 'wipe'"
          class="video-wipe-line"
          :title="$t('videoCompare.wipeHint')"
          @mousedown="onWipeMouseDown"
        >
          <div class="video-wipe-handle" />
        </div>
      </template>
    </div>
    <button class="preview-fullscreen-btn" :title="isFullscreen ? $t('common.exitFullscreen') : $t('common.fullscreen')" @click.stop="emit('toggle-fullscreen')">
      <svg v-if="!isFullscreen" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
        <polyline points="15 3 21 3 21 9" /><polyline points="9 21 3 21 3 15" />
        <line x1="21" y1="3" x2="14" y2="10" /><line x1="3" y1="21" x2="10" y2="14" />
      </svg>
      <svg v-else width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
        <polyline points="4 14 10 14 10 20" /><polyline points="20 10 14 10 14 4" />
        <line x1="10" y1="14" x2="3" y2="21" /><line x1="21" y1="3" x2="14" y2="10" />
      </svg>
    </button>
    <!-- 自定义控制条 -->
    <div class="video-controls">
      <button class="video-play-btn" @click.stop="togglePlay">
        <!-- 播放图标 -->
        <svg v-if="!isPlaying" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
          <polygon points="5,3 19,12 5,21" />
        </svg>
        <!-- 暂停图标 -->
        <svg v-else width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
          <rect x="6" y="4" width="4" height="16" />
          <rect x="14" y="4" width="4" height="16" />
        </svg>
      </button>
      <div
        class="video-progress-bar"
        @mousedown="onProgressMouseDown"
      >
        <div class="video-progress-fill" :style="{ width: progressPercent + '%' }" />
        <div class="video-progress-thumb" :style="{ left: progressPercent + '%' }" />
      </div>
      <span class="video-time">{{ formatTime(currentTime) }} / {{ formatTime(duration) }}</span>
    </div>
    <!-- 对比工具栏：开关 → 版本菜单 → 布局循环 → 对齐（展开偏移微调） -->
    <div v-if="compare.available.value" class="video-compare-bar">
      <div class="vc-group">
        <button class="vc-btn" :class="{ active: compare.isComparing.value }" @click="compare.toggle()">
          {{ compare.isComparing.value ? $t('videoCompare.close') : $t('videoCompare.open') }}
        </button>
        <span v-if="compare.target.value" class="vc-pick" :title="$t('videoCompare.pickHint')">
          <SidebarActionMenu :items="pickItems" :label="compare.target.value.label" @select="compare.pick" />
        </span>
      </div>
      <template v-if="compare.isComparing.value">
        <div class="vc-group">
          <button class="vc-btn" :title="$t('videoCompare.layoutHint')" @click="compare.toggleLayout()">
            {{ compare.layout.value === 'side' ? $t('videoCompare.layoutSide') : $t('videoCompare.layoutWipe') }}
          </button>
        </div>
        <div class="vc-group" :title="$t('videoCompare.offsetHint')">
          <button class="vc-btn" :class="{ active: offsetExpanded }" @click="offsetExpanded = !offsetExpanded">
            {{ alignLabel }}
          </button>
          <template v-if="offsetExpanded">
            <button class="vc-btn" @click="compare.nudgeOffset(-VIDEO_OFFSET_STEP_SEC)">{{ $t('videoCompare.minusSecond') }}</button>
            <button class="vc-btn" @click="compare.nudgeOffset(-VIDEO_FRAME_STEP_SEC)">{{ $t('videoCompare.minusFrame') }}</button>
            <span class="vc-offset">{{ formatOffset(compare.offsetSec.value) }}</span>
            <button class="vc-btn" @click="compare.nudgeOffset(VIDEO_FRAME_STEP_SEC)">{{ $t('videoCompare.plusFrame') }}</button>
            <button class="vc-btn" @click="compare.nudgeOffset(VIDEO_OFFSET_STEP_SEC)">{{ $t('videoCompare.plusSecond') }}</button>
            <button class="vc-btn" :disabled="compare.offsetSec.value === 0" @click="compare.resetOffset()">{{ $t('videoCompare.resetOffset') }}</button>
          </template>
        </div>
      </template>
    </div>
  </div>
</template>

<style>
/* 非 scoped — 与 FileDetailSidebar 全局样式一致 */

/* ─── 视频预览 ─── */
.preview-video-wrap {
  position: relative;
  width: 100%;
  max-height: var(--sidebar-preview-max-height);
  border-radius: var(--radius-lg);
  overflow: hidden;
  background: var(--color-neutral-900);
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  outline: none;
}

.preview-video-wrap:focus-within {
  box-shadow: 0 0 0 2px var(--color-primary-500);
}

/* 画框：行向 flex，单路时 A 独占，并排时 A/B 各半；高度由 A 的固有宽高比撑起，超出上限时整体收缩 */
.video-stage {
  position: relative;
  display: flex;
  width: 100%;
  min-height: 0;
  /* 滑动布局分割线位置（百分比 / 无单位比例），运行时由 wipeRatio 通过内联样式覆盖 */
  --wipe-x: 50%;
  --wipe-r: 0.5;
}

.preview-video {
  flex: 1 1 0;
  min-width: 0;
  min-height: 0;
  display: block;
  object-fit: contain;
  cursor: pointer;
}

/* B 槽位：并排时不占层级，B 直接是 .video-stage 的 flex 项 */
.video-b-slot {
  display: contents;
}

/* 滑动布局：槽位变成分割线右侧的裁剪窗（只盖住 A 的右半，A 的左半始终露着）。
   故意不用 clip-path 裁 <video>：视频是独立合成层，clip-path 走的是遮罩路径，
   在 WebView2 里两个视频层叠加 + 遮罩会出渲染问题；overflow 裁剪是最朴素的 ClipNode */
.video-stage.layout-wipe .video-b-slot {
  display: block;
  position: absolute;
  top: 0;
  bottom: 0;
  left: var(--wipe-x);
  right: 0;
  overflow: hidden;
}

/* 裁剪窗里的 B 撑回整个画框的宽度（槽宽 = 画框宽 × (1 − r)），右对齐，contain 几何与 A 完全重合 */
.video-stage.layout-wipe .preview-video.is-b {
  position: absolute;
  top: 0;
  bottom: 0;
  right: 0;
  height: 100%;
  width: calc(100% / (1 - var(--wipe-r)));
}

.video-wipe-line {
  position: absolute;
  top: 0;
  bottom: 0;
  left: var(--wipe-x);
  width: 2px;
  transform: translateX(-50%);
  background: var(--video-btn-text);
  cursor: ew-resize;
  z-index: 2;
}

/* 拉宽命中区，2px 线太难抓 */
.video-wipe-line::before {
  content: '';
  position: absolute;
  top: 0;
  bottom: 0;
  left: calc(-1 * var(--spacing-2));
  right: calc(-1 * var(--spacing-2));
}

.video-wipe-handle {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 20px;
  height: 20px;
  transform: translate(-50%, -50%);
  border-radius: 50%;
  background: var(--video-btn-text);
  box-shadow: var(--shadow-button-md);
}

/* A / B 角标：贴画框底部，避开右上角的全屏按钮 */
.video-ab-badge {
  position: absolute;
  bottom: var(--spacing-2);
  left: var(--spacing-2);
  padding: var(--spacing-1) var(--spacing-2);
  border-radius: var(--radius-sm);
  background: var(--overlay-btn-bg);
  color: var(--overlay-btn-text);
  font-size: var(--text-xs);
  white-space: nowrap;
  pointer-events: none;
  z-index: 1;
}

.video-stage.layout-side .video-ab-badge.is-b {
  left: calc(50% + var(--spacing-2));
}

.video-stage.layout-wipe .video-ab-badge.is-b {
  left: auto;
  right: var(--spacing-2);
}

/* 自定义控制条 */
.video-controls {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-3);
  background: var(--video-control-bg);
  backdrop-filter: blur(var(--glass-light-blur));
  -webkit-backdrop-filter: blur(var(--glass-light-blur));
}

.video-play-btn {
  flex-shrink: 0;
  width: 28px;
  height: 28px;
  border: none;
  background: var(--video-btn-bg);
  border-radius: 50%;
  color: var(--video-btn-text);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background var(--duration-fast);
}

.video-play-btn:hover {
  background: var(--video-btn-bg-hover);
}

.video-progress-bar {
  flex: 1;
  height: 4px;
  background: var(--video-track-bg);
  border-radius: 2px;
  position: relative;
  cursor: pointer;
}

.video-progress-bar:hover {
  height: 6px;
}

.video-progress-fill {
  height: 100%;
  background: var(--color-primary-500);
  border-radius: 2px;
  pointer-events: none;
}

.video-progress-thumb {
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  width: 10px;
  height: 10px;
  background: var(--video-btn-text);
  border-radius: 50%;
  pointer-events: none;
  opacity: 0;
  transition: opacity var(--duration-fast);
}

.video-progress-bar:hover .video-progress-thumb {
  opacity: 1;
}

.video-time {
  flex-shrink: 0;
  font-size: var(--text-xs);
  color: var(--video-time-text);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* ─── 对比工具栏 ───
   与 .video-controls 是同层兄弟，backdrop-filter 只能留给前者（见 glass.css 兄弟冲突规则） */
.video-compare-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-3);
  background: var(--video-control-bg);
}

.vc-group {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
}

.vc-btn {
  padding: var(--spacing-1) var(--spacing-2);
  border: none;
  border-radius: var(--radius-sm);
  background: var(--video-btn-bg);
  color: var(--video-time-text);
  font-size: var(--text-xs);
  font-family: inherit;
  white-space: nowrap;
  cursor: pointer;
  transition: background var(--duration-fast), color var(--duration-fast);
}

.vc-btn:hover:not(:disabled) {
  background: var(--video-btn-bg-hover);
  color: var(--video-btn-text);
}

.vc-btn.active {
  background: var(--color-primary-500);
  color: var(--video-btn-text);
}

.vc-btn:disabled {
  opacity: var(--button-disabled-opacity);
  cursor: not-allowed;
}

.vc-offset {
  min-width: 7ch;
  text-align: center;
  font-size: var(--text-xs);
  color: var(--video-btn-text);
  font-variant-numeric: tabular-nums;
}

/* 「所选」下拉复用 SidebarActionMenu，触发按钮按 .vc-btn 的尺寸与配色重排（弹层本身不动） */
.video-compare-bar .sidebar-action-btn {
  padding: var(--spacing-1) var(--spacing-2);
  border: none;
  border-radius: var(--radius-sm);
  background: var(--video-btn-bg);
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  color: var(--video-time-text);
  font-size: var(--text-xs);
  gap: var(--spacing-1);
}

.video-compare-bar .sidebar-action-btn:hover {
  background: var(--video-btn-bg-hover);
  color: var(--video-btn-text);
}
</style>
