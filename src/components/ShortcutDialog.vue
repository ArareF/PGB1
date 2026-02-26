<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

const props = defineProps<{ show?: boolean }>()

interface AppShortcut {
  name: string
  target_path: string
}

const emit = defineEmits<{
  save: [data: { shortcut_type: string; name: string; path: string; custom_icon: string | null }]
  cancel: []
}>()

const type = ref<'app' | 'folder' | 'web'>('app')
const path = ref('')
const name = ref('')
const customIconPath = ref<string | null>(null)
const previewLoading = ref(false)

const customIconUrl = computed(() =>
  customIconPath.value
    ? convertFileSrc(customIconPath.value.replace(/\\/g, '/'))
    : null
)

const canFetchPreview = computed(() =>
  (type.value === 'app' && !!selectedApp.value) ||
  (type.value === 'web' && path.value.trim().length > 0)
)

async function fetchIconPreview() {
  if (previewLoading.value || !canFetchPreview.value) return
  const tempId = crypto.randomUUID()
  previewLoading.value = true
  try {
    let result: string | null = null
    if (type.value === 'app') {
      result = await invoke<string>('extract_exe_icon', { exePath: path.value, iconId: tempId })
    } else if (type.value === 'web') {
      result = await invoke<string | null>('fetch_favicon', { url: path.value, iconId: tempId })
    }
    if (result) customIconPath.value = result
  } catch (e) {
    console.error('图标获取失败', e)
  } finally {
    previewLoading.value = false
  }
}

// 应用选择相关
const appList = ref<AppShortcut[]>([])
const appSearch = ref('')
const appListLoading = ref(false)
const selectedApp = ref<AppShortcut | null>(null)

const filteredApps = computed(() => {
  const q = appSearch.value.trim().toLowerCase()
  if (!q) return appList.value
  return appList.value.filter(a => a.name.toLowerCase().includes(q))
})

// 切换为应用类型时加载列表
watch(type, async (t) => {
  if (t === 'app' && appList.value.length === 0 && !appListLoading.value) {
    await loadAppList()
  }
})

onMounted(async () => {
  // 默认是应用类型，直接加载
  if (type.value === 'app') {
    await loadAppList()
  }
})

async function loadAppList() {
  appListLoading.value = true
  try {
    appList.value = await invoke<AppShortcut[]>('scan_app_shortcuts')
  } catch (e) {
    console.error('扫描应用列表失败', e)
  } finally {
    appListLoading.value = false
  }
}

function selectApp(app: AppShortcut) {
  selectedApp.value = app
  path.value = app.target_path
  name.value = app.name
  customIconPath.value = null  // 换应用时重置自定义图标
}

// 切换类型时清空
function selectType(t: 'app' | 'folder' | 'web') {
  type.value = t
  path.value = ''
  name.value = ''
  selectedApp.value = null
  appSearch.value = ''
  customIconPath.value = null
}

async function browseCustomIcon() {
  try {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: t('shortcut.imageFilter'), extensions: ['png', 'jpg', 'jpeg', 'ico', 'bmp', 'webp'] }],
    })
    if (selected && typeof selected === 'string') {
      const tempId = crypto.randomUUID()
      const cachedPath = await invoke<string>('copy_icon_to_cache', {
        srcPath: selected,
        iconId: tempId,
      })
      customIconPath.value = cachedPath
    }
  } catch (e) {
    console.error('图标复制到缓存失败', e)
  }
}

async function browse() {
  try {
    if (type.value === 'folder') {
      const selected = await openDialog({ multiple: false, directory: true })
      if (selected && typeof selected === 'string') {
        path.value = selected
        const parts = selected.replace(/\\/g, '/').split('/')
        name.value = parts[parts.length - 1] || selected
      }
    }
  } catch (e) {
    console.error('文件夹选择失败', e)
  }
}

async function browseExe() {
  try {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: t('shortcut.typeApp'), extensions: ['exe'] }],
    })
    if (selected && typeof selected === 'string') {
      path.value = selected
      const parts = selected.replace(/\\/g, '/').split('/')
      const filename = parts[parts.length - 1]
      name.value = filename.replace(/\.exe$/i, '')
      selectedApp.value = { name: name.value, target_path: selected }
    }
  } catch (e) {
    console.error('文件选择失败', e)
  }
}

const canSave = computed(() => path.value.trim().length > 0 && name.value.trim().length > 0)

function handleSave() {
  if (!canSave.value) return
  emit('save', {
    shortcut_type: type.value,
    name: name.value.trim(),
    path: path.value.trim(),
    custom_icon: customIconPath.value,
  })
}
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog">
    <div v-if="props.show !== false" class="dialog-overlay">
      <div class="dialog-content glass-strong">
        <p class="dialog-title">{{ $t('shortcut.addTitle') }}</p>

        <div class="dialog-body">
          <!-- 类型选择 -->
          <div class="type-selector">
            <button class="type-btn" :class="{ active: type === 'app' }" @click="selectType('app')">{{ $t('shortcut.typeApp') }}</button>
            <button class="type-btn" :class="{ active: type === 'folder' }" @click="selectType('folder')">{{ $t('shortcut.typeFolder') }}</button>
            <button class="type-btn" :class="{ active: type === 'web' }" @click="selectType('web')">{{ $t('shortcut.typeWeb') }}</button>
          </div>

          <!-- 应用：搜索 + 列表 -->
          <template v-if="type === 'app'">
            <input
              v-model="appSearch"
              class="field-input search-input"
              :placeholder="$t('shortcut.searchApps')"
              autocomplete="off"
            />
            <div class="app-list">
              <div v-if="appListLoading" class="app-list-hint">{{ $t('common.scanning') }}</div>
              <div v-else-if="filteredApps.length === 0" class="app-list-hint">{{ $t('shortcut.noAppsFound') }}</div>
              <button
                v-for="app in filteredApps"
                :key="app.target_path"
                class="app-item"
                :class="{ selected: selectedApp?.target_path === app.target_path }"
                @click="selectApp(app)"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" class="app-item-icon">
                  <path d="M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z" stroke="currentColor" stroke-width="1.5"/>
                  <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" stroke="currentColor" stroke-width="1.5"/>
                </svg>
                <span class="app-item-name">{{ app.name }}</span>
              </button>
            </div>
            <button class="browse-link" @click="browseExe">{{ $t('shortcut.manualSelect') }}</button>
          </template>

          <!-- 文件夹 -->
          <template v-else-if="type === 'folder'">
            <div class="path-row">
              <input v-model="path" class="field-input" :placeholder="$t('shortcut.folderPath')" readonly />
              <button class="browse-btn" @click="browse">{{ $t('common.browse') }}</button>
            </div>
            <label class="field-label">{{ $t('common.name') }}</label>
            <input v-model="name" class="field-input" :placeholder="$t('shortcut.displayName')" @keydown.enter="handleSave" />
          </template>

          <!-- 网页 -->
          <template v-else>
            <label class="field-label">{{ $t('shortcut.url') }}</label>
            <input v-model="path" class="field-input" placeholder="https://example.com" @keydown.enter="handleSave" />
            <label class="field-label">{{ $t('common.name') }}</label>
            <input v-model="name" class="field-input" :placeholder="$t('shortcut.displayName')" @keydown.enter="handleSave" />
          </template>

          <!-- 应用已选中时显示确认名称编辑框 -->
          <template v-if="type === 'app' && selectedApp">
            <label class="field-label">{{ $t('common.name') }}</label>
            <input v-model="name" class="field-input" :placeholder="$t('shortcut.displayName')" @keydown.enter="handleSave" />
          </template>

          <!-- 图标预览 + 自定义上传 -->
          <div class="icon-preview-row">
            <button
              class="icon-preview-btn"
              :class="{ 'has-custom': customIconPath }"
              :title="$t('shortcut.clickToReplaceIcon')"
              @click="browseCustomIcon"
            >
              <img v-if="customIconUrl" :src="customIconUrl" class="preview-img" draggable="false" />
              <template v-else>
                <svg v-if="type === 'app'" class="preview-svg" viewBox="0 0 24 24" fill="none">
                  <path d="M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z" stroke="currentColor" stroke-width="1.5"/>
                  <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" stroke="currentColor" stroke-width="1.5"/>
                </svg>
                <svg v-else-if="type === 'folder'" class="preview-svg" viewBox="0 0 24 24" fill="none">
                  <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/>
                </svg>
                <svg v-else class="preview-svg" viewBox="0 0 24 24" fill="none">
                  <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="1.5"/>
                  <line x1="2" y1="12" x2="22" y2="12" stroke="currentColor" stroke-width="1.5"/>
                  <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" stroke="currentColor" stroke-width="1.5"/>
                </svg>
              </template>
              <!-- 悬停编辑遮罩 -->
              <div class="preview-overlay">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                  <path d="M12 20h9" stroke="white" stroke-width="2" stroke-linecap="round"/>
                  <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              </div>
            </button>
            <div class="preview-info">
              <span class="preview-status" :class="{ 'is-custom': customIconPath }">
                {{ customIconPath ? $t('shortcut.customIcon') : $t('shortcut.autoIcon') }}
              </span>
              <span class="preview-hint">{{ $t('shortcut.clickToReplaceIcon') }}</span>
              <button
                v-if="canFetchPreview"
                class="preview-fetch-btn"
                :disabled="previewLoading"
                @click="fetchIconPreview"
              >{{ previewLoading ? $t('shortcut.fetchingIcon') : $t('shortcut.previewAutoIcon') }}</button>
            </div>
          </div>
        </div>

        <div class="dialog-actions">
          <div class="actions-right">
            <button class="dialog-btn dialog-btn-primary" :disabled="!canSave" @click="handleSave">{{ $t('common.add') }}</button>
            <button class="dialog-btn dialog-btn-secondary" @click="$emit('cancel')">{{ $t('common.cancel') }}</button>
          </div>
        </div>
      </div>
    </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal, 1000);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--overlay-backdrop);
  backdrop-filter: blur(var(--glass-light-blur));
}

.dialog-content {
  width: 320px;
  max-height: 80vh;
  border-radius: var(--floating-navbar-radius);
  padding: var(--spacing-6);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
  overflow: hidden;
}

.dialog-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-weight-heading);
  color: var(--text-primary);
  flex-shrink: 0;
}

.dialog-body {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
  overflow: hidden;
  min-height: 0;
}

.field-label {
  font-size: var(--text-base);
  color: var(--text-secondary);
}

.type-selector {
  display: flex;
  gap: var(--spacing-2);
  flex-shrink: 0;
}

.type-btn {
  flex: 1;
  height: var(--button-height);
  font-size: var(--text-base);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.type-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.type-btn.active {
  background: color-mix(in srgb, var(--color-primary-500) 20%, transparent);
  border-color: var(--color-primary-400);
  color: var(--color-primary-400);
}

.search-input {
  flex-shrink: 0;
}

.app-list {
  flex: 1;
  min-height: 0;
  max-height: 240px;
  overflow-y: auto;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
}

.app-list-hint {
  padding: var(--spacing-4);
  text-align: center;
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}

.app-item {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  width: 100%;
  padding: var(--spacing-2) var(--spacing-3);
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  text-align: left;
  transition: background var(--transition-fast);
}

.app-item:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.app-item.selected {
  background: color-mix(in srgb, var(--color-primary-500) 15%, transparent);
  color: var(--color-primary-400);
}

.app-item-icon {
  flex-shrink: 0;
  opacity: 0.6;
}

.app-item-name {
  font-size: var(--text-sm);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.browse-link {
  background: transparent;
  border: none;
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  cursor: pointer;
  padding: 0;
  text-align: left;
  text-decoration: underline;
  transition: color var(--transition-fast);
  flex-shrink: 0;
}

.browse-link:hover {
  color: var(--text-secondary);
}

.path-row {
  display: flex;
  gap: var(--spacing-2);
}

.path-row .field-input {
  flex: 1;
  min-width: 0;
}

.field-input {
  height: var(--button-height);
  padding: 0 var(--spacing-3);
  font-size: var(--text-base);
  color: var(--text-primary);
  background: var(--bg-elevated);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-md);
  outline: none;
  transition: border-color var(--transition-fast);
  width: 100%;
  box-sizing: border-box;
}

.field-input:focus {
  border-color: var(--color-primary);
}

.field-input::placeholder {
  color: var(--text-tertiary);
}

.browse-btn {
  height: var(--button-height);
  padding: 0 var(--spacing-3);
  font-size: var(--text-sm);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-medium);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  white-space: nowrap;
  transition: all var(--transition-fast);
}

.browse-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

/* 图标预览行 */
.icon-preview-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  flex-shrink: 0;
  padding-top: var(--spacing-1);
  border-top: 1px solid var(--border-subtle);
}

.icon-preview-btn {
  position: relative;
  width: 44px;
  height: 44px;
  flex-shrink: 0;
  border-radius: var(--radius-lg);
  border: 2px dashed var(--border-medium);
  background: var(--bg-elevated);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  transition: border-color var(--duration-fast) var(--ease-out);
  padding: 0;
}

.icon-preview-btn.has-custom {
  border: 2px solid var(--color-primary-400);
}

.icon-preview-btn:hover {
  border-color: var(--color-primary-300);
}

.icon-preview-btn:hover .preview-overlay {
  opacity: 1;
}

.preview-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.preview-svg {
  width: 22px;
  height: 22px;
  color: var(--text-tertiary);
}

.preview-overlay {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  transition: opacity var(--duration-fast) var(--ease-out);
}

.preview-info {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.preview-status {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.preview-status.is-custom {
  color: var(--color-primary-400);
}

.preview-hint {
  font-size: var(--text-2xs);
  color: var(--text-tertiary);
}

.preview-fetch-btn {
  margin-top: 2px;
  background: transparent;
  border: none;
  padding: 0;
  font-size: var(--text-2xs);
  color: var(--color-primary-400);
  cursor: pointer;
  text-align: left;
  text-decoration: underline;
  transition: color var(--duration-fast) var(--ease-out);
}

.preview-fetch-btn:hover:not(:disabled) {
  color: var(--color-primary-300);
}

.preview-fetch-btn:disabled {
  color: var(--text-tertiary);
  cursor: default;
  text-decoration: none;
}

.dialog-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}

.actions-right {
  display: flex;
  gap: var(--spacing-3);
  margin-left: auto;
}

.dialog-btn {
  display: inline-flex;
  align-items: center;
  height: var(--button-height);
  padding: 0 var(--spacing-5);
  font-size: var(--text-base);
  font-weight: var(--font-weight-heading);
  border-radius: var(--radius-md);
  border: none;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.dialog-btn-primary {
  background: color-mix(in srgb, var(--color-primary-500) 75%, transparent);
  backdrop-filter: blur(var(--glass-subtle-blur));
  -webkit-backdrop-filter: blur(var(--glass-subtle-blur));
  color: var(--color-neutral-0);
}

.dialog-btn-primary:hover:not(:disabled) {
  background: color-mix(in srgb, var(--color-primary-500) 90%, transparent);
}

.dialog-btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.dialog-btn-secondary {
  background: transparent;
  color: var(--text-secondary);
  border: 1px solid var(--border-medium);
}

.dialog-btn-secondary:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}
</style>

<style>
/* 弹窗进出动画 */
/* 根元素 .dialog-overlay 必须有 transition，Vue 以此计算等待时长 */
.dialog-enter-active {
  transition: opacity var(--duration-dialog) var(--ease-out);
}
.dialog-leave-active {
  transition: opacity var(--duration-dialog) var(--ease-in);
}
.dialog-enter-from,
.dialog-leave-to {
  opacity: 0;
}
/* 内容区额外的 transform 动画 */
.dialog-enter-active .dialog-content {
  transition: transform var(--duration-dialog) var(--ease-out);
}
.dialog-leave-active .dialog-content {
  transition: transform var(--duration-dialog) var(--ease-in);
}
.dialog-enter-from .dialog-content {
  transform: translateY(16px) scale(0.97);
}
.dialog-leave-to .dialog-content {
  transform: translateY(8px) scale(0.97);
}
</style>
