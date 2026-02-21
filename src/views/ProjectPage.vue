<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useNavigation } from '../composables/useNavigation'
import { useProjects } from '../composables/useProjects'
import { useTasks } from '../composables/useTasks'
import { useDirectoryFiles } from '../composables/useDirectoryFiles'
import type { TaskInfo } from '../composables/useTasks'
import TaskCard from '../components/TaskCard.vue'

const route = useRoute()
const router = useRouter()
const { setNavigation } = useNavigation()
const { projects, loadProjects } = useProjects()
const { tasks, loading, loadTasks } = useTasks()
const { openInExplorer } = useDirectoryFiles()

const projectId = route.params.projectId as string
let projectPath = ''

const enabledTasks = ref<string[]>([])
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
    { id: 'game-intro', label: '游戏介绍', handler: () => router.push({ name: 'gameIntro', params: { projectId } }) },
    { id: 'materials', label: '项目素材', handler: () => router.push({ name: 'materials', params: { projectId } }) },
    { id: 'ae-project', label: '打开AE', handler: () => openAeProject(), onLongPress: () => openAeProjectPicker(), active: !!defaultAeFile.value },
    {
      id: 'task-list',
      label: '任务列表',
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
      { id: 'open-folder', label: '打开项目文件夹', handler: () => { if (projectPath) openInExplorer(projectPath) } },
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

async function openAeProjectPicker() {
  if (!projectPath) return
  const files = await scanAepFiles()
  if (files.length === 0) {
    try { openInExplorer(getAeDir()) } catch { openInExplorer(projectPath) }
    return
  }
  aepFiles.value = files
  aepPickerVisible.value = true
}

async function pickAepFile(file: FileEntry) {
  aepPickerVisible.value = false
  // 设为默认并打开
  defaultAeFile.value = file.name
  refreshNav()  // 按钮 active 即时更新
  await invoke('set_default_ae_file', { projectPath, fileName: file.name })
  await invoke('open_file', { path: file.path })
}

async function onTasksSaved() {
  // 重新加载项目信息以更新 enabledTasks
  await loadProjects()
  const project = projects.value.find(p => p.name === projectId)
  if (project) {
    enabledTasks.value = project.enabled_tasks
    completedSubtasks.value = project.completed_subtasks
    await loadTasks(project.path)
  }
}
</script>

<template>
  <div class="project-page">
    <!-- 固定小标题栏 -->
    <div class="sub-title-bar">
      <span class="sub-title">制作任务</span>
    </div>

    <!-- 可滚动内容区 -->
    <div class="scroll-content">
      <p v-if="loading" class="loading-text">扫描中...</p>

      <TransitionGroup v-else name="card" tag="div" class="card-grid">
        <TaskCard
          v-for="(task, i) in tasks"
          :key="task.name"
          :style="{ '--delay': i * 40 + 'ms' }"
          :task="task"
          :subtask-progress="taskSubtaskProgress[task.name.toLowerCase()]"
          @click="openTask"
        />
      </TransitionGroup>
    </div>

    <!-- AE 工程文件选择弹窗 -->
    <Teleport to="body">
      <div v-if="aepPickerVisible" class="aep-overlay" @click.self="aepPickerVisible = false">
        <div class="aep-picker glass-strong">
          <p class="aep-picker-title">选择 AE 工程文件</p>
          <p class="aep-picker-hint">点击设为默认并打开</p>
          <div class="aep-picker-list">
            <button
              v-for="file in aepFiles"
              :key="file.path"
              class="aep-picker-item"
              :class="{ 'is-default': file.name === defaultAeFile }"
              @click="pickAepFile(file)"
            >
              <span class="aep-item-name">{{ file.name }}</span>
              <span v-if="file.name === defaultAeFile" class="aep-item-badge">默认</span>
            </button>
          </div>
          <button class="aep-picker-cancel" @click="aepPickerVisible = false">取消</button>
        </div>
      </div>
    </Teleport>

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

/* AE 工程选择弹窗（Teleport to body，需要用 :deep 或非 scoped，此处用 :global） */
</style>

<style>
.aep-overlay {
  position: fixed;
  inset: 0;
  z-index: 200;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-backdrop);
  backdrop-filter: blur(var(--glass-light-blur));
  -webkit-backdrop-filter: blur(var(--glass-light-blur));
}

.aep-picker {
  min-width: 320px;
  max-width: 480px;
  border-radius: var(--radius-xl);
  padding: var(--spacing-5);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.aep-picker-title {
  font-size: var(--text-xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
  margin: 0;
}

.aep-picker-hint {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
  margin: 0;
}

.aep-picker-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
  max-height: 320px;
  overflow-y: auto;
}

.aep-picker-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-3);
  text-align: left;
  padding: var(--spacing-3) var(--spacing-4);
  border: 1px solid var(--border-light);
  border-radius: var(--radius-md);
  background: var(--bg-hover);
  color: var(--text-primary);
  font-size: var(--text-base);
  font-family: inherit;
  cursor: pointer;
  transition: var(--transition-bg);
}

.aep-picker-item:hover {
  background: var(--glass-subtle-bg);
  border-color: var(--border-medium);
}

.aep-picker-item.is-default {
  border-color: color-mix(in srgb, var(--color-primary-500) 40%, transparent);
  background: color-mix(in srgb, var(--color-primary-500) 8%, transparent);
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

.aep-picker-cancel {
  align-self: flex-end;
  padding: var(--spacing-2) var(--spacing-5);
  border: 1px solid var(--border-light);
  border-radius: var(--radius-button);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-base);
  font-family: inherit;
  cursor: pointer;
  transition: var(--transition-bg);
}

.aep-picker-cancel:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}
</style>
