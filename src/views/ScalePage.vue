<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRubberBandSelect } from '../composables/useRubberBandSelect'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import { useNavigation } from '../composables/useNavigation'
import { useMaterials } from '../composables/useMaterials'
import type { MaterialInfo } from '../composables/useMaterials'
import MaterialCard from '../components/MaterialCard.vue'
import PageGuideOverlay from '../components/PageGuideOverlay.vue'
import { PAGE_GUIDE_ANNOTATIONS } from '../config/onboarding'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const { setNavigation } = useNavigation()
const { materials, loading, loadMaterials } = useMaterials()

const taskId = route.params.taskId as string
const showGuide = ref(false)

// taskPath 从路由 query 获取（TaskPage 跳转时传入）
const taskPath = route.query.taskPath as string

// 比例选择
const PRESET_SCALES = [100, 70, 50, 40]
const selectedScale = ref(70)
const customScale = ref('')

const finalScale = computed(() => {
  if (customScale.value && !isNaN(Number(customScale.value))) {
    return Math.min(100, Math.max(1, Number(customScale.value)))
  }
  return selectedScale.value
})

// 标注 Map：material.path → scale number
const scaleMap = ref<Map<string, number>>(new Map())

// 当前选中的卡片路径集合
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

// 执行状态
const executing = ref(false)
const error = ref<string | null>(null)
const scalingProgress = ref<{ current: number; total: number; name: string } | null>(null)

// 只展示静帧，且排除已上传、以及已有任意缩放版本的素材
const imageMaterials = computed(() =>
  materials.value.filter(m =>
    m.material_type === 'image' &&
    m.progress !== 'uploaded' &&
    m.scales.length === 0
  )
)

// 有标注的素材数量（用于按钮 disabled 判断）
const annotatedCount = computed(() => scaleMap.value.size)

onMounted(async () => {
  setNavigation({
    title: `${t('scale.title')} · ${taskId}`,
    showBackButton: true,
    onBack: () => router.back(),
    actions: [],
    moreMenuItems: [
      { id: 'page-guide', label: t('common.pageGuide'), handler: () => { showGuide.value = true } },
    ],
  })
  if (taskPath) {
    await loadMaterials(taskPath)
  }
})

function selectPreset(scale: number) {
  selectedScale.value = scale
  customScale.value = ''
}

function handleCustomInput(e: Event) {
  const val = (e.target as HTMLInputElement).value
  customScale.value = val.replace(/[^\d]/g, '')
  if (customScale.value) {
    selectedScale.value = 0
  }
}

function toggleCard(m: MaterialInfo) {
  const newSet = new Set(selectedPaths.value)
  if (newSet.has(m.path)) {
    newSet.delete(m.path)
  } else {
    newSet.add(m.path)
  }
  selectedPaths.value = newSet
}

// 应用比例到选中卡片（覆盖旧标注；若已有相同标注则清除）
function applyScale() {
  if (selectedPaths.value.size === 0) return
  const scale = finalScale.value
  if (scale <= 0) return

  const newMap = new Map(scaleMap.value)
  selectedPaths.value.forEach(path => {
    if (newMap.get(path) === scale) {
      // 同比例再次应用 → 清除
      newMap.delete(path)
    } else {
      // 覆盖标注
      newMap.set(path, scale)
    }
  })
  scaleMap.value = newMap
  // 应用后清空选中
  selectedPaths.value = new Set()
}

// 返回某素材的标注文字（用于 scaleLabel prop）
function scaleLabelFor(m: MaterialInfo): string | undefined {
  const s = scaleMap.value.get(m.path)
  return s !== undefined ? `${s}%` : undefined
}

async function handleExecute() {
  if (annotatedCount.value === 0 || !taskPath) return
  executing.value = true
  error.value = null
  scalingProgress.value = null

  let unlisten: UnlistenFn | null = null

  try {
    unlisten = await listen<{ current: number; total: number; name: string }>('scaling-progress', (event) => {
      scalingProgress.value = event.payload
    })

    const requests: { original_path: string; target_dir: string; scale_percent: number; base_name: string }[] = []

    scaleMap.value.forEach((scale, path) => {
      const m = imageMaterials.value.find(m => m.path === path)
      if (!m) return
      requests.push({
        original_path: m.path,
        target_dir: `${taskPath}\\01_scale\\[${scale}]`,
        scale_percent: scale,
        base_name: m.name,
      })
    })

    await invoke('execute_scaling', { requests })
    router.back()
  } catch (e) {
    error.value = String(e)
    console.error('执行缩放失败:', e)
  } finally {
    unlisten?.()
    executing.value = false
    scalingProgress.value = null
  }
}
</script>

<template>
  <!-- 素材卡片区（占满 main-content） -->
  <div
  ref="cardAreaRef"
  class="card-area"
  @mousedown="onContainerMouseDown"
  @scroll="onContainerScroll"
>
    <p v-if="loading" class="hint-text">{{ $t('common.scanning') }}</p>
    <p v-else-if="imageMaterials.length === 0" class="hint-text">{{ $t('scale.noImages') }}</p>
    <div v-else class="card-grid">
      <MaterialCard
        v-for="m in imageMaterials"
        :key="m.path"
        :material="m"
        :multi-select="true"
        :checked="selectedPaths.has(m.path)"
        :scale-label="scaleLabelFor(m)"
        @click="toggleCard(m)"
      />
    </div>
  </div>

  <!-- 控制面板：Teleport 到 #content-row，作为独立毛玻璃板块 -->
  <Teleport to="#content-row">
    <aside class="scale-control-panel">
      <div class="panel-body">
        <p class="panel-title">{{ $t('scale.scaleRatio') }}</p>

        <div class="scale-options">
          <button
            v-for="s in PRESET_SCALES"
            :key="s"
            class="scale-btn"
            :class="{ active: selectedScale === s && !customScale }"
            @click="selectPreset(s)"
          >
            {{ s }}%
          </button>
        </div>

        <div class="custom-row">
          <div class="custom-input-wrapper">
            <input
              type="text"
              class="custom-input"
              :placeholder="$t('scale.custom')"
              :value="customScale"
              @input="handleCustomInput"
            />
            <span class="input-suffix">%</span>
          </div>
        </div>

        <button
          class="apply-btn"
          :disabled="selectedPaths.size === 0 || finalScale <= 0"
          @click="applyScale"
        >
          {{ $t('scale.applyToSelected') }} ({{ selectedPaths.size }})
        </button>
      </div>

      <div class="panel-footer">
        <div v-if="error" class="error-msg">{{ error }}</div>
        <div v-if="executing && scalingProgress" class="scaling-progress">
          <div class="progress-text">
            {{ $t('scale.scalingProgress', { current: scalingProgress.current, total: scalingProgress.total }) }}
          </div>
          <div class="progress-filename">{{ scalingProgress.name }}</div>
          <div class="progress-bar-track">
            <div class="progress-bar-fill" :style="{ width: (scalingProgress.current / scalingProgress.total * 100) + '%' }" />
          </div>
        </div>
        <div v-else-if="executing" class="executing-hint">{{ $t('common.executing') }}</div>
        <div class="footer-actions">
          <button class="cancel-btn" :disabled="executing" @click="router.back()">{{ $t('common.cancel') }}</button>
          <button
            class="execute-btn"
            :disabled="annotatedCount === 0 || executing"
            @click="handleExecute"
          >
            {{ executing ? $t('common.executing') : `${$t('scale.startScale')} (${annotatedCount})` }}
          </button>
        </div>
      </div>
    </aside>
  </Teleport>

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

  <PageGuideOverlay :show="showGuide" :annotations="PAGE_GUIDE_ANNOTATIONS.scale" @close="showGuide = false" />
</template>

<style scoped>
/* 素材区：撑满 main-content */
.card-area {
  height: 100%;
  overflow-y: auto;
}

.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--card-material-width, 160px), 1fr));
  gap: var(--spacing-4);
  padding: 2px;
}

.hint-text {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  padding: var(--spacing-8);
  text-align: center;
}
</style>

<!-- Teleport 出去的面板用非 scoped style，否则 scoped hash 不会附加到 Teleport 目标 -->
<style>
/* 手动复刻 glass-medium 视觉，不用 backdrop-filter：
   Teleport 到 #content-row 后与 main-content(glass-medium) 成兄弟，
   双 backdrop-filter 在 WebView2 + Acrylic 下 gap 区域产生白色闪烁 */
.scale-control-panel {
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

.scale-control-panel .panel-body {
  padding: var(--spacing-5);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
}

.scale-control-panel .panel-title {
  font-size: var(--text-base);
  font-weight: var(--font-bold);
  color: var(--text-primary);
}

.scale-control-panel .scale-options {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.scale-control-panel .scale-btn {
  width: 100%;
  height: 36px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: var(--bg-tertiary);
  color: var(--text-primary);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: all var(--duration-fast);
  text-align: left;
  padding: 0 var(--spacing-3);
  font-family: inherit;
}

.scale-control-panel .scale-btn:hover {
  background: var(--bg-hover);
}

.scale-control-panel .scale-btn.active {
  background: var(--color-primary-500);
  border-color: var(--color-primary-500);
  color: white;
}

.scale-control-panel .custom-row {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.scale-control-panel .custom-input-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.scale-control-panel .custom-input {
  width: 100%;
  height: 36px;
  padding: 0 28px 0 var(--spacing-3);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: var(--bg-tertiary);
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-family: inherit;
}

.scale-control-panel .custom-input:focus {
  outline: none;
  border-color: var(--color-primary-500);
}

.scale-control-panel .input-suffix {
  position: absolute;
  right: var(--spacing-2);
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  pointer-events: none;
}

.scale-control-panel .apply-btn {
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

.scale-control-panel .apply-btn:hover:not(:disabled) {
  background: var(--color-primary-200);
}

.scale-control-panel .apply-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.scale-control-panel .panel-footer {
  padding: var(--spacing-4) var(--spacing-5);
  border-top: 1px solid var(--border-light);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.scale-control-panel .footer-actions {
  display: flex;
  gap: var(--spacing-2);
}

.scale-control-panel .cancel-btn {
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

.scale-control-panel .cancel-btn:hover:not(:disabled) {
  background: var(--bg-hover);
}

.scale-control-panel .execute-btn {
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

.scale-control-panel .execute-btn:hover:not(:disabled) {
  background: var(--color-primary-600);
}

.scale-control-panel .execute-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.scale-control-panel .error-msg {
  padding: var(--spacing-2) var(--spacing-3);
  background: var(--color-danger-light);
  color: var(--color-danger-dark);
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
}

.scale-control-panel .executing-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  text-align: center;
}

.scale-control-panel .scaling-progress {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.scale-control-panel .progress-text {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  font-weight: var(--font-medium);
}

.scale-control-panel .progress-filename {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.scale-control-panel .progress-bar-track {
  height: 4px;
  border-radius: 2px;
  background: var(--bg-tertiary);
  overflow: hidden;
}

.scale-control-panel .progress-bar-fill {
  height: 100%;
  border-radius: 2px;
  background: var(--color-primary-500);
  transition: width var(--duration-fast) var(--ease-out);
}
</style>
