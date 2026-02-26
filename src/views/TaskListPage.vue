<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import { useNavigation } from '../composables/useNavigation'
import { useSettings } from '../composables/useSettings'
import PageGuideOverlay from '../components/PageGuideOverlay.vue'
import { PAGE_GUIDE_ANNOTATIONS } from '../config/onboarding'

const { t } = useI18n()

interface GlobalTaskChild {
  name: string
}

interface GlobalTask {
  name: string
  children: GlobalTaskChild[]
}

interface GlobalTaskConfig {
  tasks: GlobalTask[]
}

interface ApplyTaskResult {
  created: string[]
  archived: string[]
  errors: string[]
}

interface ArchivedVersion {
  task_name: string
  timestamp: string
  display_time: string
  path: string
}

type TabType = 'enable' | 'edit' | 'archive'

const route = useRoute()
const router = useRouter()
const { setNavigation } = useNavigation()
const showGuide = ref(false)

// 从路由参数/query 读取
const projectId = route.params.projectId as string
const projectPath = route.query.projectPath as string
const enabledTasksRaw = route.query.enabledTasks as string
const initialEnabledTasks: string[] = enabledTasksRaw ? JSON.parse(enabledTasksRaw) : []

const { loadSettings } = useSettings()

const globalTasks = ref<GlobalTask[]>([])
const checkedTasks = ref<Set<string>>(new Set())
const loading = ref(true)
const saving = ref(false)
const activeTab = ref<TabType>('enable')

// ─── Tab 2: 模板编辑状态 ───
const editedTasks = ref<GlobalTask[]>([])
const newTaskName = ref('')
const newChildNames = ref<Record<number, string>>({})

// ─── Tab 3: 时光机状态 ───
const archivedVersions = ref<ArchivedVersion[]>([])
const archiveLoading = ref(false)

// ─── 内部确认/提示弹窗 ───
const innerDialog = ref<{
  visible: boolean
  title: string
  message: string
  type: 'confirm' | 'alert'
  onConfirm: (() => void) | null
}>({
  visible: false,
  title: '',
  message: '',
  type: 'confirm',
  onConfirm: null,
})

function showConfirm(title: string, message: string, onConfirm: () => void) {
  innerDialog.value = { visible: true, title, message, type: 'confirm', onConfirm }
}

function showAlert(title: string, message: string) {
  innerDialog.value = { visible: true, title, message, type: 'alert', onConfirm: null }
}

function handleInnerConfirm() {
  innerDialog.value.onConfirm?.()
  innerDialog.value.visible = false
}

function handleInnerCancel() {
  innerDialog.value.visible = false
}

/** Tab 1 是否有变更 */
const hasEnableChanges = computed(() => {
  const oldSet = new Set(initialEnabledTasks)
  if (oldSet.size !== checkedTasks.value.size) return true
  for (const name of checkedTasks.value) {
    if (!oldSet.has(name)) return true
  }
  return false
})

/** Tab 2 是否有变更 */
const hasTemplateChanges = computed(() => {
  if (editedTasks.value.length !== globalTasks.value.length) return true
  for (let i = 0; i < editedTasks.value.length; i++) {
    const edited = editedTasks.value[i]
    const original = globalTasks.value[i]
    if (edited.name !== original.name) return true
    if (edited.children.length !== original.children.length) return true
    for (let j = 0; j < edited.children.length; j++) {
      if (edited.children[j].name !== original.children[j].name) return true
    }
  }
  return false
})

// 切换 Tab 时初始化数据
watch(activeTab, (tab) => {
  if (tab === 'edit') {
    editedTasks.value = JSON.parse(JSON.stringify(globalTasks.value))
    newTaskName.value = ''
    newChildNames.value = {}
  } else if (tab === 'archive') {
    loadArchivedTasks()
  }
})

onMounted(async () => {
  setNavigation({
    title: `${t('taskList.title')} · ${projectId}`,
    showBackButton: true,
    onBack: () => router.push({ name: 'project', params: { projectId } }),
    actions: [],
    moreMenuItems: [
      { id: 'page-guide', label: t('common.pageGuide'), handler: () => { showGuide.value = true } },
    ],
  })

  try {
    const s = await loadSettings()
    const config = await invoke<GlobalTaskConfig>('load_global_tasks', {
      rootDir: s?.general.projectRootDir ?? '',
    })
    globalTasks.value = config.tasks
    checkedTasks.value = new Set(initialEnabledTasks)
  } catch (e) {
    console.error('加载全局任务清单失败:', e)
  } finally {
    loading.value = false
  }
})

// ─── Tab 1 操作 ───

function toggleTask(taskName: string) {
  const newSet = new Set(checkedTasks.value)
  if (newSet.has(taskName)) {
    newSet.delete(taskName)
  } else {
    newSet.add(taskName)
  }
  checkedTasks.value = newSet
}

function toggleChild(taskName: string, childName: string) {
  const key = `${taskName}/${childName}`
  const newSet = new Set(checkedTasks.value)
  if (newSet.has(key)) {
    newSet.delete(key)
  } else {
    newSet.add(key)
  }
  checkedTasks.value = newSet
}

// ─── Tab 2 操作 ───

function removeTask(index: number) {
  editedTasks.value.splice(index, 1)
  delete newChildNames.value[index]
}

function removeChild(taskIndex: number, childIndex: number) {
  editedTasks.value[taskIndex].children.splice(childIndex, 1)
}

function addTask() {
  const name = newTaskName.value.trim()
  if (!name) return
  editedTasks.value.push({ name, children: [] })
  newTaskName.value = ''
}

function addChild(taskIndex: number) {
  const name = (newChildNames.value[taskIndex] || '').trim()
  if (!name) return
  editedTasks.value[taskIndex].children.push({ name })
  newChildNames.value[taskIndex] = ''
}

// ─── Tab 3 操作 ───

const groupedArchives = computed(() => {
  const groups: { taskName: string; versions: ArchivedVersion[] }[] = []
  let currentGroup: { taskName: string; versions: ArchivedVersion[] } | null = null
  for (const v of archivedVersions.value) {
    if (!currentGroup || currentGroup.taskName !== v.task_name) {
      currentGroup = { taskName: v.task_name, versions: [] }
      groups.push(currentGroup)
    }
    currentGroup.versions.push(v)
  }
  return groups
})

async function loadArchivedTasks() {
  archiveLoading.value = true
  try {
    archivedVersions.value = await invoke<ArchivedVersion[]>('list_archived_tasks', {
      projectPath,
    })
  } catch (e) {
    console.error('加载归档列表失败:', e)
  } finally {
    archiveLoading.value = false
  }
}

async function restoreArchive(version: ArchivedVersion) {
  try {
    await invoke('restore_archived_task', {
      projectPath,
      taskName: version.task_name,
      timestamp: version.timestamp,
    })
    await loadArchivedTasks()
  } catch (e: any) {
    showAlert(t('taskList.restoreFailed'), typeof e === 'string' ? e : e.message || t('taskList.restoreFailed'))
  }
}

function deleteArchive(version: ArchivedVersion) {
  showConfirm(
    t('taskList.deleteArchive'),
    t('taskList.confirmDeleteArchive', { taskName: version.task_name, version: version.display_time }),
    async () => {
      try {
        await invoke('delete_archived_version', {
          projectPath,
          taskName: version.task_name,
          timestamp: version.timestamp,
        })
        await loadArchivedTasks()
      } catch (e) {
        console.error('删除归档版本失败:', e)
      }
    },
  )
}

// ─── 确定 / 取消 ───

async function handleConfirm() {
  if (activeTab.value === 'archive') {
    router.push({ name: 'project', params: { projectId } })
    return
  }

  if (activeTab.value === 'enable') {
    if (!hasEnableChanges.value) {
      router.push({ name: 'project', params: { projectId } })
      return
    }
    saving.value = true
    try {
      const result = await invoke<ApplyTaskResult>('apply_task_changes', {
        projectPath,
        enabledTasks: Array.from(checkedTasks.value),
      })
      if (result.errors.length > 0) {
        console.warn('任务变更部分失败:', result.errors)
      }
      router.push({ name: 'project', params: { projectId } })
    } catch (e) {
      console.error('应用任务变更失败:', e)
    } finally {
      saving.value = false
    }
  } else {
    if (!hasTemplateChanges.value) {
      router.push({ name: 'project', params: { projectId } })
      return
    }
    saving.value = true
    try {
      const s = await loadSettings()
      await invoke('save_global_tasks', {
        rootDir: s?.general.projectRootDir ?? '',
        config: { tasks: editedTasks.value },
      })
      globalTasks.value = JSON.parse(JSON.stringify(editedTasks.value))
      router.push({ name: 'project', params: { projectId } })
    } catch (e) {
      console.error('保存全局任务模板失败:', e)
    } finally {
      saving.value = false
    }
  }
}

function handleCancel() {
  router.push({ name: 'project', params: { projectId } })
}
</script>

<template>
  <div class="task-list-page">
    <!-- Tab 导航栏 -->
    <div class="page-header">
      <div class="tab-bar">
        <button
          class="tab-btn"
          :class="{ active: activeTab === 'enable' }"
          @click="activeTab = 'enable'"
        >
          {{ $t('taskList.enableTab') }}
        </button>
        <button
          class="tab-btn"
          :class="{ active: activeTab === 'edit' }"
          @click="activeTab = 'edit'"
        >
          {{ $t('taskList.templateTab') }}
        </button>
        <button
          class="tab-btn"
          :class="{ active: activeTab === 'archive' }"
          @click="activeTab = 'archive'"
        >
          {{ $t('taskList.timeMachine') }}
        </button>
      </div>
    </div>

    <!-- 加载中 -->
    <div v-if="loading" class="page-body">
      <p class="hint-text">{{ $t('common.loading') }}</p>
    </div>

    <!-- Tab 1: 任务启用 -->
    <div v-show="!loading && activeTab === 'enable'" class="page-body">
      <template v-for="task in globalTasks" :key="task.name">
        <label class="task-row" @click.prevent="toggleTask(task.name)">
          <span class="task-checkbox" :class="{ checked: checkedTasks.has(task.name) }" />
          <span class="task-name">{{ task.name }}</span>
        </label>
        <label
          v-for="child in task.children"
          :key="`${task.name}-${child.name}`"
          class="task-row task-child"
          @click.prevent="toggleChild(task.name, child.name)"
        >
          <span
            class="task-checkbox task-checkbox-child"
            :class="{ checked: checkedTasks.has(`${task.name}/${child.name}`) }"
          />
          <span class="task-name child-name">{{ child.name }}</span>
        </label>
      </template>
    </div>

    <!-- Tab 2: 模板编辑 -->
    <div v-show="!loading && activeTab === 'edit'" class="page-body">
      <template v-for="(task, tIdx) in editedTasks" :key="tIdx">
        <div class="edit-row">
          <input v-model="task.name" class="edit-input" :placeholder="$t('taskList.taskNamePlaceholder')" />
          <button class="edit-delete-btn" :title="$t('common.delete')" @click="removeTask(tIdx)">&times;</button>
        </div>
        <div
          v-for="(child, cIdx) in task.children"
          :key="`${tIdx}-${cIdx}`"
          class="edit-row edit-child-row"
        >
          <input v-model="child.name" class="edit-input edit-child-input" :placeholder="$t('taskList.subtaskNamePlaceholder')" />
          <button class="edit-delete-btn" :title="$t('common.delete')" @click="removeChild(tIdx, cIdx)">&times;</button>
        </div>
        <div class="edit-row edit-child-row edit-add-row">
          <input
            v-model="newChildNames[tIdx]"
            class="edit-input edit-child-input"
            :placeholder="$t('taskList.addSubtaskPlaceholder')"
            @keydown.enter="addChild(tIdx)"
          />
          <button
            class="edit-add-btn"
            :disabled="!(newChildNames[tIdx] || '').trim()"
            @click="addChild(tIdx)"
          >
            +
          </button>
        </div>
      </template>
      <div class="edit-row edit-add-task-row">
        <input
          v-model="newTaskName"
          class="edit-input"
          :placeholder="$t('taskList.addTaskPlaceholder')"
          @keydown.enter="addTask"
        />
        <button class="edit-add-btn" :disabled="!newTaskName.trim()" @click="addTask">+</button>
      </div>
    </div>

    <!-- Tab 3: 时光机 -->
    <div v-show="!loading && activeTab === 'archive'" class="page-body">
      <div v-if="archiveLoading" class="hint-text">{{ $t('common.loading') }}</div>
      <div v-else-if="groupedArchives.length === 0" class="hint-text">{{ $t('taskList.noArchived') }}</div>
      <template v-else>
        <div v-for="group in groupedArchives" :key="group.taskName" class="archive-group">
          <p class="archive-task-name">{{ group.taskName }}</p>
          <div
            v-for="ver in group.versions"
            :key="ver.timestamp"
            class="archive-version-row"
          >
            <span class="archive-time">{{ ver.display_time }}</span>
            <div class="archive-actions">
              <button class="archive-btn archive-restore-btn" @click="restoreArchive(ver)">{{ $t('taskList.restore') }}</button>
              <button class="archive-btn archive-delete-btn" @click="deleteArchive(ver)">{{ $t('common.delete') }}</button>
            </div>
          </div>
        </div>
      </template>
    </div>

    <!-- 底部操作栏 -->
    <div class="page-footer">
      <button class="action-btn action-btn-primary" :disabled="saving" @click="handleConfirm">
        {{ saving ? $t('common.processing') : activeTab === 'archive' ? $t('common.close') : $t('common.ok') }}
      </button>
      <button class="action-btn action-btn-secondary" :disabled="saving" @click="handleCancel">
        {{ $t('common.cancel') }}
      </button>
    </div>

    <!-- 内部确认/提示弹窗 -->
    <Teleport to="body">
      <div v-if="innerDialog.visible" class="inner-dialog-overlay">
        <div class="inner-dialog glass-strong">
          <p class="inner-dialog-title">{{ innerDialog.title }}</p>
          <p class="inner-dialog-message">{{ innerDialog.message }}</p>
          <div class="inner-dialog-actions">
            <button
              v-if="innerDialog.type === 'confirm'"
              class="action-btn action-btn-primary"
              @click="handleInnerConfirm"
            >
              {{ $t('common.ok') }}
            </button>
            <button class="action-btn action-btn-secondary" @click="handleInnerCancel">
              {{ innerDialog.type === 'alert' ? $t('common.gotIt') : $t('common.cancel') }}
            </button>
          </div>
        </div>
      </div>
    </Teleport>
    <PageGuideOverlay :show="showGuide" :annotations="PAGE_GUIDE_ANNOTATIONS.taskList" @close="showGuide = false" />
  </div>
</template>

<style scoped>
.task-list-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

/* ─── Tab 栏 ─── */

.page-header {
  flex-shrink: 0;
  padding: 0 var(--spacing-6);
  border-bottom: 1px solid var(--border-subtle);
}

.tab-bar {
  display: flex;
  gap: var(--spacing-1);
}

.tab-btn {
  padding: var(--spacing-3) var(--spacing-5);
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  color: var(--text-secondary);
  background: transparent;
  border: none;
  border-bottom: 2px solid transparent;
  cursor: pointer;
  transition: all var(--transition-fast);
  font-family: inherit;
}

.tab-btn:hover {
  color: var(--text-primary);
}

.tab-btn.active {
  color: var(--text-primary);
  border-bottom-color: var(--color-blue-500);
}

/* ─── 内容区 ─── */

.page-body {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-4) var(--spacing-6);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.hint-text {
  font-size: var(--text-base);
  color: var(--text-secondary);
}

/* ─── Tab 1: 任务启用 ─── */

.task-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: background var(--transition-fast);
  user-select: none;
}

.task-row:hover {
  background: var(--bg-hover);
}

.task-child {
  padding-left: var(--spacing-8);
}

.task-checkbox {
  width: 18px;
  height: 18px;
  min-width: 18px;
  border-radius: var(--radius-sm);
  border: 2px solid var(--border-medium);
  transition: all var(--transition-fast);
  position: relative;
}

.task-checkbox.checked {
  background: color-mix(in srgb, var(--color-primary-500) 75%, transparent);
  border-color: color-mix(in srgb, var(--color-primary-500) 75%, transparent);
  backdrop-filter: blur(var(--glass-light-blur));
  -webkit-backdrop-filter: blur(var(--glass-light-blur));
}

.task-checkbox.checked::after {
  content: '';
  position: absolute;
  top: 2px;
  left: 5px;
  width: 4px;
  height: 8px;
  border: solid var(--color-neutral-0);
  border-width: 0 2px 2px 0;
  transform: rotate(45deg);
}

.task-checkbox-child {
  width: 14px;
  height: 14px;
  min-width: 14px;
}

.task-checkbox-child.checked::after {
  top: 1px;
  left: 3px;
  width: 4px;
  height: 7px;
}

.task-name {
  font-size: var(--text-base);
  color: var(--text-primary);
}

.child-name {
  color: var(--text-secondary);
}

/* ─── Tab 2: 模板编辑 ─── */

.edit-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-1) var(--spacing-3);
}

.edit-child-row {
  padding-left: var(--spacing-8);
}

.edit-add-row {
  opacity: 0.7;
}

.edit-add-row:focus-within {
  opacity: 1;
}

.edit-input {
  flex: 1;
  min-width: 0;
  height: var(--button-sm-height);
  padding: 0 var(--spacing-2);
  font-size: var(--text-base);
  font-family: inherit;
  color: var(--text-primary);
  background: var(--bg-secondary);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  outline: none;
  transition: border-color var(--transition-fast);
}

.edit-input:focus {
  border-color: var(--color-blue-500);
}

.edit-input::placeholder {
  color: var(--text-tertiary);
}

.edit-child-input {
  font-size: var(--text-sm);
}

.edit-delete-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  min-width: 24px;
  font-size: var(--text-lg);
  line-height: 1;
  color: var(--text-tertiary);
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.edit-delete-btn:hover {
  color: var(--color-red-500);
  background: var(--bg-hover);
}

.edit-add-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  min-width: 24px;
  font-size: var(--text-lg);
  line-height: 1;
  color: var(--color-blue-500);
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.edit-add-btn:hover:not(:disabled) {
  background: var(--bg-hover);
}

.edit-add-btn:disabled {
  color: var(--text-tertiary);
  cursor: not-allowed;
}

.edit-add-task-row {
  margin-top: var(--spacing-3);
  padding-top: var(--spacing-3);
  border-top: 1px solid var(--border-subtle);
}

/* ─── Tab 3: 时光机 ─── */

.archive-group {
  margin-bottom: var(--spacing-3);
}

.archive-group:last-child {
  margin-bottom: 0;
}

.archive-task-name {
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
  padding: var(--spacing-1) var(--spacing-3);
  margin: 0;
}

.archive-version-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-2) var(--spacing-3);
  padding-left: var(--spacing-8);
  border-radius: var(--radius-md);
  transition: background var(--transition-fast);
}

.archive-version-row:hover {
  background: var(--bg-hover);
}

.archive-time {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.archive-actions {
  display: flex;
  gap: var(--spacing-2);
}

.archive-btn {
  display: inline-flex;
  align-items: center;
  height: 26px;
  padding: 0 var(--spacing-3);
  font-size: var(--text-xs);
  font-weight: var(--font-weight-heading);
  font-family: inherit;
  border-radius: var(--radius-sm);
  border: none;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.archive-restore-btn {
  color: var(--color-blue-500);
  background: transparent;
  border: 1px solid var(--color-blue-500);
}

.archive-restore-btn:hover {
  background: color-mix(in srgb, var(--color-primary-500) 15%, transparent);
}

.archive-delete-btn {
  color: var(--text-tertiary);
  background: transparent;
  border: 1px solid var(--border-subtle);
}

.archive-delete-btn:hover {
  color: var(--color-red-500);
  border-color: var(--color-red-500);
  background: color-mix(in srgb, var(--color-danger) 10%, transparent);
}

/* ─── 底部操作栏 ─── */

.page-footer {
  flex-shrink: 0;
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-3);
  padding: var(--spacing-4) var(--spacing-6);
  border-top: 1px solid var(--border-subtle);
}

.action-btn {
  display: inline-flex;
  align-items: center;
  height: var(--button-md-height);
  padding: 0 var(--spacing-5);
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  font-family: inherit;
  border-radius: var(--radius-md);
  border: none;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.action-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.action-btn-primary {
  background: color-mix(in srgb, var(--color-primary-500) 75%, transparent);
  backdrop-filter: blur(var(--glass-subtle-blur));
  -webkit-backdrop-filter: blur(var(--glass-subtle-blur));
  color: var(--color-neutral-0);
}

.action-btn-primary:hover:not(:disabled) {
  background: color-mix(in srgb, var(--color-primary-500) 90%, transparent);
}

.action-btn-secondary {
  background: transparent;
  color: var(--text-secondary);
  border: 1px solid var(--border-medium);
}

.action-btn-secondary:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}

/* ─── 内部确认/提示弹窗 ─── */

.inner-dialog-overlay {
  position: fixed;
  inset: 0;
  z-index: calc(var(--z-modal-backdrop) + 10);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-backdrop);
  backdrop-filter: blur(var(--glass-light-blur));
}

.inner-dialog {
  min-width: 300px;
  max-width: 300px;
  border-radius: var(--floating-navbar-radius);
  padding: var(--spacing-6);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
}

.inner-dialog-title {
  font-size: var(--text-lg);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
  margin: 0;
}

.inner-dialog-message {
  font-size: var(--text-base);
  color: var(--text-secondary);
  line-height: 1.5;
  margin: 0;
}

.inner-dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-3);
}
</style>
