<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useNavigation } from '../composables/useNavigation'
import { useProjects } from '../composables/useProjects'
import { useSettings } from '../composables/useSettings'
import { useDirectoryFiles } from '../composables/useDirectoryFiles'
import type { ProjectInfo } from '../composables/useProjects'
import ProjectCard from '../components/ProjectCard.vue'
import CreateProjectDialog from '../components/CreateProjectDialog.vue'
import EditProjectDialog from '../components/EditProjectDialog.vue'
import PageGuideOverlay from '../components/PageGuideOverlay.vue'
import { PAGE_GUIDE_ANNOTATIONS } from '../config/onboarding'

const router = useRouter()
const { t } = useI18n()
const { setNavigation } = useNavigation()
const { projects, loading, loadProjects } = useProjects()
const { loadSettings } = useSettings()
const { openInExplorer } = useDirectoryFiles()

const showCreateDialog = ref(false)
const showGuide = ref(false)
const projectRootDir = ref('')

// 排序模式（localStorage 持久化）
const SORT_MODE_KEY = 'pgb1-home-sort'
const sortMode = ref<'default' | 'deadline' | 'priority'>(
  (localStorage.getItem(SORT_MODE_KEY) as 'default' | 'deadline' | 'priority') ?? 'default'
)
watch(sortMode, val => localStorage.setItem(SORT_MODE_KEY, val))

// 判断项目是否已完成
function isProjectComplete(p: ProjectInfo): boolean {
  const enabled = p.enabled_tasks
  const parentTasks = enabled.filter(t => !t.includes('/'))
  let total = 0, done = 0
  const completedSubs = new Set(p.completed_subtasks)
  const completedT = new Set(p.completed_tasks)
  for (const parent of parentTasks) {
    const children = enabled.filter(t => t.startsWith(parent + '/'))
    if (children.length === 0) {
      total++
      if (completedT.has(parent)) done++
    } else {
      total += children.length
      done += children.filter(c => completedSubs.has(c)).length
    }
  }
  return total > 0 && done >= total
}

// 优先度排序：high(0) > medium(1) > null/无(2) > low(3)
const PRIORITY_ORDER: Record<string, number> = { high: 0, medium: 1, low: 3 }

// 仅识别标准日期格式（YYYY-MM-DD），忽略文字备注（如"转交了"）
const DATE_PATTERN = /^\d{4}-\d{2}-\d{2}$/

function parseDeadline(deadline: string | null | undefined): Date | null {
  if (!deadline || !DATE_PATTERN.test(deadline)) return null
  const d = new Date(deadline)
  return isNaN(d.getTime()) ? null : d
}

const sortedProjects = computed(() => {
  const list = [...projects.value]
  if (sortMode.value === 'default') return list

  if (sortMode.value === 'priority') {
    return list.sort((a, b) => {
      const ao = a.priority ? (PRIORITY_ORDER[a.priority] ?? 2) : 2
      const bo = b.priority ? (PRIORITY_ORDER[b.priority] ?? 2) : 2
      if (ao !== bo) return ao - bo
      return a.name.localeCompare(b.name)
    })
  }

  // deadline 排序：优先度最优先，其次按截止日期
  const today = new Date()
  today.setHours(0, 0, 0, 0)
  return list.sort((a, b) => {
    // 第一键：优先度
    const ao = a.priority ? (PRIORITY_ORDER[a.priority] ?? 2) : 2
    const bo = b.priority ? (PRIORITY_ORDER[b.priority] ?? 2) : 2
    if (ao !== bo) return ao - bo
    // 第二键：有效日期在前，无有效日期（含文字备注）沉底
    const aDate = parseDeadline(a.deadline)
    const bDate = parseDeadline(b.deadline)
    if (!!aDate !== !!bDate) return aDate ? -1 : 1
    if (!aDate || !bDate) return a.name.localeCompare(b.name)
    // 同优先度 + 同有日期：按完成状态，再按日期
    const aComplete = isProjectComplete(a)
    const bComplete = isProjectComplete(b)
    if (aComplete !== bComplete) return aComplete ? 1 : -1
    const aOverdue = aDate < today
    const bOverdue = bDate < today
    if (aOverdue !== bOverdue) return aOverdue ? -1 : 1
    return aDate.getTime() - bDate.getTime()
  })
})

const editTarget = ref<ProjectInfo | null>(null)
const editMode = ref<'rename' | 'deadline' | 'delete' | null>(null)

function onProjectAction(project: ProjectInfo, action: 'rename' | 'deadline' | 'delete') {
  editTarget.value = project
  editMode.value = action
}

function onProjectUpdated(_updated: ProjectInfo) {
  editTarget.value = null
  editMode.value = null
  loadProjects()
}

function onProjectDeleted(_path: string) {
  editTarget.value = null
  editMode.value = null
  loadProjects()
}

function closeEditDialog() {
  editTarget.value = null
  editMode.value = null
}

function onProjectRefresh() {
  loadProjects()
}

/* 注册主页导航配置 */
setNavigation({
  title: t('app.name'),
  showBackButton: false,
  actions: [],
  moreMenuItems: [
    { id: 'page-guide', label: t('common.pageGuide'), handler: () => { showGuide.value = true } },
  ],
})

function onVisibilityChange() {
  if (document.visibilityState === 'visible') loadProjects()
}

onMounted(async () => {
  loadProjects()
  document.addEventListener('visibilitychange', onVisibilityChange)
  const s = await loadSettings()
  if (s?.general.projectRootDir) projectRootDir.value = s.general.projectRootDir
})
onUnmounted(() => {
  document.removeEventListener('visibilitychange', onVisibilityChange)
})

function openProject(project: ProjectInfo) {
  router.push({ name: 'project', params: { projectId: project.name } })
}

function onProjectCreated(projectName: string) {
  showCreateDialog.value = false
  router.push({ name: 'project', params: { projectId: projectName } })
}
</script>

<template>
  <div class="home-page">
    <div class="page-header">
      <p class="page-hint">{{ $t('home.myProjects') }}</p>
      <button
        v-if="projectRootDir"
        class="folder-btn"
        :title="$t('home.openProjectFolder')"
        @click="openInExplorer(projectRootDir)"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
        </svg>
      </button>
      <div class="sort-tabs">
        <button
          v-for="mode in (['default', 'deadline', 'priority'] as const)"
          :key="mode"
          class="sort-tab"
          :class="{ 'is-active': sortMode === mode }"
          @click="sortMode = mode"
        >{{ $t(`home.sort${mode.charAt(0).toUpperCase() + mode.slice(1)}`) }}</button>
      </div>
      <button class="add-btn" :title="$t('home.createProject')" @click="showCreateDialog = true">+</button>
    </div>

    <!-- 可滚动内容区 -->
    <div class="scroll-content">
      <p v-if="loading" class="loading-text">{{ $t('common.scanning') }}</p>

      <TransitionGroup v-else name="card" tag="div" class="card-grid">
        <ProjectCard
          v-for="(project, i) in sortedProjects"
          :key="project.name"
          :style="{ '--delay': i * 40 + 'ms' }"
          :project="project"
          @click="openProject"
          @action="onProjectAction"
          @refresh="onProjectRefresh"
        />
      </TransitionGroup>
    </div>

    <CreateProjectDialog
      :show="showCreateDialog"
      @created="onProjectCreated"
      @cancel="showCreateDialog = false"
    />

    <EditProjectDialog
      v-if="editTarget && editMode"
      :show="true"
      :project="editTarget"
      :mode="editMode!"
      @updated="onProjectUpdated"
      @deleted="onProjectDeleted"
      @cancel="closeEditDialog"
    />

  </div>

  <PageGuideOverlay :show="showGuide" :annotations="PAGE_GUIDE_ANNOTATIONS.home" @close="showGuide = false" />
</template>

<style scoped>
.home-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

/* 可滚动区 */
.scroll-content {
  flex: 1;
  overflow-y: auto;
  padding-top: var(--spacing-4);
}

.page-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-4);
  padding-bottom: var(--spacing-4);
  border-bottom: 1px solid var(--border-medium);
}

.page-hint {
  font-size: var(--text-3xl);
  color: var(--text-primary);
}

.add-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-secondary);
  background: transparent;
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.add-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
  border-color: var(--color-primary);
}

.loading-text {
  font-size: var(--text-lg);
  color: var(--text-tertiary);
}

.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--card-project-width), 1fr));
  gap: var(--gap-card);
}

.sort-tabs {
  display: flex;
  gap: var(--spacing-1);
  margin-left: auto;
}

.sort-tab {
  height: 28px;
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
