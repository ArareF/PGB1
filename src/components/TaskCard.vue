<script setup lang="ts">
import { computed } from 'vue'
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

defineEmits<{
  click: [task: TaskInfo]
}>()

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
    // 有子任务：子任务进度 + 文件上传双重检查
    if (p.completed >= p.total && filesAllUploaded()) {
      return { label: t('taskCard.completed'), cls: 'status-completed' }
    }
    if (p.completed > 0) {
      return { label: `${t('taskCard.inProgress')} ${p.completed}/${p.total}`, cls: 'status-wip' }
    }
    return { label: `${t('taskCard.notStarted')} 0/${p.total}`, cls: 'status-pending' }
  }
  // 无子任务：3态
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

.status-completed {
  background: var(--tag-status-completed-bg);
}

.task-size {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}
</style>
