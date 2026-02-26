<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import type { MaterialInfo } from '../composables/useMaterials'
import MaterialCard from './MaterialCard.vue'

const props = defineProps<{
  taskPath: string
  materials: MaterialInfo[]
}>()

useI18n()

const emit = defineEmits<{
  close: []
  start: [data: { images: Record<string, number>, sequences: { name: string, fps: number }[] }]
}>()

/** 序列帧帧率映射表 */
const fpsMap = ref<Record<string, string>>({})

/** 选中的素材路径 */
const selectedPaths = ref<Set<string>>(new Set())

/** 过滤出未转换的静帧 */
const pendingImages = computed(() => {
  return props.materials.filter(m => m.material_type === 'image' && m.progress !== 'done' && m.progress !== 'uploaded')
})

/** 过滤出未转换的序列帧 */
const pendingSequences = computed(() => {
  return props.materials.filter(m => m.material_type === 'sequence' && m.progress !== 'done' && m.progress !== 'uploaded')
})

function toggleSelectAll() {
  const allCount = pendingImages.value.length + pendingSequences.value.length
  if (selectedPaths.value.size === allCount) {
    selectedPaths.value = new Set()
  } else {
    const all = new Set<string>()
    pendingImages.value.forEach(m => all.add(m.path))
    pendingSequences.value.forEach(m => all.add(m.path))
    selectedPaths.value = all
  }
}

function toggleItem(path: string) {
  const newSet = new Set(selectedPaths.value)
  if (newSet.has(path)) {
    newSet.delete(path)
  } else {
    newSet.add(path)
  }
  selectedPaths.value = newSet
}

/** 校验是否可以开始转换 */
const canStart = computed(() => {
  if (selectedPaths.value.size === 0) return false
  
  // 检查所有选中的序列帧是否都填了有效的帧率 (1-120)
  for (const seq of pendingSequences.value) {
    if (selectedPaths.value.has(seq.path)) {
      const fps = Number(fpsMap.value[seq.path])
      if (isNaN(fps) || fps < 1 || fps > 120) return false
    }
  }
  
  return true
})

/** 提交转换请求 */
async function handleStart() {
  if (!canStart.value) return

  const images: Record<string, number> = {}
  const sequences: { name: string, fps: number }[] = []

  // 1. 处理静帧
  for (const img of pendingImages.value) {
    if (selectedPaths.value.has(img.path)) {
      images[img.name] = 0 // 比例由后端识别 01_scale 确定
    }
  }

  // 2. 处理序列帧
  for (const seq of pendingSequences.value) {
    if (selectedPaths.value.has(seq.path)) {
      sequences.push({
        name: seq.name,
        fps: Number(fpsMap.value[seq.path])
      })
    }
  }

  emit('start', { images, sequences })
}

onMounted(() => {
  // 默认全选
  const all = new Set<string>()
  pendingImages.value.forEach(m => all.add(m.path))
  pendingSequences.value.forEach(m => all.add(m.path))
  selectedPaths.value = all
})
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog">
    <div class="dialog-overlay">
      <div class="dialog-content glass-strong">
        <div class="dialog-header">
          <p class="dialog-title">{{ $t('convert.selectTitle') }}</p>
          <p class="dialog-subtitle">{{ $t('convert.selectDesc') }}</p>
        </div>

        <div class="dialog-body">
          <div class="selection-controls">
            <button class="ghost-btn" @click="toggleSelectAll">
              {{ selectedPaths.size === (pendingImages.length + pendingSequences.length) ? $t('common.deselectAll') : $t('common.selectAll') }}
            </button>
          </div>

          <div class="scroll-area custom-scroll">
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
                <div
                  v-for="m in pendingSequences"
                  :key="m.path"
                  class="seq-item-container"
                >
                  <MaterialCard
                    :material="m"
                    :multi-select="true"
                    :checked="selectedPaths.has(m.path)"
                    class="mini-card"
                    @click="toggleItem(m.path)"
                  />
                  
                  <div class="fps-control">
                    <span class="fps-label">FPS:</span>
                    <input
                      v-model="fpsMap[m.path]"
                      type="text"
                      class="fps-input"
                      placeholder="24"
                      maxlength="3"
                      :disabled="!selectedPaths.has(m.path)"
                    />
                  </div>
                </div>
              </div>
            </div>

            <div v-if="pendingImages.length === 0 && pendingSequences.length === 0" class="empty-state">
              {{ $t('convert.allConverted') }}
            </div>
          </div>
        </div>

        <div class="dialog-actions">
          <button class="dialog-btn secondary" @click="emit('close')">
            {{ $t('common.cancel') }}
          </button>
          <button
            class="dialog-btn primary"
            :disabled="!canStart"
            @click="handleStart"
          >
            {{ $t('convert.startMaking') }}
          </button>
        </div>
      </div>
    </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-backdrop);
  backdrop-filter: blur(var(--glass-light-blur));
}

.dialog-content {
  width: 560px;
  max-height: 85vh;
  border-radius: var(--radius-2xl);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: var(--shadow-modal);
}

.dialog-header {
  padding: var(--spacing-6) var(--spacing-6) var(--spacing-4);
  flex-shrink: 0;
}

.dialog-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-bold);
  color: var(--text-primary);
  margin-bottom: var(--spacing-1);
}

.dialog-subtitle {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.dialog-body {
  flex: 1;
  overflow: hidden;
  padding: 0 var(--spacing-6);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
}

.selection-controls {
  display: flex;
  justify-content: flex-end;
  padding-bottom: var(--spacing-2);
}

.ghost-btn {
  background: transparent;
  border: none;
  color: var(--color-primary-500);
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  cursor: pointer;
  padding: 4px 8px;
  border-radius: var(--radius-sm);
}

.ghost-btn:hover {
  background: var(--bg-hover);
}

.scroll-area {
  flex: 1;
  overflow-y: auto;
  padding-bottom: var(--spacing-6);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-6);
}

.section-label {
  font-size: var(--text-base);
  font-weight: var(--font-bold);
  color: var(--text-secondary);
  margin-bottom: var(--spacing-3);
  padding-bottom: var(--spacing-2);
  border-bottom: 1px solid var(--border-medium);
}

/* 统一网格布局 */
.material-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: var(--spacing-3);
}

/* 序列帧布局容器 */
.seq-item-container {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

/* FPS 控制条 */
.fps-control {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-2);
  padding-top: 2px;
}

.fps-label {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  font-weight: var(--font-bold);
}

.fps-input {
  width: 40px;
  height: 24px;
  text-align: center;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-heavy);
  background: var(--bg-tertiary); /* 改为主题跟随色 */
  color: var(--text-primary);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  transition: all var(--duration-fast);
}

.fps-input:focus {
  outline: none;
  border-color: var(--color-primary-500);
  background: var(--bg-primary); /* 聚焦时高亮 */
}

.fps-input:disabled {
  opacity: 0.5;
  background: var(--bg-app);
  color: var(--text-tertiary);
}

/* 复用卡片样式调整 */
.mini-card {
  --card-material-width: 100% !important;
  --card-material-padding: var(--spacing-2) !important;
  --card-material-gap: var(--spacing-2) !important;
}



/* 深度选择器覆盖内部样式 */
.mini-card :deep(.card-name) {
  font-size: var(--text-xs) !important;
}
.mini-card :deep(.progress-tag) {
  height: 18px !important;
  font-size: var(--text-2xs) !important;
  padding: 0 4px !important;
}
.mini-card :deep(.size-tag) {
  font-size: var(--text-2xs) !important;
}
.mini-card :deep(.checkbox) {
  width: 16px !important;
  height: 16px !important;
}

.empty-state {
  padding: var(--spacing-12);
  text-align: center;
  color: var(--text-tertiary);
}

.dialog-actions {
  padding: var(--spacing-6);
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-3);
  flex-shrink: 0;
}

.dialog-btn {
  height: var(--button-md-height);
  padding: 0 var(--spacing-8);
  border-radius: var(--radius-lg);
  font-weight: var(--font-bold);
  cursor: pointer;
  transition: all var(--duration-fast);
  border: none;
}

.dialog-btn.primary {
  background: var(--color-primary-500);
  color: white;
}

.dialog-btn.primary:hover:not(:disabled) {
  background: var(--color-primary-600);
}

.dialog-btn.secondary {
  background: transparent;
  color: var(--text-secondary);
  border: 1px solid var(--border-medium);
}

.dialog-btn.secondary:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.dialog-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>

<style>
/* 弹窗进出动画 */
.dialog-enter-active {
  transition: opacity var(--duration-dialog) var(--ease-out);
}
.dialog-leave-active {
  transition: opacity var(--duration-dialog) var(--ease-in);
}
.dialog-enter-from,
.dialog-leave-to {
  opacity: 0;
}
.dialog-enter-active .dialog-content {
  transition: transform var(--duration-dialog) var(--ease-out);
}
.dialog-leave-active .dialog-content {
  transition: transform var(--duration-dialog) var(--ease-in);
}
.dialog-enter-from .dialog-content {
  transform: translateY(16px) scale(0.97);
}
.dialog-leave-to .dialog-content {
  transform: translateY(8px) scale(0.97);
}
</style>
