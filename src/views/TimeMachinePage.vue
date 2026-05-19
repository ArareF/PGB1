<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import { useNavigation } from '../composables/useNavigation'
import { useArchivedMaterials } from '../composables/useArchivedMaterials'
import { formatSize } from '../utils/format'
import type { ArchivedVersion, ArchivedMaterialVersion } from '../types/task'

type TabType = 'task' | 'material'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const { setNavigation } = useNavigation()

const projectId = route.params.projectId as string
const projectPath = ref((route.query.projectPath as string) ?? '')

const activeTab = ref<TabType>('task')

// ─── 任务归档 ───
const archivedTasks = ref<ArchivedVersion[]>([])
const taskLoading = ref(false)

async function loadArchivedTasks() {
  if (!projectPath.value) return
  taskLoading.value = true
  try {
    archivedTasks.value = await invoke<ArchivedVersion[]>('list_archived_tasks', {
      projectPath: projectPath.value,
    })
  } catch (e) {
    console.error('[TimeMachinePage] 加载任务归档失败:', e)
  } finally {
    taskLoading.value = false
  }
}

const groupedTaskArchives = computed(() => {
  const groups: { taskName: string; versions: ArchivedVersion[] }[] = []
  let current: { taskName: string; versions: ArchivedVersion[] } | null = null
  for (const v of archivedTasks.value) {
    if (!current || current.taskName !== v.task_name) {
      current = { taskName: v.task_name, versions: [] }
      groups.push(current)
    }
    current.versions.push(v)
  }
  return groups
})

// ─── 素材归档 ───
const materialArchive = useArchivedMaterials(() => projectPath.value)

const groupedMaterialArchives = computed(() => {
  // 按 task_name → base_name 分组
  const map = new Map<string, Map<string, ArchivedMaterialVersion[]>>()
  for (const v of materialArchive.versions.value) {
    if (!map.has(v.task_name)) map.set(v.task_name, new Map())
    const baseMap = map.get(v.task_name)!
    if (!baseMap.has(v.base_name)) baseMap.set(v.base_name, [])
    baseMap.get(v.base_name)!.push(v)
  }
  return Array.from(map.entries()).map(([taskName, baseMap]) => ({
    taskName,
    materials: Array.from(baseMap.entries()).map(([baseName, versions]) => ({ baseName, versions })),
  }))
})

// ─── 提示/确认弹窗 ───
interface InnerDialog {
  visible: boolean
  title: string
  message: string
  type: 'confirm' | 'alert'
  onConfirm: (() => void) | null
}

const innerDialog = ref<InnerDialog>({
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

// ─── 操作 ───

async function restoreTask(version: ArchivedVersion) {
  try {
    await invoke('restore_archived_task', {
      projectPath: projectPath.value,
      taskName: version.task_name,
      timestamp: version.timestamp,
    })
    await loadArchivedTasks()
  } catch (e: any) {
    showAlert(t('taskList.restoreFailed'), typeof e === 'string' ? e : e.message || t('taskList.restoreFailed'))
  }
}

function deleteTaskArchive(version: ArchivedVersion) {
  showConfirm(
    t('taskList.deleteArchive'),
    t('taskList.confirmDeleteArchive', { taskName: version.task_name, version: version.display_time }),
    async () => {
      try {
        await invoke('delete_archived_version', {
          projectPath: projectPath.value,
          taskName: version.task_name,
          timestamp: version.timestamp,
        })
        await loadArchivedTasks()
      } catch (e) {
        console.error('[TimeMachinePage] 删除任务归档失败:', e)
      }
    },
  )
}

async function restoreMaterial(version: ArchivedMaterialVersion) {
  try {
    await materialArchive.restore(version)
    await materialArchive.load()
  } catch (e: any) {
    showAlert(
      t('timeMachine.restoreMaterialFailed'),
      typeof e === 'string' ? e : e.message || t('timeMachine.restoreMaterialFailed'),
    )
  }
}

function deleteMaterialArchive(version: ArchivedMaterialVersion) {
  showConfirm(
    t('timeMachine.deleteMaterialArchive'),
    t('timeMachine.confirmDeleteMaterialArchive', {
      baseName: version.base_name,
      version: version.display_time,
    }),
    async () => {
      try {
        await materialArchive.remove(version)
        await materialArchive.load()
      } catch (e) {
        console.error('[TimeMachinePage] 删除素材归档失败:', e)
      }
    },
  )
}

onMounted(async () => {
  setNavigation({
    title: `${t('timeMachine.title')} · ${projectId}`,
    showBackButton: true,
    onBack: () => router.push({ name: 'project', params: { projectId } }),
    actions: [],
    moreMenuItems: [],
  })
  await Promise.all([loadArchivedTasks(), materialArchive.load()])
})
</script>

<template>
  <div class="time-machine-page">
    <div class="page-header">
      <div class="tab-bar">
        <button
          class="tab-btn"
          :class="{ active: activeTab === 'task' }"
          @click="activeTab = 'task'"
        >
          {{ $t('timeMachine.taskTab') }}
        </button>
        <button
          class="tab-btn"
          :class="{ active: activeTab === 'material' }"
          @click="activeTab = 'material'"
        >
          {{ $t('timeMachine.materialTab') }}
        </button>
      </div>
      <p class="retention-hint">{{ $t('timeMachine.retentionHint') }}</p>
    </div>

    <!-- 任务归档 Tab -->
    <div v-show="activeTab === 'task'" class="page-body">
      <p v-if="taskLoading" class="hint-text">{{ $t('common.loading') }}</p>
      <p v-else-if="groupedTaskArchives.length === 0" class="hint-text">{{ $t('taskList.noArchived') }}</p>
      <template v-else>
        <div v-for="group in groupedTaskArchives" :key="group.taskName" class="archive-group">
          <p class="archive-task-name">{{ group.taskName }}</p>
          <div
            v-for="ver in group.versions"
            :key="ver.timestamp"
            class="archive-version-row"
          >
            <span class="archive-time">{{ ver.display_time }}</span>
            <div class="archive-actions">
              <button class="archive-btn archive-restore-btn" @click="restoreTask(ver)">{{ $t('taskList.restore') }}</button>
              <button class="archive-btn archive-delete-btn" @click="deleteTaskArchive(ver)">{{ $t('common.delete') }}</button>
            </div>
          </div>
        </div>
      </template>
    </div>

    <!-- 素材归档 Tab -->
    <div v-show="activeTab === 'material'" class="page-body">
      <p v-if="materialArchive.loading.value" class="hint-text">{{ $t('common.loading') }}</p>
      <p v-else-if="groupedMaterialArchives.length === 0" class="hint-text">{{ $t('timeMachine.noArchivedMaterials') }}</p>
      <template v-else>
        <div v-for="group in groupedMaterialArchives" :key="group.taskName" class="archive-group">
          <p class="archive-task-name">{{ group.taskName }}</p>
          <div v-for="mat in group.materials" :key="mat.baseName" class="material-subgroup">
            <p class="material-base-name">{{ mat.baseName }}</p>
            <div
              v-for="ver in mat.versions"
              :key="ver.timestamp"
              class="archive-version-row material-version-row"
            >
              <div class="material-ver-meta">
                <span class="archive-time">{{ ver.display_time }}</span>
                <span class="material-ver-size">{{ formatSize(ver.size_bytes) }}</span>
                <span v-if="ver.stages.length" class="material-ver-stages" :title="ver.stages.join('\n')">
                  {{ $t('timeMachine.stagesCount', { n: ver.stages.length }) }}
                </span>
              </div>
              <div class="archive-actions">
                <button class="archive-btn archive-restore-btn" @click="restoreMaterial(ver)">{{ $t('taskList.restore') }}</button>
                <button class="archive-btn archive-delete-btn" @click="deleteMaterialArchive(ver)">{{ $t('common.delete') }}</button>
              </div>
            </div>
          </div>
        </div>
      </template>
    </div>

    <div class="page-footer">
      <button class="action-btn action-btn-primary" @click="router.push({ name: 'project', params: { projectId } })">
        {{ $t('common.close') }}
      </button>
    </div>

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
  </div>
</template>

<style scoped>
.time-machine-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.page-header {
  flex-shrink: 0;
  padding: 0 var(--spacing-6);
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  gap: var(--spacing-4);
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

.retention-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  margin: 0;
}

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

.archive-group {
  margin-bottom: var(--spacing-4);
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

.material-subgroup {
  margin-left: var(--spacing-4);
  margin-bottom: var(--spacing-2);
}

.material-base-name {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  padding: var(--spacing-1) var(--spacing-3);
  margin: 0;
  font-weight: var(--font-weight-heading);
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

.material-version-row {
  padding-left: var(--spacing-10);
}

.archive-time {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.material-ver-meta {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.material-ver-size {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  font-variant-numeric: tabular-nums;
}

.material-ver-stages {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  cursor: help;
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
  min-width: 320px;
  max-width: 420px;
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
  white-space: pre-wrap;
}

.inner-dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-3);
}
</style>
