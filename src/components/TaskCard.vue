<script setup lang="ts">
import { computed, ref, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import type { TaskInfo } from '../composables/useTasks'

const { t } = useI18n()

interface SubtaskProgress {
  completed: number
  total: number
}

const props = defineProps<{
  task: TaskInfo
  subtaskProgress?: SubtaskProgress
}>()

const emit = defineEmits<{
  click: [task: TaskInfo]
  action: [task: TaskInfo, action: 'priority', value: string | null]
}>()

// 菜单控制
const showMenu = ref(false)
const menuBtnRef = ref<HTMLElement | null>(null)
const menuStyle = ref({ top: '0px', right: '0px' })

async function toggleMenu() {
  showMenu.value = !showMenu.value
  if (showMenu.value) {
    await nextTick()
    if (menuBtnRef.value) {
      const rect = menuBtnRef.value.getBoundingClientRect()
      menuStyle.value = {
        top: `${rect.bottom + 4}px`,
        right: `${window.innerWidth - rect.right}px`,
      }
    }
  }
}

function setPriority(option: string) {
  showMenu.value = false
  const value = option === 'normal' ? null : option
  emit('action', props.task, 'priority', value)
}

/** 文件上传是否全部完成（素材 + 预览视频） */
function filesAllUploaded(): boolean {
  const { material_total: mTotal, material_uploaded: mUploaded, video_total: vTotal, video_uploaded: vUploaded } = props.task
  const materialsOk = mTotal === 0 || mUploaded >= mTotal
  const videosOk = vTotal === 0 || vUploaded >= vTotal
  return materialsOk && videosOk
}

/** 进度状态：未开始 / 制作中 / 已完成 */
const statusInfo = computed(() => {
  const p = props.subtaskProgress
  if (p && p.total > 0) {
    if (p.completed >= p.total && filesAllUploaded()) {
      return { label: t('taskCard.completed'), cls: 'status-completed' }
    }
    if (p.completed > 0) {
      return { label: `${t('taskCard.inProgress')} ${p.completed}/${p.total}`, cls: 'status-wip' }
    }
    return { label: `${t('taskCard.notStarted')} 0/${p.total}`, cls: 'status-pending' }
  }
  const { material_total: total, material_uploaded: uploaded } = props.task
  if (total > 0 && uploaded >= total && filesAllUploaded()) {
    return { label: t('taskCard.completed'), cls: 'status-completed' }
  }
  if (total > 0) {
    return { label: t('taskCard.inProgress'), cls: 'status-wip' }
  }
  return { label: t('taskCard.notStarted'), cls: 'status-pending' }
})

/** 文件大小格式化 */
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}
</script>

<template>
  <div
    class="task-card glass-subtle"
    role="button"
    tabindex="0"
    @click="$emit('click', task)"
    @keydown.enter="$emit('click', task)"
  >
    <!-- 优先度标签 + 名称 -->
    <div class="task-name-row">
      <span
        v-if="task.priority"
        class="priority-dot"
        :class="`priority-dot--${task.priority}`"
      />
      <span class="task-name">{{ task.name }}</span>
    </div>

    <div class="task-bottom">
      <span class="status-tag" :class="statusInfo.cls">{{ statusInfo.label }}</span>
      <span class="task-size">{{ formatSize(task.size_bytes) }}</span>
    </div>

    <!-- ··· 菜单按钮 -->
    <button
      ref="menuBtnRef"
      class="card-menu-btn"
      :class="{ visible: showMenu }"
      @click.stop="toggleMenu"
      @blur="showMenu = false"
    >
      ···
    </button>

    <!-- 下拉菜单 — Teleport to body，避免父级 backdrop-filter 干扰毛玻璃 -->
    <!-- Teleport 在 button 内部，使组件保持单根节点，兼容 TransitionGroup 动画 -->
    <Teleport to="body">
    <Transition name="card-menu">
      <div v-if="showMenu" class="card-menu glass-medium" :style="menuStyle" @click.stop>
        <!-- 优先度选择器 -->
        <div class="menu-priority-section">
          <span class="menu-priority-label">{{ $t('priority.setPriority') }}</span>
          <div class="menu-priority-pills">
            <button
              v-for="p in ['high', 'medium', 'normal', 'low']"
              :key="p"
              class="priority-pill"
              :class="[`priority-pill--${p}`, { 'is-active': p === 'normal' ? !task.priority : task.priority === p }]"
              @mousedown.prevent="setPriority(p)"
            >{{ $t(`priority.${p}`) }}</button>
          </div>
        </div>
      </div>
    </Transition>
    </Teleport>
  </div>
</template>

<style scoped>
.task-card {
  width: var(--card-task-width);
  height: var(--card-task-height);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: var(--card-task-padding);
  border-radius: var(--card-border-radius);
  cursor: pointer;
  user-select: none;
  outline: none;
  transition: var(--transition-card-hover);
  text-align: left;
  position: relative;
}

.task-card:hover {
  transform: translateY(var(--card-hover-lift));
  box-shadow: var(--card-shadow-hover);
}

/* 名称行 */
.task-name-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
  min-width: 0;
  overflow: hidden;
}

.task-name {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Priority 圆点（纯色小圆，无文字） */
.priority-dot {
  flex-shrink: 0;
  width: 9px;
  height: 9px;
  border-radius: 50%;
}

.priority-dot--high   { background: var(--priority-high-dot); }
.priority-dot--medium { background: var(--priority-medium-dot); }
.priority-dot--low    { background: var(--priority-low-dot); }

.task-bottom {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.status-tag {
  display: inline-flex;
  align-items: center;
  height: var(--tag-height);
  padding: 0 var(--tag-padding-x);
  font-size: var(--tag-font-size);
  font-weight: var(--tag-font-weight);
  border-radius: var(--tag-border-radius);
  color: var(--tag-status-text);
}

.status-pending  { background: var(--tag-status-pending-bg); }
.status-wip      { background: var(--tag-status-wip-bg); }
.status-completed{ background: var(--tag-status-completed-bg); }

.task-size {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

/* ··· 菜单按钮 */
.card-menu-btn {
  position: absolute;
  top: var(--spacing-2);
  right: var(--spacing-2);
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--text-xl);
  font-weight: bold;
  letter-spacing: 1px;
  color: var(--text-secondary);
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  opacity: 0;
  transform: scale(0.85);
  transition: opacity var(--duration-fast) var(--ease-out),
              transform var(--duration-fast) var(--ease-out),
              background var(--duration-fast) var(--ease-out);
  line-height: 1;
  padding-bottom: 4px;
}

.task-card:hover .card-menu-btn,
.card-menu-btn.visible {
  opacity: 1;
  transform: scale(1);
}

.card-menu-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}
</style>
