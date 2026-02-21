<script setup lang="ts">
import { computed } from 'vue'
import type { TaskInfo } from '../composables/useTasks'

interface SubtaskProgress {
  completed: number
  total: number
}

const props = defineProps<{
  task: TaskInfo
  subtaskProgress?: SubtaskProgress
}>()

defineEmits<{
  click: [task: TaskInfo]
}>()

/** 进度状态：未开始 / 制作中 / 进行中 / 已完成 */
const statusInfo = computed(() => {
  const p = props.subtaskProgress
  if (p && p.total > 0) {
    // 有子任务：用子任务进度（无法感知本地制作状态，0/N 视为未开始）
    if (p.completed >= p.total) {
      return { label: '已完成', cls: 'status-completed' }
    }
    if (p.completed > 0) {
      return { label: `进行中 ${p.completed}/${p.total}`, cls: 'status-progress' }
    }
    return { label: `未开始 0/${p.total}`, cls: 'status-pending' }
  }
  // 无子任务：3态，不显示数字（分母只有子任务才有意义）
  const { material_total: total, material_uploaded: uploaded, video_total: vTotal, video_uploaded: vUploaded } = props.task
  const allMaterialsUploaded = total > 0 && uploaded >= total
  const allVideosUploaded = vTotal === 0 || vUploaded >= vTotal
  if (allMaterialsUploaded && allVideosUploaded) {
    return { label: '已完成', cls: 'status-completed' }
  }
  if (total > 0) {
    return { label: '制作中', cls: 'status-wip' }
  }
  return { label: '未开始', cls: 'status-pending' }
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
  <button
    class="task-card glass-subtle"
    @click="$emit('click', task)"
  >
    <span class="task-name">{{ task.name }}</span>

    <div class="task-bottom">
      <span class="status-tag" :class="statusInfo.cls">{{ statusInfo.label }}</span>
      <span class="task-size">{{ formatSize(task.size_bytes) }}</span>
    </div>
  </button>
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
  border: none;
  cursor: pointer;
  transition: var(--transition-card-hover);
  text-align: left;
}

.task-card:hover {
  transform: translateY(var(--card-hover-lift));
  box-shadow: var(--card-shadow-hover);
}

.task-name {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

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

.status-pending {
  background: var(--tag-status-pending-bg);
}

.status-wip {
  background: var(--tag-status-wip-bg);
}

.status-progress {
  background: var(--tag-status-progress-bg);
}

.status-completed {
  background: var(--tag-status-completed-bg);
}

.task-size {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}
</style>
