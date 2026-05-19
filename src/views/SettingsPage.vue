<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useNavigation } from '../composables/useNavigation'
import { useSettings } from '../composables/useSettings'
import type { AppSettings } from '../composables/useSettings'
import PageGuideOverlay from '../components/PageGuideOverlay.vue'
import { PAGE_GUIDE_ANNOTATIONS } from '../config/onboarding'

import WorkflowSettings from './settings/WorkflowSettings.vue'
import TranslationSettings from './settings/TranslationSettings.vue'
import AttendanceSettings from './settings/AttendanceSettings.vue'
import GeneralSettings from './settings/GeneralSettings.vue'
import AboutSettings from './settings/AboutSettings.vue'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const { setNavigation } = useNavigation()
const { loadSettings, saveSettings } = useSettings()

type TabId = 'workflow' | 'translation' | 'attendance' | 'general' | 'about'
const activeTab = ref<TabId>('workflow')

/** 本地编辑中的 AppSettings 副本（workflow/translation/general 三 Tab 共享） */
const editSettings = ref<AppSettings | null>(null)
const isDirty = ref(false)
const saving = ref(false)
const saveError = ref('')

/** 传给 v-model 子组件的 writable computed（v-if 保证非 null） */
const settingsModel = computed<AppSettings>({
  get: () => editSettings.value as AppSettings,
  set: (v) => { editSettings.value = v },
})

/** AttendanceSettings 子组件 ref（通过 defineExpose 拿 save / isDirty / isSaving / saved） */
const attendanceRef = ref<InstanceType<typeof AttendanceSettings> | null>(null)

/** 引导浮层 */
const showGuide = ref(false)
const guideIsAttendance = ref(false)

async function init() {
  const current = await loadSettings()
  if (current) {
    editSettings.value = JSON.parse(JSON.stringify(current))
  }

  // 从路由 query 读取初始 tab（更多菜单跳转时传入）
  if (route.query.tab === 'attendance') {
    activeTab.value = 'attendance'
  }

  // 新手引导完成后跳转过来，自动弹出出勤配置指引
  if (route.query.guide === 'attendance') {
    guideIsAttendance.value = true
    showGuide.value = true
  }

  setNavigation({
    title: t('settings.title'),
    showBackButton: true,
    onBack: () => router.back(),
    moreMenuItems: [
      { id: 'page-guide', label: t('common.pageGuide'), handler: () => { showGuide.value = true } },
    ],
  })
}

onMounted(() => {
  init()
})

/** 监听 AppSettings 深度变化标记为脏 */
watch(editSettings, () => {
  isDirty.value = true
}, { deep: true })

/** 语言切换后重新设置 navigation 标题（让 t() 用新 locale 重新求值） */
watch(
  () => editSettings.value?.general.language,
  (newLang, oldLang) => {
    if (newLang && oldLang && newLang !== oldLang) {
      setNavigation({
        title: t('settings.title'),
        showBackButton: true,
        onBack: () => router.back(),
        moreMenuItems: [
          { id: 'page-guide', label: t('common.pageGuide'), handler: () => { showGuide.value = true } },
        ],
      })
    }
  },
)

async function handleSave() {
  if (!editSettings.value) return
  saving.value = true
  saveError.value = ''
  try {
    await saveSettings(editSettings.value)
    isDirty.value = false
  } catch (e) {
    saveError.value = String(e)
    console.error('保存设置失败:', e)
  } finally {
    saving.value = false
  }
}

async function handleAttendanceSave() {
  if (attendanceRef.value) {
    await attendanceRef.value.save()
  }
}
</script>

<template>
  <div class="settings-page">
    <div v-if="editSettings" class="settings-container">
      <!-- 左侧 Tab 导航 -->
      <aside class="settings-sidebar">
        <button class="tab-btn" :class="{ active: activeTab === 'workflow' }" @click="activeTab = 'workflow'">
          {{ $t('settings.workflowTab') }}
        </button>
        <button class="tab-btn" :class="{ active: activeTab === 'translation' }" @click="activeTab = 'translation'">
          {{ $t('settings.translationTab') }}
        </button>
        <button class="tab-btn" :class="{ active: activeTab === 'attendance' }" @click="activeTab = 'attendance'">
          {{ $t('settings.attendanceTab') }}
        </button>
        <button class="tab-btn" :class="{ active: activeTab === 'general' }" @click="activeTab = 'general'">
          {{ $t('settings.generalTab') }}
        </button>
        <button class="tab-btn" :class="{ active: activeTab === 'about' }" @click="activeTab = 'about'">
          {{ $t('settings.aboutTab') }}
        </button>

        <div class="sidebar-footer">
          <!-- 日报打卡 Tab 有独立保存按钮；关于 Tab 无按钮；其他 Tab 共用保存按钮 -->
          <p v-if="saveError && activeTab !== 'attendance' && activeTab !== 'about'" class="error-text">{{ saveError }}</p>
          <button
            v-if="activeTab !== 'attendance' && activeTab !== 'about'"
            class="save-btn"
            :disabled="!isDirty || saving"
            @click="handleSave"
          >
            {{ saving ? $t('common.saving') : $t('common.saveChanges') }}
          </button>
          <button
            v-else-if="activeTab === 'attendance'"
            class="save-btn"
            :class="{ 'save-btn-success': attendanceRef?.saved }"
            :disabled="!attendanceRef?.isDirty || attendanceRef?.isSaving"
            @click="handleAttendanceSave"
          >
            {{ attendanceRef?.isSaving ? $t('common.saving') : (attendanceRef?.saved ? $t('common.saved') : $t('common.saveChanges')) }}
          </button>
        </div>
      </aside>

      <!-- 右侧内容区：5 个子组件全部常驻（v-show 切换）
           避免 v-if 卸载导致 AttendanceSettings 未保存字段丢失、
           且规避首次切 Tab 时 load_attendance_config 的冷启动延迟。 -->
      <main class="settings-content">
        <WorkflowSettings v-show="activeTab === 'workflow'" v-model="settingsModel" />
        <TranslationSettings v-show="activeTab === 'translation'" v-model="settingsModel" />
        <AttendanceSettings v-show="activeTab === 'attendance'" ref="attendanceRef" />
        <GeneralSettings v-show="activeTab === 'general'" v-model="settingsModel" @persisted="isDirty = false" />
        <AboutSettings v-show="activeTab === 'about'" />
      </main>
    </div>
    <div v-else class="loading-state">
      {{ $t('common.loading') }}
    </div>
  </div>

  <PageGuideOverlay
    :show="showGuide"
    :annotations="guideIsAttendance ? PAGE_GUIDE_ANNOTATIONS.settingsAttendance : PAGE_GUIDE_ANNOTATIONS.settings"
    @close="showGuide = false; guideIsAttendance = false"
  />
</template>

<style scoped>
/* =============================================
   父组件：页面结构样式（sidebar / tab / save btn）
   ============================================= */
.settings-page {
  height: 100%;
  overflow: hidden;
}

.settings-container {
  display: flex;
  height: 100%;
}

.settings-sidebar {
  width: 200px;
  border-right: 1px solid var(--border-medium);
  padding: var(--spacing-4);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.tab-btn {
  height: 40px;
  padding: 0 var(--spacing-4);
  border-radius: var(--radius-md);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  text-align: left;
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.tab-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.tab-btn.active {
  background: var(--color-primary-500);
  color: var(--text-inverse);
}

.sidebar-footer {
  margin-top: auto;
  padding-top: var(--spacing-4);
}

.save-btn {
  width: 100%;
  height: 40px;
  border-radius: var(--radius-md);
  border: none;
  background: var(--color-success);
  color: var(--text-inverse);
  font-weight: var(--font-bold);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.save-btn:hover:not(:disabled) {
  background: var(--color-success-dark);
}

.save-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  background: var(--color-neutral-400);
}

/* 保存成功短暂高亮 */
.save-btn-success {
  background: var(--color-success) !important;
}

.settings-content {
  flex: 1;
  padding: var(--spacing-8);
  overflow-y: auto;
}

.loading-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-tertiary);
}

/* =============================================
   穿透到子组件：共享表单样式
   --------------------------------------------
   Workflow / Translation / Attendance / General /
   About 五个子组件共享同一套 form 样式，用 :deep()
   穿透 scoped 边界避免五次重复。
   ============================================= */
:deep(.settings-section) {
  max-width: 600px;
  display: flex;
  flex-direction: column;
  gap: var(--spacing-6);
}

:deep(.section-title) {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-primary);
  margin-bottom: var(--spacing-2);
}

:deep(.form-group) {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

:deep(.form-label) {
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  color: var(--text-secondary);
}

:deep(.form-input),
:deep(.form-select) {
  height: 40px;
  padding: 0 var(--spacing-3);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-heavy);
  background: var(--bg-tertiary);
  color: var(--text-primary);
  font-size: var(--text-sm);
}

:deep(.form-input:focus) {
  outline: none;
  border-color: var(--color-primary-500);
}

:deep(.form-input-time) {
  width: 160px;
}

:deep(.form-hint) {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

:deep(.path-input-group) {
  display: flex;
  gap: var(--spacing-2);
}

:deep(.path-input-group .form-input) {
  flex: 1;
}

:deep(.browse-btn) {
  height: 40px;
  padding: 0 var(--spacing-4);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: var(--bg-secondary);
  color: var(--text-primary);
  cursor: pointer;
  white-space: nowrap;
}

:deep(.browse-btn:hover) {
  background: var(--bg-hover);
}

:deep(.theme-toggle-row) {
  display: flex;
  align-items: center;
  gap: var(--spacing-4);
}

:deep(.theme-current) {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

:deep(.checkbox-label) {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  cursor: pointer;
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
}

.error-text,
:deep(.error-text) {
  font-size: var(--text-sm);
  color: var(--color-danger);
}

:deep(.form-group-disabled) {
  opacity: 0.35;
  pointer-events: none;
}

/* ─── translation 独占 ─── */
:deep(.lang-pair-row) {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}
:deep(.lang-select) {
  flex: 1;
}
:deep(.lang-separator) {
  color: var(--text-secondary);
  font-size: var(--text-lg);
  flex-shrink: 0;
}

/* ─── general 独占 ─── */
:deep(.fps-input-row) {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

:deep(.fps-input) {
  width: 100px;
}

:deep(.fps-unit) {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

/* ─── about 独占 ─── */
:deep(.about-section) {
  justify-content: flex-start;
}

:deep(.about-card) {
  display: flex;
  flex-direction: column;
  gap: 0;
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

:deep(.about-row) {
  display: flex;
  align-items: center;
  padding: var(--spacing-4) var(--spacing-6);
  border-bottom: 1px solid var(--border-medium);
}

:deep(.about-row:last-child) {
  border-bottom: none;
}

:deep(.about-label) {
  width: 100px;
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  color: var(--text-secondary);
  flex-shrink: 0;
}

:deep(.about-value) {
  font-size: var(--text-sm);
  color: var(--text-primary);
}

:deep(.check-update-btn) {
  margin-top: var(--spacing-4);
  padding: var(--spacing-2) var(--spacing-5);
  border-radius: var(--radius-button);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-family: inherit;
  cursor: pointer;
  transition: all var(--duration-fast);
}
:deep(.check-update-btn:hover:not(:disabled)) {
  background: var(--bg-hover);
  color: var(--text-primary);
  border-color: var(--color-primary);
}
:deep(.check-update-btn:disabled) {
  opacity: 0.6;
  cursor: default;
}

/* ─── attendance 独占 ─── */
:deep(.attendance-group) {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
  padding-bottom: var(--spacing-4);
  border-bottom: 1px solid var(--border-medium);
}

:deep(.attendance-group:last-of-type) {
  border-bottom: none;
}

:deep(.attendance-group-title) {
  font-size: var(--text-base);
  font-weight: var(--font-bold);
  color: var(--text-primary);
}

:deep(.test-clock-btn) {
  height: 40px;
  padding: 0 var(--spacing-5);
  border-radius: var(--radius-md);
  border: 1px solid var(--color-primary-500);
  background: transparent;
  color: var(--color-primary-500);
  font-weight: var(--font-bold);
  cursor: pointer;
  transition: all var(--duration-fast);
  align-self: flex-start;
}

:deep(.test-clock-btn:hover:not(:disabled)) {
  background: var(--color-primary-500);
  color: var(--text-inverse);
}

:deep(.test-clock-btn:disabled) {
  opacity: 0.4;
  cursor: not-allowed;
}

:deep(.test-progress) {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-sm);
  background: var(--bg-tertiary);
}

:deep(.test-progress.test-success) {
  color: var(--color-success);
}

:deep(.test-progress.test-error) {
  color: var(--color-danger);
}

:deep(.mode-btn-group) {
  display: flex;
  gap: var(--spacing-1);
}

:deep(.mode-btn) {
  flex: 1;
  height: var(--button-height);
  font-size: var(--text-sm);
  font-weight: var(--font-weight-heading);
  color: var(--text-secondary);
  background: var(--bg-hover);
  border: 1px solid var(--border-light);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

:deep(.mode-btn:hover) {
  color: var(--text-primary);
  border-color: var(--border-medium);
}

:deep(.mode-btn.active) {
  color: var(--color-primary-300);
  background: color-mix(in srgb, var(--color-primary-500) 15%, transparent);
  border-color: color-mix(in srgb, var(--color-primary-500) 40%, transparent);
}

:deep(.group-title-row) {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--spacing-3);
}

:deep(.group-title-row .attendance-group-title) {
  margin-bottom: 0;
}

:deep(.toggle-label) {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  font-size: var(--text-sm);
  color: var(--text-secondary);
  cursor: pointer;
  user-select: none;
}
</style>
