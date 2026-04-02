<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'

const props = defineProps<{
  /** 视频文件路径（原始路径，内部 convertFileSrc） */
  src: string
  /** 是否全屏模式 */
  isFullscreen?: boolean
}>()

const emit = defineEmits<{
  'toggle-fullscreen': []
}>()

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
  } else if (e.key === 'ArrowLeft') {
    e.preventDefault()
    seekTo(v.currentTime - (e.ctrlKey ? (1 / 30) : 1))
  } else if (e.key === 'ArrowRight') {
    e.preventDefault()
    seekTo(v.currentTime + (e.ctrlKey ? (1 / 30) : 1))
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
</script>

<template>
  <div
    class="preview-video-wrap"
    tabindex="0"
    @keydown="onVideoKeydown"
  >
    <video
      ref="videoRef"
      :key="src"
      :src="convertFileSrc(src)"
      class="preview-video"
      preload="metadata"
      @timeupdate="onVideoTimeUpdate"
      @loadedmetadata="onVideoLoaded"
      @ended="onVideoEnded"
      @click="togglePlay"
    />
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
  </div>
</template>

<style>
/* 非 scoped — 与 FileDetailSidebar 全局样式一致 */

/* ─── 视频预览 ─── */
.preview-video-wrap {
  position: relative;
  width: 100%;
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

.preview-video {
  width: 100%;
  display: block;
  object-fit: contain;
  cursor: pointer;
}

/* 自定义控制条 */
.video-controls {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-3);
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(var(--glass-light-blur));
  -webkit-backdrop-filter: blur(var(--glass-light-blur));
}

.video-play-btn {
  flex-shrink: 0;
  width: 28px;
  height: 28px;
  border: none;
  background: rgba(255, 255, 255, 0.15);
  border-radius: 50%;
  color: #fff;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background var(--duration-fast);
}

.video-play-btn:hover {
  background: rgba(255, 255, 255, 0.25);
}

.video-progress-bar {
  flex: 1;
  height: 4px;
  background: rgba(255, 255, 255, 0.2);
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
  background: #fff;
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
  color: rgba(255, 255, 255, 0.7);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
</style>
