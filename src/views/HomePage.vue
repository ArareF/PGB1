<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useNavigation } from '../composables/useNavigation'
import { useProjects } from '../composables/useProjects'
import { useSettings } from '../composables/useSettings'
import { useStatusBar } from '../composables/useStatusBar'
import { useDirectoryFiles } from '../composables/useDirectoryFiles'
import { useNotes, usePageNote } from '../composables/useNotes'
import type { ProjectInfo } from '../composables/useProjects'
import ProjectCard from '../components/ProjectCard.vue'
import CreateProjectDialog from '../components/CreateProjectDialog.vue'
import EditProjectDialog from '../components/EditProjectDialog.vue'
import NoteDialog from '../components/NoteDialog.vue'
import NoteRenderer from '../components/NoteRenderer.vue'
import PageGuideOverlay from '../components/PageGuideOverlay.vue'
import { PAGE_GUIDE_ANNOTATIONS } from '../config/onboarding'
import { priorityRank } from '../config/priority'

const router = useRouter()
const { t } = useI18n()
const { setNavigation } = useNavigation()
const { projects, loading, loadProjects } = useProjects()
const { loadSettings } = useSettings()
const { openInExplorer } = useDirectoryFiles()
const { isOvertime, startOvertime, endOvertime } = useStatusBar()

const showCreateDialog = ref(false)
const showGuide = ref(false)
const projectRootDir = ref('')

// 笔记
const noteTarget = ref<ProjectInfo | null>(null)
const showNoteDialog = ref(false)
const { loadNotes: loadPageNotes, getNote: getPageNote, saveNote: savePageNote, hasNote: hasPageNote } = useNotes(projectRootDir)
const { showPageNote, pageNoteText, openPageNote, closePageNote, onPageNoteSave, onPageNoteUpdate, onPageNoteCheckbox } =
  usePageNote(getPageNote, savePageNote, 'page')
async function openPinboard() {
  if (!projectRootDir.value) return
  await invoke('open_pinboard_window', {
    dirPath: projectRootDir.value,
    canvasKey: 'home',
    title: t('home.myProjects'),
  })
}
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

// 仅识别标准日期格式（YYYY-MM-DD），忽略文字备注（如"转交了"）
const DATE_PATTERN = /^\d{4}-\d{2}-\d{2}$/

function parseDeadline(deadline: string | null | undefined): Date | null {
  if (!deadline || !DATE_PATTERN.test(deadline)) return null
  // 不用 new Date('YYYY-MM-DD')：JS 会把纯日期当作 UTC 00:00 解析，
  // 负时区机器（如 UTC-7）下 '2026-04-14' 会落到本地 2026-04-13 17:00，
  // 导致"是否逾期"判断与排序整体错位。必须按本地时区构造。
  const [y, m, d] = deadline.split('-').map(Number)
  const date = new Date(y, m - 1, d)
  return isNaN(date.getTime()) ? null : date
}

const sortedProjects = computed(() => {
  const list = [...projects.value]
  if (sortMode.value === 'default') return list

  if (sortMode.value === 'priority') {
    return list.sort((a, b) => {
      const ao = priorityRank(a.priority)
      const bo = priorityRank(b.priority)
      if (ao !== bo) return ao - bo
      return a.name.localeCompare(b.name)
    })
  }

  // deadline 排序：优先度最优先，其次按截止日期
  const today = new Date()
  today.setHours(0, 0, 0, 0)
  return list.sort((a, b) => {
    // 第一键：优先度
    const ao = priorityRank(a.priority)
    const bo = priorityRank(b.priority)
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

function onProjectAction(project: ProjectInfo, action: 'rename' | 'deadline' | 'delete' | 'note') {
  if (action === 'note') {
    noteTarget.value = project
    showNoteDialog.value = true
    return
  }
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

async function onNoteSave(text: string) {
  if (!noteTarget.value) return
  await invoke('set_note', {
    dirPath: noteTarget.value.path,
    key: 'card:' + noteTarget.value.name.toLowerCase(),
    note: text || null,
  })
  showNoteDialog.value = false
  noteTarget.value = null
  loadProjects()
}

/** checkbox 切换：静默保存，不关闭弹窗 */
async function onNoteUpdate(text: string) {
  if (!noteTarget.value) return
  noteTarget.value.note = text || null
  await invoke('set_note', {
    dirPath: noteTarget.value.path,
    key: 'card:' + noteTarget.value.name.toLowerCase(),
    note: text || null,
  })
}

function closeNoteDialog() {
  showNoteDialog.value = false
  noteTarget.value = null
}

/** tooltip checkbox 切换：直接持久化 */
async function onTooltipNoteSave(project: ProjectInfo, text: string) {
  project.note = text || null
  await invoke('set_note', {
    dirPath: project.path,
    key: 'card:' + project.name.toLowerCase(),
    note: text || null,
  })
}

function onProjectRefresh() {
  loadProjects()
}

/* 注册主页导航配置（根据加班状态动态更新 actions） */
function refreshHomeNav() {
  const actions = isOvertime.value
    ? [{
        id: 'end-overtime',
        label: t('home.endOvertime'),
        variant: 'success' as const,
        handler: async () => {
          try {
            await invoke('open_reminder_window', { reminderType: 'overtime' })
          } catch (e) {
            console.error('打开退勤打卡窗口失败:', e)
          }
        },
      }]
    : []
  setNavigation({
    title: t('app.name'),
    showBackButton: false,
    actions,
    moreMenuItems: [
      { id: 'page-guide', label: t('common.pageGuide'), handler: () => { showGuide.value = true } },
    ],
  })
}

refreshHomeNav()
watch(isOvertime, refreshHomeNav)

function onVisibilityChange() {
  if (document.visibilityState === 'visible') loadProjects()
}

let unlistenOvertimeStarted: UnlistenFn | null = null
let unlistenOvertimeEnded: UnlistenFn | null = null
let unlistenClockProgress: UnlistenFn | null = null

onMounted(async () => {
  loadProjects()
  document.addEventListener('visibilitychange', onVisibilityChange)
  // 补偿检测：用户可能在非主页时点了加班，localStorage 已设置但 ref 未同步
  if (localStorage.getItem('pgb1-overtime-active') === 'true' && !isOvertime.value) {
    startOvertime()
  }
  const s = await loadSettings()
  if (s?.general.projectRootDir) {
    projectRootDir.value = s.general.projectRootDir
    await loadPageNotes()
  }
  // 监听退勤弹窗发出的加班事件
  unlistenOvertimeStarted = await listen('overtime-started', () => {
    startOvertime()
  })
  // 监听加班状态清除（出勤弹窗检测到漏打退勤时发出）
  unlistenOvertimeEnded = await listen('overtime-ended', () => {
    endOvertime()
  })
  // 监听打卡完成事件（退勤成功后清除加班状态）
  unlistenClockProgress = await listen('clock-progress', (event: any) => {
    const step = event.payload?.step
    if (isOvertime.value && (step === 'success' || step === 'already-done')) {
      endOvertime()
    }
  })
})
onUnmounted(() => {
  document.removeEventListener('visibilitychange', onVisibilityChange)
  unlistenOvertimeStarted?.()
  unlistenOvertimeEnded?.()
  unlistenClockProgress?.()
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
      <div v-if="hasPageNote('page')" class="note-preview-inline">
        <NoteRenderer :text="getPageNote('page')!" @toggle-checkbox="onPageNoteCheckbox('page', $event)" />
      </div>
      <button
        v-if="projectRootDir"
        class="note-btn"
        :class="{ 'has-note': hasPageNote('page') }"
        :title="$t('note.pageNote')"
        @click="openPageNote()"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>
      </button>
      <button
        v-if="projectRootDir"
        class="note-btn"
        :title="$t('pinboard.title')"
        @click="openPinboard"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18"/><path d="M3 9h6"/></svg>
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
          @note-save="onTooltipNoteSave"
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

    <NoteDialog
      :show="showNoteDialog"
      :title="$t('note.editNote')"
      :note="noteTarget?.note ?? ''"
      @save="onNoteSave"
      @update="onNoteUpdate"
      @cancel="closeNoteDialog"
    />

    <NoteDialog
      :show="showPageNote"
      :title="$t('note.pageNote')"
      :note="pageNoteText"
      @save="onPageNoteSave"
      @update="onPageNoteUpdate"
      @cancel="closePageNote"
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
  padding: var(--spacing-4) var(--spacing-2) var(--spacing-2);
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

/* .sort-tabs, .sort-tab → design-system.css 公共类 */

</style>
