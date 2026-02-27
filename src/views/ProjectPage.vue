<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useNavigation } from '../composables/useNavigation'
import { useProjects } from '../composables/useProjects'
import { useTasks } from '../composables/useTasks'
import { useDirectoryFiles } from '../composables/useDirectoryFiles'
import type { TaskInfo } from '../composables/useTasks'
import TaskCard from '../components/TaskCard.vue'
import PageGuideOverlay from '../components/PageGuideOverlay.vue'
import { PAGE_GUIDE_ANNOTATIONS } from '../config/onboarding'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const { setNavigation } = useNavigation()
const { projects, loadProjects } = useProjects()
const { tasks, loading, loadTasks } = useTasks()
const { openInExplorer } = useDirectoryFiles()

const projectId = route.params.projectId as string
let projectPath = ''

const enabledTasks = ref<string[]>([])
const showGuide = ref(false)

// 排序模式（localStorage 持久化）
const SORT_MODE_KEY = 'pgb1-project-sort'
const sortMode = ref<'default' | 'priority'>(
  (localStorage.getItem(SORT_MODE_KEY) as 'default' | 'priority') ?? 'default'
)
watch(sortMode, val => localStorage.setItem(SORT_MODE_KEY, val))

// 优先度排序：high(0) > medium(1) > null/无(2) > low(3)
const PRIORITY_ORDER: Record<string, number> = { high: 0, medium: 1, low: 3 }

const sortedTasks = computed(() => {
  const list = [...tasks.value]
  if (sortMode.value === 'default') return list
  return list.sort((a, b) => {
    const ao = a.priority ? (PRIORITY_ORDER[a.priority] ?? 2) : 2
    const bo = b.priority ? (PRIORITY_ORDER[b.priority] ?? 2) : 2
    if (ao !== bo) return ao - bo
    return a.name.localeCompare(b.name)
  })
})

// 处理 TaskCard 的优先度 action
async function onTaskAction(task: TaskInfo, _action: 'priority', value: string | null) {
  if (!projectPath) return
  await invoke('set_task_priority', {
    projectPath,
    taskName: task.name,
    priority: value,
  })
  await loadTasks(projectPath)
}

const completedSubtasks = ref<string[]>([])
const defaultAeFile = ref<string | null>(null)

/** 计算每个任务的子任务进度：{ taskNameLower: { completed, total } } */
const taskSubtaskProgress = computed(() => {
  const map: Record<string, { completed: number; total: number }> = {}
  const completedSet = new Set(completedSubtasks.value)

  for (const key of enabledTasks.value) {
    if (!key.includes('/')) continue
    const taskName = key.split('/')[0].toLowerCase()
    if (!map[taskName]) {
      map[taskName] = { completed: 0, total: 0 }
    }
    map[taskName].total++
    if (completedSet.has(key)) {
      map[taskName].completed++
    }
  }
  return map
})

function buildNavActions() {
  return [
    { id: 'game-intro', label: t('project.gameIntro'), handler: () => router.push({ name: 'gameIntro', params: { projectId } }) },
    { id: 'materials', label: t('project.projectMaterials'), handler: () => router.push({ name: 'materials', params: { projectId } }) },
    { id: 'ae-project', label: t('project.openAE'), handler: () => openAeProject(), onLongPress: (rect: DOMRect) => openAeProjectPicker(rect), active: !!defaultAeFile.value },
    {
      id: 'task-list',
      label: t('project.taskList'),
      handler: () => {
        router.push({
          name: 'taskList',
          params: { projectId },
          query: {
            projectPath,
            enabledTasks: JSON.stringify(enabledTasks.value),
          },
        })
      },
    },
  ]
}

function refreshNav() {
  setNavigation({
    title: projectId,
    showBackButton: true,
    onBack: () => router.push({ name: 'home' }),
    actions: buildNavActions(),
    moreMenuItems: [
      { id: 'page-guide', label: t('common.pageGuide'), handler: () => { showGuide.value = true } },
    ],
  })
}

/* 注册项目页导航配置（初始，active 为 false） */
refreshNav()

onMounted(async () => {
  await loadProjects()
  const project = projects.value.find(p => p.name === projectId)
  if (project) {
    projectPath = project.path
    enabledTasks.value = project.enabled_tasks
    completedSubtasks.value = project.completed_subtasks
    defaultAeFile.value = project.default_ae_file
    refreshNav()   // 更新 active 状态
    await loadTasks(project.path)
  }
})

function openTask(task: TaskInfo) {
  router.push({ name: 'task', params: { projectId, taskId: task.name } })
}

interface FileEntry { name: string; path: string; is_dir: boolean }

const aepPickerVisible = ref(false)
const aepFiles = ref<FileEntry[]>([])
const aepPanelStyle = ref({ top: '0px', right: '0px' })

function getAeDir() {
  return projectPath.replace(/\\/g, '/') + '/03_Render_VFX/VFX/AE'
}

async function scanAepFiles(): Promise<FileEntry[]> {
  try {
    const entries = await invoke<FileEntry[]>('scan_directory', { dirPath: getAeDir() })
    return entries
      .filter(e => !e.is_dir && e.name.toLowerCase().endsWith('.aep'))
      .sort((a, b) => a.name.localeCompare(b.name))
  } catch {
    return []
  }
}

async function openAeProject() {
  if (!projectPath) return
  const files = await scanAepFiles()
  if (files.length === 0) {
    try { openInExplorer(getAeDir()) } catch { openInExplorer(projectPath) }
    return
  }
  // 有默认打默认，否则打文件名排序最后一个（最新）
  const target = defaultAeFile.value
    ? files.find(f => f.name === defaultAeFile.value) ?? files[files.length - 1]
    : files[files.length - 1]
  await invoke('open_file', { path: target.path })
}

async function openAeProjectPicker(btnRect: DOMRect) {
  if (!projectPath) return
  const files = await scanAepFiles()
  if (files.length === 0) {
    try { openInExplorer(getAeDir()) } catch { openInExplorer(projectPath) }
    return
  }
  aepFiles.value = files
  aepPanelStyle.value = {
    top: `${btnRect.bottom + 8}px`,
    right: `${window.innerWidth - btnRect.right}px`,
  }
  aepPickerVisible.value = true
}

function onAepOutsideClick() {
  // 面板内的点击由 @click.stop 阻止冒泡，到达这里说明点在外部
  if (aepPickerVisible.value) {
    aepPickerVisible.value = false
  }
}

async function pickAepFile(file: FileEntry) {
  aepPickerVisible.value = false
  // 设为默认并打开
  defaultAeFile.value = file.name
  refreshNav()  // 按钮 active 即时更新
  await invoke('set_default_ae_file', { projectPath, fileName: file.name })
  await invoke('open_file', { path: file.path })
}

onMounted(() => {
  document.addEventListener('click', onAepOutsideClick)
})

onUnmounted(() => {
  document.removeEventListener('click', onAepOutsideClick)
})

</script>

<template>
  <div class="project-page">
    <!-- 固定小标题栏 -->
    <div class="sub-title-bar">
      <span class="sub-title">{{ $t('project.tasks') }}</span>
      <button
        class="folder-btn"
        :title="$t('project.openProjectFolder')"
        @click="openInExplorer(projectPath)"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
        </svg>
      </button>
      <div class="sort-tabs">
        <button
          v-for="mode in (['default', 'priority'] as const)"
          :key="mode"
          class="sort-tab"
          :class="{ 'is-active': sortMode === mode }"
          @click="sortMode = mode"
        >{{ $t(`project.sort${mode.charAt(0).toUpperCase() + mode.slice(1)}`) }}</button>
      </div>
    </div>

    <!-- 可滚动内容区 -->
    <div class="scroll-content">
      <p v-if="loading" class="loading-text">{{ $t('common.scanning') }}</p>

      <TransitionGroup v-else name="card" tag="div" class="card-grid">
        <TaskCard
          v-for="(task, i) in sortedTasks"
          :key="task.name"
          :style="{ '--delay': i * 40 + 'ms' }"
          :task="task"
          :subtask-progress="taskSubtaskProgress[task.name.toLowerCase()]"
          @click="openTask"
          @action="onTaskAction"
        />
      </TransitionGroup>
    </div>

    <!-- AE 工程文件选择下拉面板 -->
    <Teleport to="body">
      <Transition name="aep-panel">
        <div
          v-if="aepPickerVisible"
          class="aep-dropdown-panel"
          :style="aepPanelStyle"
          @click.stop
        >
          <div class="aep-dropdown-title">{{ $t('project.selectAeFile') }}</div>
          <div class="aep-dropdown-hint">{{ $t('project.clickToSetDefault') }}</div>
          <div class="aep-dropdown-list">
            <button
              v-for="file in aepFiles"
              :key="file.path"
              class="aep-dropdown-item"
              :class="{ 'is-default': file.name === defaultAeFile }"
              @click="pickAepFile(file)"
            >
              <span class="aep-item-name">{{ file.name }}</span>
              <span v-if="file.name === defaultAeFile" class="aep-item-badge">{{ $t('project.default') }}</span>
            </button>
          </div>
        </div>
      </Transition>
    </Teleport>

    <PageGuideOverlay :show="showGuide" :annotations="PAGE_GUIDE_ANNOTATIONS.project" @close="showGuide = false" />
  </div>
</template>

<style scoped>
.project-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

/* .sub-title-bar, .sub-title → design-system.css 公共类 */

/* 可滚动区 */
.scroll-content {
  flex: 1;
  overflow-y: auto;
  padding-top: var(--spacing-4);
}

.loading-text {
  font-size: var(--text-lg);
  color: var(--text-tertiary);
}

.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--card-task-width), 1fr));
  gap: var(--gap-card);
}

/* AE 工程选择下拉面板（Teleport to body，需要全局样式） */

.sub-title-bar {
  gap: var(--spacing-3);
}

.sort-tabs {
  display: flex;
  gap: var(--spacing-1);
  margin-left: auto;
}

.sort-tab {
  height: 26px;
  padding: 0 var(--spacing-3);
  font-size: var(--text-xs);
  font-family: inherit;
  color: var(--text-secondary);
  background: transparent;
  border: 1px solid var(--border-light);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.sort-tab:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.sort-tab.is-active {
  color: var(--color-primary-300);
  background: color-mix(in srgb, var(--color-primary-500) 12%, transparent);
  border-color: var(--color-primary-700);
}
</style>

<style>
.aep-dropdown-panel {
  position: fixed;
  z-index: var(--z-dropdown);
  min-width: 240px;
  max-width: 400px;
  padding: var(--padding-sm);
  border-radius: var(--radius-md);
  background: var(--glass-medium-bg);
  border: 1px solid var(--border-light);
  box-shadow: var(--shadow-lg);
  backdrop-filter: blur(var(--panel-blur));
  -webkit-backdrop-filter: blur(var(--panel-blur));
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.aep-dropdown-title {
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  color: var(--text-secondary);
  padding: var(--padding-xs);
  border-bottom: 1px solid var(--border-light);
  margin-bottom: var(--spacing-1);
}

.aep-dropdown-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  padding: 0 var(--padding-xs);
}

.aep-dropdown-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
  max-height: 280px;
  overflow-y: auto;
}

.aep-dropdown-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-2);
  text-align: left;
  padding: var(--padding-xs) var(--padding-xs);
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-primary);
  font-size: var(--text-xs);
  font-family: inherit;
  cursor: pointer;
  transition: background 0.15s ease;
}

.aep-dropdown-item:hover {
  background: var(--bg-hover);
}

.aep-dropdown-item.is-default {
  background: color-mix(in srgb, var(--color-primary-500) 10%, transparent);
  color: var(--color-primary-500);
}

.aep-dropdown-item.is-default:hover {
  background: color-mix(in srgb, var(--color-primary-500) 18%, transparent);
}

.aep-item-name {
  word-break: break-all;
  flex: 1;
}

.aep-item-badge {
  flex-shrink: 0;
  font-size: var(--text-xs);
  padding: 2px var(--spacing-2);
  border-radius: var(--radius-tag);
  background: color-mix(in srgb, var(--color-primary-500) 20%, transparent);
  color: var(--color-primary-500);
}

/* 下拉面板进出场动画 */
.aep-panel-enter-active,
.aep-panel-leave-active {
  transition: var(--transition-dropdown);
  transform-origin: top right;
}
.aep-panel-enter-from {
  transform: translateY(-6px) scale(0.95);
  opacity: 0;
}
.aep-panel-leave-to {
  transform: translateY(-6px) scale(0.95);
  opacity: 0;
}
</style>
