<script setup lang="ts">
import { computed, ref, onMounted, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { getPsdThumbnail } from '../composables/usePsdThumbnail'
import type { ProjectInfo } from '../composables/useProjects'

const props = defineProps<{
  project: ProjectInfo
}>()

const showMenu = ref(false)

defineEmits<{
  click: [project: ProjectInfo]
  action: [project: ProjectInfo, action: 'rename' | 'deadline' | 'delete']
}>()

// 分母：无子任务的父任务数 + 所有子任务数（有子任务的父任务本身不计入）
const totalTaskCount = computed(() => {
  const enabled = props.project.enabled_tasks
  const parentTasks = enabled.filter(t => !t.includes('/'))
  let total = 0
  for (const parent of parentTasks) {
    const children = enabled.filter(t => t.startsWith(parent + '/'))
    total += children.length === 0 ? 1 : children.length
  }
  return total
})

// 已完成数：无子任务的父任务用 completed_tasks，有子任务的父任务用 completed_subtasks
const completedTaskCount = computed(() => {
  const enabled = props.project.enabled_tasks
  const completedSubs = new Set(props.project.completed_subtasks)
  const completedTasks = new Set(props.project.completed_tasks)
  const parentTasks = enabled.filter(t => !t.includes('/'))
  let done = 0
  for (const parent of parentTasks) {
    const children = enabled.filter(t => t.startsWith(parent + '/'))
    if (children.length === 0) {
      if (completedTasks.has(parent)) done++
    } else {
      done += children.filter(c => completedSubs.has(c)).length
    }
  }
  return done
})

const progressPercent = computed(() => {
  const total = totalTaskCount.value
  if (total === 0) return 0
  return Math.round(completedTaskCount.value / total * 100)
})

// AppIcon 渲染逻辑
const iconSrc = ref<string | null>(null)

async function loadIcon() {
  const iconPath = props.project.app_icon
  if (!iconPath) {
    iconSrc.value = null
    return
  }
  const ext = iconPath.split('.').pop()?.toLowerCase() ?? ''
  if (ext === 'png') {
    iconSrc.value = convertFileSrc(iconPath)
  } else if (ext === 'psd' || ext === 'psb') {
    iconSrc.value = await getPsdThumbnail(iconPath, 128)
  } else {
    iconSrc.value = null
  }
}

onMounted(loadIcon)
watch(() => props.project.app_icon, loadIcon)
</script>

<template>
  <div
    class="project-card glass-subtle"
    @click="$emit('click', project)"
  >
    <!-- 左侧 ICON -->
    <div class="card-icon" :class="{ 'card-icon--has-image': iconSrc }">
      <img
        v-if="iconSrc"
        :src="iconSrc"
        class="card-icon-img"
        alt=""
      />
      <svg
        v-else
        width="32"
        height="32"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
      </svg>
    </div>

    <!-- 右侧信息 -->
    <div class="card-info">
      <span class="card-name">{{ project.name }}</span>
      <span class="card-deadline">
        {{ project.deadline ?? '未设置截止日期' }}
      </span>
    </div>

    <!-- 底部进度条 -->
    <div class="card-progress">
      <div class="progress-bar">
        <div
          class="progress-fill"
          :class="{ 'is-complete': progressPercent >= 100 }"
          :style="{ width: progressPercent + '%' }"
        />
      </div>
      <span class="progress-text">{{ completedTaskCount }} / {{ totalTaskCount }} 个任务</span>
    </div>

    <!-- ··· 菜单按钮（hover 时显示） -->
    <button
      class="card-menu-btn"
      :class="{ visible: showMenu }"
      @click.stop="showMenu = !showMenu"
      @blur="showMenu = false"
    >
      ···
    </button>

    <!-- 下拉菜单 -->
    <Transition name="card-menu">
      <div v-if="showMenu" class="card-menu" @click.stop>
        <button class="menu-item" @mousedown.prevent="$emit('action', project, 'rename')">
          重命名
        </button>
        <button class="menu-item" @mousedown.prevent="$emit('action', project, 'deadline')">
          修改截止日期
        </button>
        <button class="menu-item menu-item--danger" @mousedown.prevent="$emit('action', project, 'delete')">
          删除项目
        </button>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.project-card {
  width: var(--card-project-width);
  height: var(--card-project-height);
  display: grid;
  grid-template-columns: auto 1fr;
  grid-template-rows: 1fr auto;
  gap: 0 var(--card-project-gap);
  padding: var(--card-project-padding);
  align-items: center;
  border-radius: var(--card-border-radius);
  border: none;
  cursor: pointer;
  transition: var(--transition-card-hover);
  text-align: left;
}

.project-card:hover {
  transform: translateY(var(--card-hover-lift));
  box-shadow: var(--card-shadow-hover);
}

.card-icon {
  grid-row: 1;
  grid-column: 1;
  width: var(--card-project-icon-size);
  height: var(--card-project-icon-size);
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-lg);
  background: var(--bg-hover);
  color: var(--text-secondary);
  overflow: hidden;
  position: relative;
}

.card-icon-img {
  width: 110%;
  height: 110%;
  object-fit: cover;
  border-radius: var(--radius-lg);
}

.card-icon--has-image::after {
  content: '';
  position: absolute;
  inset: 0;
  border-radius: var(--radius-lg);
  box-shadow: inset 0 0 0 3px var(--color-neutral-0);
  pointer-events: none;
}

.card-info {
  grid-row: 1;
  grid-column: 2;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: var(--spacing-1);
  min-width: 0;
}

.card-name {
  font-size: var(--text-xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.card-deadline {
  font-size: var(--text-base);
  color: var(--text-tertiary);
}

.card-progress {
  grid-row: 2;
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  margin-top: var(--spacing-3);
}

.progress-bar {
  flex: 1;
  height: 4px;
  background: var(--bg-hover);
  border-radius: var(--radius-full);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--color-primary-500);
  border-radius: var(--radius-full);
  transition: width var(--duration-normal) var(--ease-out),
              background-color var(--duration-normal) var(--ease-out);
}

.progress-fill.is-complete {
  background: var(--color-success);
}

.progress-text {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  white-space: nowrap;
}

/* ··· 菜单 */
.project-card {
  position: relative;
}

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

.project-card:hover .card-menu-btn,
.card-menu-btn.visible {
  opacity: 1;
  transform: scale(1);
}

.card-menu-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.card-menu {
  position: absolute;
  top: calc(var(--spacing-2) + 28px + 4px);
  right: var(--spacing-2);
  min-width: 140px;
  background: var(--glass-medium-bg);
  backdrop-filter: blur(var(--glass-medium-blur));
  -webkit-backdrop-filter: blur(var(--glass-medium-blur));
  border: var(--glass-medium-border);
  border-radius: var(--radius-md);
  box-shadow: var(--glass-medium-shadow);
  overflow: hidden;
  z-index: 10;
}

.menu-item {
  display: block;
  width: 100%;
  padding: var(--spacing-2) var(--spacing-4);
  font-size: var(--text-base);
  color: var(--text-primary);
  background: transparent;
  border: none;
  text-align: left;
  cursor: pointer;
  transition: background var(--transition-fast);
}

.menu-item:hover {
  background: var(--bg-hover);
}

.menu-item--danger {
  color: var(--color-danger);
}

.menu-item--danger:hover {
  background: var(--color-danger-light);
  opacity: 0.8;
}

/* 卡片下拉菜单进出动画 */
.card-menu-enter-active,
.card-menu-leave-active {
  transition: var(--transition-dropdown);
  transform-origin: top right;
}
.card-menu-enter-from {
  transform: translateY(-6px) scale(0.95);
  opacity: 0.8;
}
.card-menu-leave-to {
  transform: translateY(-6px) scale(0.95);
  opacity: 0;
}
</style>
