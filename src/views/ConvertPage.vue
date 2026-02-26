<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import { useNavigation } from '../composables/useNavigation'
import { useMaterials } from '../composables/useMaterials'
import { useSettings } from '../composables/useSettings'
import MaterialCard from '../components/MaterialCard.vue'
import { useRubberBandSelect } from '../composables/useRubberBandSelect'
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

const selectedPaths = ref<Set<string>>(new Set())
const cardAreaRef = ref<HTMLElement | null>(null)
const alwaysEnabled = ref(true)

const { isSelecting, selectionRect, onContainerMouseDown, onContainerScroll } =
  useRubberBandSelect({
    containerRef: cardAreaRef,
    cardSelector: '.material-card[data-path]',
    isEnabled: alwaysEnabled,
    onSelect: (paths) => {
      selectedPaths.value = paths
    },
  })

function toggleItem(path: string) {
  const newSet = new Set(selectedPaths.value)
  if (newSet.has(path)) {
    newSet.delete(path)
  } else {
    newSet.add(path)
  }
  selectedPaths.value = newSet
}

function toggleSelectAll() {
  // 全选/取消全选仅针对静帧（序列帧需手动标注FPS）
  const allImagesSelected = pendingImages.value.every(m => selectedPaths.value.has(m.path))
  const newSet = new Set(selectedPaths.value)
  if (allImagesSelected) {
    pendingImages.value.forEach(m => newSet.delete(m.path))
  } else {
    pendingImages.value.forEach(m => newSet.add(m.path))
  }
  selectedPaths.value = newSet
}

const selectedImageCount = computed(() =>
  pendingImages.value.filter(m => selectedPaths.value.has(m.path)).length
)

const annotatedSequenceCount = computed(() =>
  pendingSequences.value.filter(m => fpsMap.value.has(m.path)).length
)

// 当前选中的序列帧（用于判断应用按钮是否可用）
const selectedSequencePaths = computed(() =>
  pendingSequences.value.filter(m => selectedPaths.value.has(m.path))
)

// ─── FPS 标注（批次标注模式） ──────────────────────────

const fpsMap = ref<Map<string, number>>(new Map())
const fpsInput = ref('')

function fpsLabelFor(m: { path: string }): string | undefined {
  const fps = fpsMap.value.get(m.path)
  return fps !== undefined ? `${fps}fps` : undefined
}

function applyFps() {
  const fps = Number(fpsInput.value)
  if (isNaN(fps) || fps < 1 || fps > 120) return
  if (selectedSequencePaths.value.length === 0) return

  const newMap = new Map(fpsMap.value)
  selectedSequencePaths.value.forEach(m => {
    if (newMap.get(m.path) === fps) {
      newMap.delete(m.path)  // 同值再次应用 → 清除
    } else {
      newMap.set(m.path, fps)
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
    m => selectedPaths.value.has(m.path) && fpsMap.value.has(m.path)
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

async function handleStart() {
  if (!canStart.value || !taskPath) return

  const images: Record<string, number> = {}
  const sequences: { name: string; fps: number }[] = []

  for (const img of pendingImages.value) {
    if (selectedPaths.value.has(img.path)) {
      images[img.name] = 0
    }
  }
  fpsMap.value.forEach((fps, path) => {
    if (!selectedPaths.value.has(path)) return  // 未选中的跳过
    const seq = pendingSequences.value.find(m => m.path === path)
    if (seq) sequences.push({ name: seq.name, fps })
  })

  isConverting.value = true
  failedSequences.value = []
  sequenceError.value = ''
  conversionProgress.value = { current: 0, total: Object.keys(images).length + sequences.length }

  try {
    if (unlistenOrganized) unlistenOrganized()
    if (unlistenFailed) unlistenFailed()
    unlistenOrganized = await listen<string>('conversion-organized', () => {
      conversionProgress.value.current++
    })
    unlistenFailed = await listen<string>('sequence-conversion-failed', (event) => {
      failedSequences.value.push(event.payload)
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
  if (unlistenOrganized) unlistenOrganized()
  if (unlistenFailed) unlistenFailed()
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
  pendingImages.value.forEach(m => all.add(m.path))
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
            :key="m.path"
            :material="m"
            :multi-select="true"
            :checked="selectedPaths.has(m.path)"
            class="mini-card"
            @click="toggleItem(m.path)"
          />
        </div>
      </div>

      <!-- 序列帧分区 -->
      <div v-if="pendingSequences.length > 0" class="section">
        <p class="section-label">{{ $t('convert.sequenceSection') }} ({{ pendingSequences.length }})</p>
        <div class="material-grid">
          <MaterialCard
            v-for="m in pendingSequences"
            :key="m.path"
            :material="m"
            :multi-select="true"
            :checked="selectedPaths.has(m.path)"
            :scale-label="fpsLabelFor(m)"
            class="mini-card"
            @click="toggleItem(m.path)"
          />
        </div>
      </div>
    </template>
  </div>

  <!-- 控制面板：Teleport 到 #content-row，作为独立毛玻璃板块 -->
  <Teleport to="#content-row">
    <aside class="convert-control-panel">
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
          {{ pendingImages.every(m => selectedPaths.has(m.path)) ? $t('common.deselectAll') : $t('common.selectAll') }}
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
            class="execute-btn"
            :class="{ done: conversionProgress.current >= conversionProgress.total }"
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

.mini-card {
  --card-material-width: 100% !important;
  --card-material-padding: var(--spacing-2) !important;
  --card-material-gap: var(--spacing-2) !important;
}

.mini-card :deep(.card-name) { font-size: var(--text-xs) !important; }
.mini-card :deep(.progress-tag) { height: 18px !important; font-size: var(--text-2xs) !important; padding: 0 4px !important; }
.mini-card :deep(.size-tag) { font-size: var(--text-2xs) !important; }
.mini-card :deep(.checkbox) { width: 16px !important; height: 16px !important; }
</style>

<!-- Teleport 出去的面板用非 scoped style -->
<style>
/* 手动复刻 glass-medium 视觉，不用 backdrop-filter：
   Teleport 到 #content-row 后与 main-content(glass-medium) 成兄弟，
   双 backdrop-filter 在 WebView2 + Acrylic 下 gap 区域产生白色闪烁 */
.convert-control-panel {
  width: 220px;
  flex-shrink: 0;
  border-radius: var(--floating-main-radius);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  overflow: hidden;
  background: var(--glass-medium-bg);
  border: var(--glass-medium-border);
  box-shadow: var(--glass-medium-shadow);
}

.convert-control-panel .panel-body {
  padding: var(--spacing-5);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
  flex: 1;
  overflow-y: auto;
}

.convert-control-panel .panel-title {
  font-size: var(--text-base);
  font-weight: var(--font-bold);
  color: var(--text-primary);
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

.convert-control-panel .custom-input-wrapper {
  position: relative;
  display: flex;
  align-items: center;
  width: 100%;
}

.convert-control-panel .custom-input {
  width: 100%;
  height: 36px;
  padding: 0 32px 0 var(--spacing-3);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: var(--bg-tertiary);
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-family: inherit;
}

.convert-control-panel .custom-input:focus {
  outline: none;
  border-color: var(--color-primary-500);
}

.convert-control-panel .input-suffix {
  position: absolute;
  right: var(--spacing-2);
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  pointer-events: none;
}

.convert-control-panel .apply-btn {
  width: 100%;
  height: 36px;
  border-radius: var(--radius-md);
  border: none;
  background: var(--color-primary-100);
  color: var(--color-primary-600);
  font-weight: var(--font-bold);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--duration-fast);
  font-family: inherit;
}

.convert-control-panel .apply-btn:hover:not(:disabled) {
  background: var(--color-primary-200);
}

.convert-control-panel .apply-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
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

.convert-control-panel .panel-footer {
  padding: var(--spacing-4) var(--spacing-5);
  border-top: 1px solid var(--border-light);
  display: flex;
  gap: var(--spacing-2);
  flex-shrink: 0;
}

.convert-control-panel .cancel-btn {
  flex: 1;
  height: 36px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--duration-fast);
  font-family: inherit;
}

.convert-control-panel .cancel-btn:hover {
  background: var(--bg-hover);
}

.convert-control-panel .execute-btn {
  flex: 2;
  height: 36px;
  border-radius: var(--radius-md);
  border: none;
  background: var(--color-primary-500);
  color: white;
  font-weight: var(--font-bold);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--duration-fast);
  font-family: inherit;
}

.convert-control-panel .execute-btn:hover:not(:disabled) {
  background: var(--color-primary-600);
}

.convert-control-panel .execute-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.convert-control-panel .execute-btn.done {
  background: var(--color-success-500, #22c55e);
}

.convert-control-panel .execute-btn.done:hover {
  background: var(--color-success-600, #16a34a);
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
