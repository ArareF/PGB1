<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import { useNavigation } from '../composables/useNavigation'
import { useMaterials, materialUid, type MaterialInfo } from '../composables/useMaterials'
import { useSettings } from '../composables/useSettings'
import MaterialCard from '../components/MaterialCard.vue'
import { useMultiSelect } from '../composables/useMultiSelect'
import PageGuideOverlay from '../components/PageGuideOverlay.vue'
import { PAGE_GUIDE_ANNOTATIONS } from '../config/onboarding'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const { setNavigation } = useNavigation()
const { materials, loading, loadMaterials } = useMaterials()
const { settings, loadSettings, saveSettings } = useSettings()
const tpPresetOpen = ref(false)

const taskId = route.params.taskId as string
const showGuide = ref(false)
const taskPath = route.query.taskPath as string

// ─── 素材过滤 ────────────────────────────────────────

const pendingImages = computed(() =>
  materials.value.filter(m =>
    m.material_type === 'image' &&
    m.progress !== 'done' &&
    m.progress !== 'uploaded'
  )
)

const pendingSequences = computed(() =>
  materials.value.filter(m =>
    m.material_type === 'sequence' &&
    m.progress !== 'done' &&
    m.progress !== 'uploaded'
  )
)

const totalPending = computed(() => pendingImages.value.length + pendingSequences.value.length)

// ─── 选中状态 ────────────────────────────────────────

const cardAreaRef = ref<HTMLElement | null>(null)

const {
  selectedPaths, toggleSelection: toggleItem,
  isSelecting, selectionRect, onContainerMouseDown, onContainerScroll,
} = useMultiSelect({
  allPaths: computed(() => [
    ...pendingImages.value.map(materialUid),
    ...pendingSequences.value.map(materialUid),
  ]),
  alwaysEnabled: true,
  rubberBand: { containerRef: cardAreaRef, cardSelector: '.material-card[data-path]' },
})

function toggleSelectAll() {
  // 全选/取消全选仅针对静帧（序列帧需手动标注FPS）
  const allImagesSelected = pendingImages.value.every(m => selectedPaths.value.has(materialUid(m)))
  const newSet = new Set(selectedPaths.value)
  if (allImagesSelected) {
    pendingImages.value.forEach(m => newSet.delete(materialUid(m)))
  } else {
    pendingImages.value.forEach(m => newSet.add(materialUid(m)))
  }
  selectedPaths.value = newSet
}

const selectedImageCount = computed(() =>
  pendingImages.value.filter(m => selectedPaths.value.has(materialUid(m))).length
)

const annotatedSequenceCount = computed(() =>
  pendingSequences.value.filter(m => fpsMap.value.has(materialUid(m))).length
)

// 当前选中的序列帧（用于判断应用按钮是否可用）
const selectedSequencePaths = computed(() =>
  pendingSequences.value.filter(m => selectedPaths.value.has(materialUid(m)))
)

// ─── FPS 标注（批次标注模式） ──────────────────────────

/** key = materialUid：未规范化目录里多个序列共用 path，用 path 会串标注 */
const fpsMap = ref<Map<string, number>>(new Map())
const fpsInput = ref('')

function fpsLabelFor(m: MaterialInfo): string | undefined {
  const fps = fpsMap.value.get(materialUid(m))
  return fps !== undefined ? `${fps}fps` : undefined
}

function applyFps() {
  const fps = Number(fpsInput.value)
  if (isNaN(fps) || fps < 1 || fps > 120) return
  if (selectedSequencePaths.value.length === 0) return

  const newMap = new Map(fpsMap.value)
  selectedSequencePaths.value.forEach(m => {
    const uid = materialUid(m)
    if (newMap.get(uid) === fps) {
      newMap.delete(uid)  // 同值再次应用 → 清除
    } else {
      newMap.set(uid, fps)
    }
  })
  fpsMap.value = newMap
}

const fpsInputValid = computed(() => {
  const n = Number(fpsInput.value)
  return fpsInput.value !== '' && !isNaN(n) && n >= 1 && n <= 120
})

// ─── 校验 ────────────────────────────────────────────

const canStart = computed(() => {
  const hasImages = selectedImageCount.value > 0
  // 有效序列帧 = 已选中 且 已标注 FPS
  const hasSequences = pendingSequences.value.some(
    m => selectedPaths.value.has(materialUid(m)) && fpsMap.value.has(materialUid(m))
  )
  return hasImages || hasSequences
})

// ─── 转换执行 ────────────────────────────────────────

const isConverting = ref(false)
const conversionProgress = ref({ current: 0, total: 0 })
const failedSequences = ref<string[]>([])
const sequenceError = ref('')
let unlistenOrganized: (() => void) | null = null
let unlistenFailed: (() => void) | null = null
/** 当前会话选中的素材名白名单，用于过滤跨会话的残留事件 */
let expectedNames = new Set<string>()
/** 已完成的素材名去重集，防止同一素材多次计数 */
let organizedNames = new Set<string>()
/** 已跳过（未发布）的序列帧名去重集，对称防止同名重复扣减 total */
let failedNames = new Set<string>()

async function handleStart() {
  if (!canStart.value || !taskPath) return

  const images: Record<string, number> = {}
  const sequences: { name: string; fps: number }[] = []

  for (const img of pendingImages.value) {
    if (selectedPaths.value.has(materialUid(img))) {
      images[img.name] = 0
    }
  }
  fpsMap.value.forEach((fps, uid) => {
    if (!selectedPaths.value.has(uid)) return  // 未选中的跳过
    const seq = pendingSequences.value.find(m => materialUid(m) === uid)
    if (seq) sequences.push({ name: seq.name, fps })
  })

  isConverting.value = true
  failedSequences.value = []
  sequenceError.value = ''
  expectedNames = new Set([...Object.keys(images), ...sequences.map(s => s.name)])
  organizedNames = new Set()
  failedNames = new Set()
  conversionProgress.value = { current: 0, total: expectedNames.size }

  try {
    // 清理旧会话（防止残留 watcher 的 500ms 延迟任务泄漏事件）
    if (unlistenOrganized) unlistenOrganized()
    if (unlistenFailed) unlistenFailed()
    try { await invoke('stop_conversion') } catch { /* 无旧会话时静默忽略 */ }
    unlistenOrganized = await listen<string>('conversion-organized', (event) => {
      const name = event.payload
      // 白名单过滤 + 去重：只统计当前会话选中的素材，每个名字只计一次
      if (expectedNames.has(name) && !organizedNames.has(name)) {
        organizedNames.add(name)
        conversionProgress.value.current = organizedNames.size
      }
    })
    unlistenFailed = await listen<string>('sequence-conversion-failed', (event) => {
      const name = event.payload
      // 白名单过滤 + 去重：跳过的序列帧从 total 中剔除，保证 current/total 能正常收敛到相等
      if (expectedNames.has(name) && !failedNames.has(name)) {
        failedNames.add(name)
        conversionProgress.value.total -= 1
        failedSequences.value.push(name)
      }
    })

    if (!settings.value) throw new Error('应用设置未加载')

    await invoke('start_conversion', {
      request: {
        task_path: taskPath,
        images,
        sequences,
        imagine_path: settings.value.workflow.imaginePath,
        texture_packer_cli_path: settings.value.workflow.texturePackerCliPath,
        texture_packer_gui_path: settings.value.workflow.texturePackerGuiPath,
        tp_scale: settings.value.workflow.tpScale,
        tp_webp_quality: settings.value.workflow.tpWebpQuality,
      }
    })
  } catch (err) {
    console.error('转换流程启动失败:', err)
    isConverting.value = false
    return
  }

  // 序列帧转换独立处理 —— 失败不应终止静帧的 Imagine 流程
  if (sequences.length > 0) {
    try {
      await invoke('execute_sequence_conversion', { sequences })
    } catch (seqErr) {
      console.error('序列帧转换失败:', seqErr)
      conversionProgress.value.total -= sequences.length
      sequenceError.value = String(seqErr)
    }
  }
}

async function handleFinish() {
  try {
    await invoke('stop_conversion')
  } catch (err) {
    console.error('停止转换失败:', err)
  } finally {
    isConverting.value = false
    if (unlistenOrganized) { unlistenOrganized(); unlistenOrganized = null }
    if (unlistenFailed) { unlistenFailed(); unlistenFailed = null }
    router.back()
  }
}

onUnmounted(() => {
  if (unlistenOrganized) { unlistenOrganized(); unlistenOrganized = null }
  if (unlistenFailed) { unlistenFailed(); unlistenFailed = null }
  // 组件卸载时清理转换会话，防止残留 watcher 持续运行
  if (isConverting.value) {
    invoke('stop_conversion').catch(() => {})
  }
})

// ─── TP 预设输入 + 失焦保存 ──────────────────────────
function onTpScaleInput(e: Event) {
  if (!settings.value) return
  settings.value.workflow.tpScale = Number((e.target as HTMLInputElement).value) || 0
}
function onTpWebpQualityInput(e: Event) {
  if (!settings.value) return
  settings.value.workflow.tpWebpQuality = Math.round(Number((e.target as HTMLInputElement).value) || 0)
}
function onTpPresetBlur() {
  if (settings.value) saveSettings(settings.value)
}

// ─── 初始化 ──────────────────────────────────────────

onMounted(async () => {
  setNavigation({
    title: `${t('task.convert')} · ${taskId}`,
    showBackButton: true,
    onBack: () => router.back(),
    actions: [],
    moreMenuItems: [
      { id: 'page-guide', label: t('common.pageGuide'), handler: () => { showGuide.value = true } },
    ],
  })
  await Promise.all([
    taskPath ? loadMaterials(taskPath) : Promise.resolve(),
    loadSettings(),
  ])
  // 默认全选静帧（序列帧需手动标注FPS）
  const all = new Set<string>()
  pendingImages.value.forEach(m => all.add(materialUid(m)))
  selectedPaths.value = all
})
</script>

<template>
  <!-- 素材区（占满 main-content） -->
  <div
    ref="cardAreaRef"
    class="card-area custom-scroll"
    @mousedown="onContainerMouseDown"
    @scroll="onContainerScroll"
  >
    <p v-if="loading" class="hint-text">{{ $t('common.scanning') }}</p>
    <div v-else-if="totalPending === 0" class="hint-text">{{ $t('convert.noMaterials') }}</div>
    <template v-else>
      <!-- 静帧分区 -->
      <div v-if="pendingImages.length > 0" class="section">
        <p class="section-label">{{ $t('convert.imageSection') }} ({{ pendingImages.length }})</p>
        <div class="material-grid">
          <MaterialCard
            v-for="m in pendingImages"
            :key="materialUid(m)"
            :material="m"
            :multi-select="true"
            :checked="selectedPaths.has(materialUid(m))"
            class="mini-card"
            @click="toggleItem(materialUid(m))"
          />
        </div>
      </div>

      <!-- 序列帧分区 -->
      <div v-if="pendingSequences.length > 0" class="section">
        <p class="section-label">{{ $t('convert.sequenceSection') }} ({{ pendingSequences.length }})</p>
        <div class="material-grid">
          <MaterialCard
            v-for="m in pendingSequences"
            :key="materialUid(m)"
            :material="m"
            :multi-select="true"
            :checked="selectedPaths.has(materialUid(m))"
            :scale-label="fpsLabelFor(m)"
            class="mini-card"
            @click="toggleItem(materialUid(m))"
          />
        </div>
      </div>
    </template>
  </div>

  <!-- 控制面板：Teleport 到 #content-row，作为独立毛玻璃板块 -->
  <Teleport to="#content-row">
    <aside class="control-panel convert-control-panel">
      <!-- 选择模式 -->
      <div v-if="!isConverting" class="panel-body">
        <p class="panel-title">{{ $t('convert.title') }}</p>

        <div class="stats">
          <div class="stat-row">
            <span class="stat-label">{{ $t('convert.imageTab') }}</span>
            <span class="stat-value">{{ selectedImageCount }} / {{ pendingImages.length }}</span>
          </div>
          <div class="stat-row">
            <span class="stat-label">{{ $t('convert.sequenceTab') }}</span>
            <span class="stat-value">{{ annotatedSequenceCount }} / {{ pendingSequences.length }}</span>
          </div>
        </div>

        <button v-if="pendingImages.length > 0" class="ghost-btn" @click="toggleSelectAll">
          {{ pendingImages.every(m => selectedPaths.has(materialUid(m))) ? $t('common.deselectAll') : $t('common.selectAll') }}
        </button>

        <!-- 序列帧 FPS 标注区 -->
        <template v-if="pendingSequences.length > 0">
          <div class="divider" />
          <p class="panel-subtitle">{{ $t('convert.sequenceFps') }}</p>
          <div class="fps-annotate-row">
            <div class="custom-input-wrapper">
              <input
                type="text"
                class="custom-input"
                placeholder="24"
                maxlength="3"
                :value="fpsInput"
                @input="fpsInput = ($event.target as HTMLInputElement).value.replace(/[^\d]/g, '')"
              />
              <span class="input-suffix">fps</span>
            </div>
          </div>
          <button
            class="apply-btn"
            :disabled="selectedSequencePaths.length === 0 || !fpsInputValid"
            @click="applyFps"
          >
            {{ $t('convert.applyToSelected') }} ({{ selectedSequencePaths.length }})
          </button>
          <div class="seq-stat">
            <span class="stat-label">{{ $t('convert.annotated') }}</span>
            <span class="stat-value">{{ annotatedSequenceCount }} / {{ pendingSequences.length }}</span>
          </div>
        </template>

        <!-- TP 预设折叠面板 -->
        <div class="divider" />
        <div class="tp-preset-section">
          <button class="tp-preset-toggle" @click="tpPresetOpen = !tpPresetOpen">
            <span>{{ $t('convert.tpPreset') }}</span>
            <svg
              class="tp-preset-arrow"
              :class="{ open: tpPresetOpen }"
              width="12" height="12" viewBox="0 0 12 12"
            >
              <path d="M3 4.5L6 7.5L9 4.5" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </button>
          <div v-show="tpPresetOpen" class="tp-preset-body">
            <div class="tp-preset-row">
              <label class="tp-preset-label">{{ $t('convert.tpScale') }}</label>
              <div class="custom-input-wrapper">
                <input
                  type="text"
                  class="custom-input"
                  :value="settings?.workflow.tpScale"
                  @input="onTpScaleInput"
                  @blur="onTpPresetBlur"
                />
              </div>
            </div>
            <div class="tp-preset-row">
              <label class="tp-preset-label">{{ $t('convert.tpWebpQuality') }}</label>
              <div class="custom-input-wrapper">
                <input
                  type="text"
                  class="custom-input"
                  :value="settings?.workflow.tpWebpQuality"
                  @input="onTpWebpQualityInput"
                  @blur="onTpPresetBlur"
                />
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 进度模式 -->
      <div v-else class="panel-body">
        <p class="panel-title">{{ $t('convert.converting') }}</p>

        <div class="progress-section">
          <div class="progress-count">
            {{ conversionProgress.current }} / {{ conversionProgress.total }}
          </div>
          <div class="progress-track">
            <div
              class="progress-fill"
              :style="{ width: conversionProgress.total > 0 ? (conversionProgress.current / conversionProgress.total * 100) + '%' : '0%' }"
            />
          </div>
          <p class="progress-hint">
            <template v-if="conversionProgress.current < conversionProgress.total">
              {{ $t('convert.externalToolHint') }}
            </template>
            <template v-else-if="conversionProgress.total > 0">
              {{ $t('convert.conversionComplete') }}
            </template>
          </p>
          <div v-if="sequenceError" class="failed-list">
            <p class="failed-title">{{ $t('convert.sequenceStartFailed') }}</p>
            <p class="failed-item">{{ sequenceError }}</p>
          </div>
          <div v-if="failedSequences.length > 0" class="failed-list">
            <p class="failed-title">{{ $t('convert.skippedSequences') }}</p>
            <p v-for="name in failedSequences" :key="name" class="failed-item">{{ name }}</p>
          </div>
        </div>
      </div>

      <div class="panel-footer">
        <template v-if="!isConverting">
          <button class="cancel-btn" @click="router.back()">{{ $t('common.cancel') }}</button>
          <button class="execute-btn" :disabled="!canStart" @click="handleStart">
            {{ $t('convert.startMaking') }}
          </button>
        </template>
        <template v-else>
          <button
            v-if="conversionProgress.current < conversionProgress.total"
            class="cancel-btn"
            @click="handleFinish"
          >
            {{ $t('convert.cancelConversion') }}
          </button>
          <button
            class="execute-btn"
            :class="{ done: conversionProgress.current >= conversionProgress.total && conversionProgress.total > 0 }"
            :disabled="conversionProgress.current < conversionProgress.total || conversionProgress.total === 0"
            @click="handleFinish"
          >
            {{ $t('convert.finishConversion') }}
          </button>
        </template>
      </div>
    </aside>
  </Teleport>

  <!-- 框选矩形覆盖层 -->
  <Teleport to="body">
    <div
      v-if="isSelecting && selectionRect"
      class="rubber-band-overlay"
      :style="{
        left: selectionRect.left + 'px',
        top: selectionRect.top + 'px',
        width: (selectionRect.right - selectionRect.left) + 'px',
        height: (selectionRect.bottom - selectionRect.top) + 'px',
      }"
    />
  </Teleport>

  <PageGuideOverlay :show="showGuide" :annotations="PAGE_GUIDE_ANNOTATIONS.convert" @close="showGuide = false" />
</template>

<style scoped>
/* 素材区：撑满 main-content */
.card-area {
  height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--spacing-6);
}

.hint-text {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  padding: var(--spacing-8);
  text-align: center;
}

.section-label {
  font-size: var(--text-base);
  font-weight: var(--font-bold);
  color: var(--text-secondary);
  margin-bottom: var(--spacing-3);
  padding-bottom: var(--spacing-2);
  border-bottom: 1px solid var(--border-medium);
}

.material-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: var(--spacing-3);
}

/* .mini-card 公共类已提取到 design-system.css */
</style>

<!-- Teleport 出去的面板用非 scoped style -->
<style>
/* Convert 独有样式（公共面板样式已提取到 design-system.css .control-panel）*/

/* Convert panel-body 覆盖：可滚动 */
.convert-control-panel .panel-body {
  flex: 1;
  overflow-y: auto;
}

/* Convert panel-footer 覆盖：不收缩 */
.convert-control-panel .panel-footer {
  flex-shrink: 0;
}

/* Convert custom-input-wrapper 覆盖：撑满宽度 */
.convert-control-panel .custom-input-wrapper {
  width: 100%;
}

/* Convert custom-input 覆盖：右侧 padding 更大（suffix 文字更长） */
.convert-control-panel .custom-input {
  padding: 0 32px 0 var(--spacing-3);
}

.convert-control-panel .stats {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.convert-control-panel .stat-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.convert-control-panel .stat-label {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.convert-control-panel .stat-value {
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  color: var(--text-primary);
}

.convert-control-panel .ghost-btn {
  background: transparent;
  border: 1px solid var(--border-medium);
  color: var(--color-primary-500);
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  cursor: pointer;
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-md);
  transition: all var(--duration-fast);
  width: 100%;
  font-family: inherit;
}

.convert-control-panel .ghost-btn:hover {
  background: var(--bg-hover);
}

.convert-control-panel .divider {
  height: 1px;
  background: var(--border-light);
  margin: var(--spacing-1) 0;
}

.convert-control-panel .panel-subtitle {
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  color: var(--text-secondary);
}

.convert-control-panel .fps-annotate-row {
  display: flex;
  align-items: center;
}

.convert-control-panel .seq-stat {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.convert-control-panel .progress-section {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.convert-control-panel .progress-count {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-primary);
  text-align: center;
}

.convert-control-panel .progress-track {
  height: 8px;
  background: var(--border-heavy);
  border-radius: var(--radius-full);
  overflow: hidden;
}

.convert-control-panel .progress-fill {
  height: 100%;
  background: var(--color-primary-500);
  border-radius: var(--radius-full);
  transition: width var(--duration-normal);
}

.convert-control-panel .progress-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  line-height: 1.4;
}

.convert-control-panel .failed-list {
  padding: var(--spacing-2) var(--spacing-3);
  background: var(--color-danger-light);
  border-radius: var(--radius-md);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.convert-control-panel .failed-title {
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  color: var(--color-danger-dark, #b91c1c);
}

.convert-control-panel .failed-item {
  font-size: var(--text-xs);
  color: var(--color-danger-dark, #b91c1c);
  word-break: break-all;
}

.convert-control-panel .execute-btn.done {
  background: var(--color-success);
}

.convert-control-panel .execute-btn.done:hover {
  background: var(--color-success-dark);
}

/* TP 预设折叠面板 */
.convert-control-panel .tp-preset-section {
  display: flex;
  flex-direction: column;
}

.convert-control-panel .tp-preset-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  cursor: pointer;
  padding: 0;
  font-family: inherit;
}

.convert-control-panel .tp-preset-toggle:hover {
  color: var(--text-primary);
}

.convert-control-panel .tp-preset-arrow {
  transition: transform var(--duration-fast);
  color: var(--text-tertiary);
}

.convert-control-panel .tp-preset-arrow.open {
  transform: rotate(180deg);
}

.convert-control-panel .tp-preset-body {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
  margin-top: var(--spacing-3);
}

.convert-control-panel .tp-preset-row {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.convert-control-panel .tp-preset-label {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}
</style>
