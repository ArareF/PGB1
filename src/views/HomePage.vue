<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useNavigation } from '../composables/useNavigation'
import { APP_NAME } from '../config/app'
import { useProjects } from '../composables/useProjects'
import { useSettings } from '../composables/useSettings'
import { useDirectoryFiles } from '../composables/useDirectoryFiles'
import type { ProjectInfo } from '../composables/useProjects'
import ProjectCard from '../components/ProjectCard.vue'
import CreateProjectDialog from '../components/CreateProjectDialog.vue'
import EditProjectDialog from '../components/EditProjectDialog.vue'

const router = useRouter()
const { setNavigation } = useNavigation()
const { projects, loading, loadProjects } = useProjects()
const { loadSettings } = useSettings()
const { openInExplorer } = useDirectoryFiles()

const showCreateDialog = ref(false)
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

/* 注册主页导航配置 */
setNavigation({
  title: APP_NAME,
  showBackButton: false,
  actions: [],
  moreMenuItems: [
    { id: 'open-root', label: '打开项目文件夹', handler: async () => {
      const s = await loadSettings()
      if (s?.general.projectRootDir) openInExplorer(s.general.projectRootDir)
    }},
    { id: 'settings', label: '程序设置', handler: () => router.push({ name: 'settings' }) },
  ],
})

onMounted(() => {
  loadProjects()
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
      <p class="page-hint">我的项目</p>
      <button class="add-btn" title="新建项目" @click="showCreateDialog = true">+</button>
    </div>

    <!-- 可滚动内容区 -->
    <div class="scroll-content">
      <p v-if="loading" class="loading-text">扫描中...</p>

      <TransitionGroup v-else name="card" tag="div" class="card-grid">
        <ProjectCard
          v-for="(project, i) in projects"
          :key="project.name"
          :style="{ '--delay': i * 40 + 'ms' }"
          :project="project"
          @click="openProject"
          @action="onProjectAction"
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
</style>
