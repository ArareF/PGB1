<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { MaterialInfo } from '../composables/useMaterials'
import MaterialCard from './MaterialCard.vue'

const props = defineProps<{
  taskPath: string
  materials: MaterialInfo[] // 传入 TaskPage 中过滤后的静帧
}>()

const emit = defineEmits<{
  close: []
  success: []
}>()

const PRESET_SCALES = [100, 70, 50, 40]
const selectedScale = ref(70)
const customScale = ref('')
const executing = ref(false)
const progress = ref({ current: 0, total: 0 })
const error = ref<string | null>(null)

// 选中的素材路径
const selectedPaths = ref<Set<string>>(new Set())

/** 仅处理静帧（虽然传入的应该已经是静帧，但保险起见） */
const imageMaterials = computed(() => {
  return props.materials.filter(m => m.material_type === 'image')
})

/** 获取最终使用的缩放比例 */
const finalScale = computed(() => {
  if (customScale.value && !isNaN(Number(customScale.value))) {
    return Math.min(100, Math.max(1, Number(customScale.value)))
  }
  return selectedScale.value
})

/** 选中的素材列表 */
const finalSelectedMaterials = computed(() => {
  return imageMaterials.value.filter(m => selectedPaths.value.has(m.path))
})

onMounted(() => {
  // 默认全选传入的所有素材
  const all = new Set<string>()
  imageMaterials.value.forEach(m => all.add(m.path))
  selectedPaths.value = all
})

function toggleSelectAll() {
  if (selectedPaths.value.size === imageMaterials.value.length) {
    selectedPaths.value = new Set()
  } else {
    const all = new Set<string>()
    imageMaterials.value.forEach(m => all.add(m.path))
    selectedPaths.value = all
  }
}

function toggleItem(material: MaterialInfo) {
  const newSet = new Set(selectedPaths.value)
  if (newSet.has(material.path)) {
    newSet.delete(material.path)
  } else {
    newSet.add(material.path)
  }
  selectedPaths.value = newSet
}

/** 执行缩放 */
async function handleExecute() {
  if (finalSelectedMaterials.value.length === 0) return

  executing.value = true
  error.value = null
  progress.value = { current: 0, total: finalSelectedMaterials.value.length }

  try {
    const scale = finalScale.value
    const requests = finalSelectedMaterials.value.map(m => ({
      original_path: m.path,
      target_dir: `${props.taskPath}\\01_scale\\[${scale}]`,
      scale_percent: scale,
      base_name: m.name,
    }))

    await invoke('execute_scaling', { requests })
    
    emit('success')
    emit('close')
  } catch (e) {
    error.value = String(e)
    console.error('执行缩放失败:', e)
  } finally {
    executing.value = false
  }
}

function handleCustomInput(e: Event) {
  const val = (e.target as HTMLInputElement).value
  customScale.value = val.replace(/[^\d]/g, '')
  if (customScale.value) {
    selectedScale.value = 0
  }
}

function selectPreset(scale: number) {
  selectedScale.value = scale
  customScale.value = ''
}
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog">
    <div class="dialog-overlay" @click.self="!executing && emit('close')">
      <div class="dialog-content glass-strong">
        <div class="dialog-header">
          <p class="dialog-title">批量缩放素材</p>
          <p class="dialog-subtitle">为选中的静帧素材生成指定比例的源文件</p>
        </div>

        <div class="dialog-body">
          <!-- 比例选择区 -->
          <div class="scale-selector-section">
            <p class="section-label">选择缩放比例</p>
            <div class="scale-options">
              <button
                v-for="s in PRESET_SCALES"
                :key="s"
                class="scale-btn"
                :class="{ active: selectedScale === s }"
                @click="selectPreset(s)"
              >
                {{ s }}%
              </button>
              <div class="custom-scale-input-wrapper">
                <input
                  type="text"
                  class="custom-input"
                  placeholder="自定义"
                  :value="customScale"
                  @input="handleCustomInput"
                />
                <span class="input-suffix">%</span>
              </div>
            </div>
          </div>

          <!-- 素材列表概览 -->
          <div class="material-overview-section">
            <div class="section-header">
              <p class="section-label">待处理素材 ({{ finalSelectedMaterials.length }}/{{ imageMaterials.length }})</p>
              <button class="ghost-btn" @click="toggleSelectAll">
                {{ selectedPaths.size === imageMaterials.length ? '取消全选' : '全选' }}
              </button>
            </div>
            
            <div class="material-grid custom-scroll">
              <MaterialCard
                v-for="m in imageMaterials"
                :key="m.path"
                :material="m"
                :multi-select="true"
                :checked="selectedPaths.has(m.path)"
                class="mini-card"
                @click="toggleItem(m)"
              />
              
              <div v-if="imageMaterials.length === 0" class="empty-hint">
                未选中任何静帧素材
              </div>
            </div>
          </div>

          <!-- 进度/错误展示 -->
          <div v-if="executing || error" class="status-section">
            <div v-if="executing" class="progress-bar-container">
              <p class="progress-text">处理中... 请稍候</p>
              <div class="progress-track">
                <div class="progress-fill infinite-animation"></div>
              </div>
            </div>
            <div v-if="error" class="error-msg">
              {{ error }}
            </div>
          </div>
        </div>

        <div class="dialog-actions">
          <button class="dialog-btn secondary" :disabled="executing" @click="emit('close')">
            取消
          </button>
          <button
            class="dialog-btn primary"
            :disabled="executing || finalSelectedMaterials.length === 0 || finalScale <= 0"
            @click="handleExecute"
          >
            {{ executing ? '执行中...' : '开始缩放' }}
          </button>
        </div>
      </div>
    </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
/* ... (保留原有样式) ... */
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
  width: 520px; /* 稍微加宽以容纳卡片 */
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
  overflow-y: auto;
  padding: 0 var(--spacing-6) var(--spacing-4);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-6);
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-3);
}

.section-label {
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  color: var(--text-secondary);
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

/* 比例选择 */
.scale-options {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-2);
}

.scale-btn {
  height: 36px;
  padding: 0 var(--spacing-4);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: var(--bg-tertiary);
  color: var(--text-primary);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.scale-btn:hover {
  background: var(--bg-hover);
}

.scale-btn.active {
  background: var(--color-primary-500);
  border-color: var(--color-primary-500);
  color: white;
}

.custom-scale-input-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.custom-input {
  width: 100px;
  height: 36px;
  padding: 0 24px 0 var(--spacing-3);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: var(--bg-tertiary);
  color: var(--text-primary);
  font-size: var(--text-sm);
}

.custom-input:focus {
  outline: none;
  border-color: var(--color-primary-500);
}

.input-suffix {
  position: absolute;
  right: var(--spacing-2);
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  pointer-events: none;
}

/* 素材卡片网格 - 使用复用的 MaterialCard */
.material-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: var(--spacing-3);
  padding: 2px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-light);
  border-radius: var(--radius-lg);
  padding: var(--spacing-3);
  max-height: 320px;
  overflow-y: auto;
}

/* 覆盖 MaterialCard 的样式以适应弹窗 */
.mini-card {
  --card-material-width: 100% !important; /* 强制填满网格单元 */
  --card-material-padding: var(--spacing-2) !important;
  --card-material-gap: var(--spacing-2) !important;
}

/* 深度选择器覆盖内部文字大小 */
.mini-card :deep(.card-name) {
  font-size: var(--text-xs) !important;
}
.mini-card :deep(.progress-tag) {
  height: 20px !important;
  font-size: var(--text-2xs) !important;
  padding: 0 6px !important;
}
.mini-card :deep(.size-tag) {
  font-size: var(--text-2xs) !important;
}

.empty-hint {
  padding: var(--spacing-8);
  text-align: center;
  color: var(--text-tertiary);
  grid-column: 1 / -1;
}

/* 状态展示 */
.progress-bar-container {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.progress-text {
  font-size: var(--text-xs);
  color: var(--text-secondary);
}

.progress-track {
  height: 6px;
  background: var(--border-medium);
  border-radius: var(--radius-full);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--color-primary-500);
  width: 40%;
  border-radius: var(--radius-full);
}

.infinite-animation {
  animation: progress-slide 1.5s infinite linear;
  width: 30%;
}

@keyframes progress-slide {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(333%); }
}

.error-msg {
  padding: var(--spacing-3);
  background: var(--color-danger-light);
  color: var(--color-danger-dark);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
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
  padding: 0 var(--spacing-6);
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

.dialog-btn.secondary:hover:not(:disabled) {
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
