<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import { useNavigation } from '../composables/useNavigation'
import { clearMediaCaches } from '../composables/useMediaCache'
import { partitionNormalizeItems } from '../utils/normalizeItems'

interface NormalizeItem {
  base_name: string
  material_type: 'static' | 'sequence'
  ext: string
  frame_count: number
  needs_rename: boolean
  is_png: boolean
  is_add_or_screen: boolean
  thumbnail_path: string
  paths: string[]
  target_name: string
  has_backup: boolean
}

/** 每个素材的操作勾选状态 */
interface Selection {
  rename: boolean
  trim: boolean
  blackBg: boolean
}

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const { setNavigation } = useNavigation()

const taskId = route.params.taskId as string
const taskPath = route.query.taskPath as string

const loading = ref(true)
const executing = ref(false)
const error = ref<string | null>(null)
const items = ref<NormalizeItem[]>([])
const selections = ref<Selection[]>([])
const progress = ref<{ current: number; total: number; name: string } | null>(null)
// 缩略图缓存破坏：每次重新盘点自增，强制刷新原地改动后的预览图
const reloadTick = ref(0)
// 已规范化素材下沉到底部且默认折叠，避免遮挡当前待处理内容
const normalizedExpanded = ref(false)

// ── 全局操作开关（驱动各行默认值，可逐行覆盖）──
const gRename = ref(true)   // 命名规范化默认开
const gTrim = ref(false)
const gBlackBg = ref(false)
const gBackup = ref(true)   // 执行前备份默认开

// ── 每行资格判定 ──
function canRename(it: NormalizeItem) { return it.needs_rename }
function canTrim(it: NormalizeItem) { return it.material_type === 'static' && it.is_png }
function canBlackBg(it: NormalizeItem) { return it.material_type === 'static' && it.is_png && it.is_add_or_screen }

/** 左侧显示原始名字：静帧取磁盘文件名（去扩展名），序列帧取基础名 */
function displayName(it: NormalizeItem): string {
  if (it.material_type === 'static' && it.paths[0]) {
    const file = it.paths[0].split(/[\\/]/).pop() ?? it.base_name
    const dot = file.lastIndexOf('.')
    return dot > 0 ? file.slice(0, dot) : file
  }
  return it.base_name
}

/** 按当前全局开关初始化某行的勾选（仅对有资格的操作生效）*/
function defaultSelection(it: NormalizeItem): Selection {
  return {
    rename: canRename(it) && gRename.value,
    trim: canTrim(it) && gTrim.value,
    blackBg: canBlackBg(it) && gBlackBg.value,
  }
}

async function loadItems() {
  if (!taskPath) { loading.value = false; return }
  loading.value = true
  error.value = null
  try {
    items.value = await invoke<NormalizeItem[]>('scan_normalize_items', { taskPath })
    selections.value = items.value.map(defaultSelection)
    reloadTick.value++
  } catch (e) {
    error.value = String(e)
    console.error('盘点规范化素材失败:', e)
  } finally {
    loading.value = false
  }
}

// 全局开关切换 → 批量重置有资格行的对应操作（手动覆盖会被下一次全局切换重置，符合"批量设置"语义）
watch(gRename, (v) => { items.value.forEach((it, i) => { if (canRename(it)) selections.value[i].rename = v }) })
watch(gTrim, (v) => { items.value.forEach((it, i) => { if (canTrim(it)) selections.value[i].trim = v }) })
watch(gBlackBg, (v) => { items.value.forEach((it, i) => { if (canBlackBg(it)) selections.value[i].blackBg = v }) })

/** 待执行的请求（至少勾选一项操作的素材）*/
const pendingRequests = computed(() =>
  items.value
    .map((it, i) => ({ it, sel: selections.value[i] }))
    .filter(({ sel }) => sel.rename || sel.trim || sel.blackBg)
)

const totalOps = computed(() =>
  selections.value.reduce((sum, s) => sum + (s.rename ? 1 : 0) + (s.trim ? 1 : 0) + (s.blackBg ? 1 : 0), 0)
)

/** 仅改变展示顺序；index 始终指向原始 selections，避免行内操作错位 */
const groupedItems = computed(() => partitionNormalizeItems(items.value))
const displayItems = computed(() => [
  ...groupedItems.value.pending,
  ...groupedItems.value.normalized,
])

onMounted(async () => {
  setNavigation({
    title: `${t('normalize.title')} · ${taskId}`,
    showBackButton: true,
    onBack: () => router.back(),
    actions: [],
    moreMenuItems: [],
  })
  await loadItems()
})

async function handleExecute() {
  if (pendingRequests.value.length === 0 || !taskPath) return
  executing.value = true
  error.value = null
  progress.value = null

  let unlisten: UnlistenFn | null = null
  try {
    unlisten = await listen<{ current: number; total: number; name: string }>('normalize-progress', (e) => {
      progress.value = e.payload
    })

    const requests = pendingRequests.value.map(({ it, sel }) => ({
      paths: it.paths,
      material_type: it.material_type,
      target_name: it.target_name,
      do_rename: sel.rename,
      do_trim: sel.trim,
      do_black_bg: sel.blackBg,
    }))

    await invoke('execute_normalize_v2', { requests, backup: gBackup.value })
    // 规范化是原地改 00_original 的文件（自适应画布 / 加黑底），路径完全不变——
    // 不清缓存的话返回任务页，序列帧 LRU 与静帧 URL 都会命中改之前的旧图
    clearMediaCaches()
    router.back()
  } catch (e) {
    error.value = String(e)
    console.error('执行规范化失败:', e)
  } finally {
    unlisten?.()
    executing.value = false
    progress.value = null
  }
}

/** 恢复某素材的纯净原件（覆盖当前文件），完成后重新盘点 */
async function handleRestore(it: NormalizeItem) {
  if (!it.paths[0]) return
  try {
    await invoke('restore_normalize_backup', { currentPath: it.paths[0], backupName: it.target_name })
    // 恢复原件同样是原地覆盖，先清缓存再重新盘点
    clearMediaCaches()
    await loadItems()
  } catch (e) {
    error.value = String(e)
    console.error('恢复原件失败:', e)
  }
}
</script>

<template>
  <!-- 素材列表区（占满 main-content）-->
  <div class="list-area">
    <p v-if="loading" class="hint-text">{{ $t('common.scanning') }}</p>
    <p v-else-if="items.length === 0" class="hint-text">{{ $t('normalize.noMaterials') }}</p>

    <div v-else class="normalize-table">
      <!-- 表头：操作列名（各行 checkbox 据此列对齐）-->
      <div class="table-head">
        <span class="col-thumb" />
        <span class="col-meta">{{ $t('normalize.columnMaterial') }}</span>
        <span class="col-preview">{{ $t('normalize.colPreview') }}</span>
        <span class="col-restore" />
        <span class="col-op">{{ $t('normalize.optRename') }}</span>
        <span class="col-op">{{ $t('normalize.optTrim') }}</span>
        <span class="col-op">{{ $t('normalize.optBlackBg') }}</span>
      </div>

      <div class="table-body">
        <div
          v-for="{ item: it, index: i } in displayItems"
          v-show="it.needs_rename || normalizedExpanded"
          :key="it.target_name + it.material_type"
          class="material-row"
          :class="{ 'normalized-row': !it.needs_rename }"
        >
          <!-- 缩略图（cache-bust：reloadTick 随重新盘点自增）-->
          <div class="thumb col-thumb">
            <img :src="`${convertFileSrc(it.thumbnail_path)}?v=${reloadTick}`" :alt="it.base_name" loading="lazy" />
            <span v-if="it.material_type === 'sequence'" class="frame-badge">{{ it.frame_count }}</span>
          </div>

          <!-- 名称 + 类型 + 恢复 -->
          <div class="meta col-meta">
            <span class="name" :title="displayName(it)">{{ displayName(it) }}</span>
            <span class="type-tag" :class="it.material_type">
              {{ it.material_type === 'sequence' ? $t('normalize.typeSequence') : $t('normalize.typeStatic') }}<template v-if="it.ext"> · {{ it.ext.toUpperCase() }}</template>
            </span>
          </div>

          <!-- 操作预览（竖线分隔，显示会进行的操作）-->
          <div class="preview col-preview">
            <template v-if="selections[i].rename">
              <span v-if="it.material_type === 'sequence'" class="pv-line">→ {{ it.target_name }}/ · {{ $t('normalize.moveFrames', { count: it.frame_count }) }}</span>
              <span v-else class="pv-line">→ {{ it.target_name }}</span>
            </template>
            <span v-if="selections[i].trim" class="pv-chip">{{ $t('normalize.optTrim') }}</span>
            <span v-if="selections[i].blackBg" class="pv-chip">{{ $t('normalize.optBlackBg') }}</span>
            <span v-if="!selections[i].rename && !selections[i].trim && !selections[i].blackBg" class="pv-none">{{ $t('normalize.noOp') }}</span>
          </div>

          <!-- 恢复原件（位于命名列左侧，仅有备份时显示）-->
          <div class="op-cell col-restore">
            <button v-if="it.has_backup" class="restore-btn" @click="handleRestore(it)">↺ {{ $t('normalize.restore') }}</button>
          </div>

          <!-- 操作列：命名 / 自适应画布 / 添加黑底（按列对齐）-->
          <div class="op-cell col-op">
            <label v-if="canRename(it)" class="op" @click.prevent="selections[i].rename = !selections[i].rename">
              <span class="checkbox" :class="{ checked: selections[i].rename }" />
            </label>
            <span v-else class="op-dash">{{ $t('normalize.alreadyNamed') }}</span>
          </div>
          <div class="op-cell col-op">
            <label v-if="canTrim(it)" class="op" @click.prevent="selections[i].trim = !selections[i].trim">
              <span class="checkbox" :class="{ checked: selections[i].trim }" />
            </label>
            <span v-else class="op-dash muted" :title="it.material_type === 'static' && !it.is_png ? $t('normalize.notPng') : ''">—</span>
          </div>
          <div class="op-cell col-op">
            <label v-if="canBlackBg(it)" class="op" @click.prevent="selections[i].blackBg = !selections[i].blackBg">
              <span class="checkbox" :class="{ checked: selections[i].blackBg }" />
            </label>
            <span v-else class="op-dash muted" :title="canTrim(it) && !it.is_add_or_screen ? $t('normalize.notAddScreen') : ''">—</span>
          </div>
        </div>

        <button
          v-if="groupedItems.normalized.length > 0"
          class="normalized-disclosure"
          type="button"
          :aria-expanded="normalizedExpanded"
          @click="normalizedExpanded = !normalizedExpanded"
        >
          <span class="disclosure-title">
            <span class="disclosure-icon" :class="{ expanded: normalizedExpanded }" aria-hidden="true">›</span>
            {{ $t('normalize.normalizedGroup', { count: groupedItems.normalized.length }) }}
          </span>
          <span class="disclosure-action">
            {{ normalizedExpanded ? $t('normalize.collapseGroup') : $t('normalize.expandGroup') }}
          </span>
        </button>
      </div>
    </div>
  </div>

  <!-- 控制面板：Teleport 到 #content-row -->
  <Teleport to="#content-row">
    <aside class="control-panel normalize-control-panel">
      <div class="panel-body">
        <p class="panel-title">{{ $t('normalize.columnOps') }}</p>

        <label class="global-toggle" @click.prevent="gRename = !gRename">
          <span class="checkbox" :class="{ checked: gRename }" />
          <span class="toggle-text">
            <span class="toggle-label">{{ $t('normalize.optRename') }}</span>
            <span class="toggle-hint">{{ $t('normalize.optRenameHint') }}</span>
          </span>
        </label>

        <label class="global-toggle" @click.prevent="gTrim = !gTrim">
          <span class="checkbox" :class="{ checked: gTrim }" />
          <span class="toggle-text">
            <span class="toggle-label">{{ $t('normalize.optTrim') }}</span>
            <span class="toggle-hint">{{ $t('normalize.optTrimHint') }}</span>
          </span>
        </label>

        <label class="global-toggle" @click.prevent="gBlackBg = !gBlackBg">
          <span class="checkbox" :class="{ checked: gBlackBg }" />
          <span class="toggle-text">
            <span class="toggle-label">{{ $t('normalize.optBlackBg') }}</span>
            <span class="toggle-hint">{{ $t('normalize.optBlackBgHint') }}</span>
          </span>
        </label>

        <div class="divider" />

        <label class="global-toggle" @click.prevent="gBackup = !gBackup">
          <span class="checkbox" :class="{ checked: gBackup }" />
          <span class="toggle-text">
            <span class="toggle-label">{{ $t('normalize.optBackup') }}</span>
            <span class="toggle-hint">{{ $t('normalize.optBackupHint') }}</span>
          </span>
        </label>
      </div>

      <div class="panel-footer">
        <div v-if="error" class="error-msg">{{ error }}</div>
        <div v-if="executing && progress" class="exec-progress">
          <div class="progress-text">{{ $t('normalize.progress', { current: progress.current, total: progress.total }) }}</div>
          <div class="progress-filename">{{ progress.name }}</div>
          <div class="progress-bar-track">
            <div class="progress-bar-fill" :style="{ width: (progress.current / progress.total * 100) + '%' }" />
          </div>
        </div>
        <div v-else-if="executing" class="executing-hint">{{ $t('common.executing') }}</div>

        <div class="footer-actions">
          <button class="cancel-btn" :disabled="executing" @click="router.back()">{{ $t('common.cancel') }}</button>
          <button class="execute-btn" :disabled="pendingRequests.length === 0 || executing" @click="handleExecute">
            {{ executing ? $t('common.executing') : `${$t('normalize.execute')} (${totalOps})` }}
          </button>
        </div>
      </div>
    </aside>
  </Teleport>
</template>

<style scoped>
.list-area {
  height: 100%;
  display: flex;
  flex-direction: column;
}

/* 七列网格：缩略图 | 名称 | 预览 | 恢复 | 命名 | 自适应画布 | 添加黑底（表头与行共用列定义，保证按列对齐）*/
.normalize-table {
  --grid-cols: 56px minmax(110px, 1fr) minmax(150px, 1.5fr) 104px 72px 100px 88px;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 2px;
}

/* 表头固定在顶部不参与滚动（滚动容器压到其下方）*/
.table-head {
  flex-shrink: 0;
  display: grid;
  grid-template-columns: var(--grid-cols);
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-2) var(--spacing-2) var(--spacing-3);
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  color: var(--text-primary);
  letter-spacing: 0.02em;
  border-bottom: 1px solid var(--border-medium);
}

.table-head .col-op {
  text-align: center;
}

/* 只有素材行在表头下方独立滚动 */
.table-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}

.material-row {
  order: 0;
  display: grid;
  grid-template-columns: var(--grid-cols);
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-2);
  border-bottom: 1px solid var(--border-light);
}

.material-row:hover {
  background: var(--bg-hover);
}

.material-row.normalized-row {
  order: 2;
}

.normalized-disclosure {
  order: 1;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-3);
  padding: var(--spacing-3) var(--spacing-2);
  border: 0;
  border-bottom: 1px solid var(--border-light);
  background: transparent;
  color: var(--text-secondary);
  font: inherit;
  cursor: pointer;
  transition: background var(--duration-fast), color var(--duration-fast);
}

.normalized-disclosure:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.normalized-disclosure:focus-visible {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.disclosure-title {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
}

.disclosure-icon {
  display: inline-flex;
  color: var(--text-tertiary);
  transform: rotate(0deg);
  transition: transform var(--duration-fast) var(--ease-out);
}

.disclosure-icon.expanded {
  transform: rotate(90deg);
}

.disclosure-action {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.thumb {
  position: relative;
  width: 56px;
  height: 56px;
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--bg-secondary);
}

.thumb img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.frame-badge {
  position: absolute;
  right: 2px;
  bottom: 2px;
  padding: 0 4px;
  border-radius: var(--radius-sm);
  background: var(--overlay-backdrop);
  color: white;
  font-size: var(--text-2xs);
  font-weight: var(--font-bold);
}

.meta {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.name {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.type-tag {
  font-size: var(--text-2xs);
  color: var(--text-tertiary);
}

.type-tag.sequence {
  color: var(--color-primary-600);
}

.restore-btn {
  padding: var(--spacing-2) var(--spacing-3);
  border: 1px solid var(--border-medium);
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-family: inherit;
  white-space: nowrap;
  cursor: pointer;
  transition: all var(--duration-fast);
}

.restore-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

/* 操作预览列：左侧竖线分隔 */
.preview {
  min-width: 0;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
  border-left: 1px solid var(--border-medium);
  padding-left: var(--spacing-3);
}

.pv-line {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}

.pv-chip {
  font-size: var(--text-2xs);
  padding: 0 6px;
  border-radius: var(--radius-sm);
  background: var(--color-primary-50);
  color: var(--color-primary-700);
  white-space: nowrap;
}

.pv-none {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.op-cell {
  display: flex;
  align-items: center;
  justify-content: center;
}

.op {
  display: flex;
  align-items: center;
  cursor: pointer;
  user-select: none;
}

.op-dash {
  font-size: var(--text-2xs);
  color: var(--text-tertiary);
  white-space: nowrap;
}

.op-dash.muted {
  opacity: 0.5;
}

/* Checkbox（与规范化弹窗一致） */
.checkbox {
  width: 18px;
  height: 18px;
  border: 2px solid var(--border-heavy);
  border-radius: var(--radius-sm);
  position: relative;
  transition: all var(--duration-fast);
  flex-shrink: 0;
}

.checkbox.checked {
  background: var(--color-primary-500);
  border-color: var(--color-primary-500);
}

.checkbox.checked::after {
  content: '';
  position: absolute;
  top: 2px;
  left: 5px;
  width: 4px;
  height: 8px;
  border: solid white;
  border-width: 0 2px 2px 0;
  transform: rotate(45deg);
}

.hint-text {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  padding: var(--spacing-8);
  text-align: center;
}
</style>

<!-- Teleport 出去的面板用非 scoped style -->
<style>
.normalize-control-panel .panel-body {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.normalize-control-panel .global-toggle {
  display: flex;
  align-items: flex-start;
  gap: var(--spacing-3);
  cursor: pointer;
  user-select: none;
}

.normalize-control-panel .toggle-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.normalize-control-panel .toggle-label {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--text-primary);
}

.normalize-control-panel .toggle-hint {
  font-size: var(--text-2xs);
  color: var(--text-tertiary);
  line-height: 1.3;
}

.normalize-control-panel .divider {
  height: 1px;
  background: var(--border-light);
  margin: var(--spacing-1) 0;
}

.normalize-control-panel .panel-footer {
  flex-direction: column;
  gap: var(--spacing-3);
}

.normalize-control-panel .footer-actions {
  display: flex;
  gap: var(--spacing-2);
}

.normalize-control-panel .error-msg {
  padding: var(--spacing-2) var(--spacing-3);
  background: var(--color-danger-light);
  color: var(--color-danger-dark);
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
}

.normalize-control-panel .executing-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  text-align: center;
}

.normalize-control-panel .exec-progress {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.normalize-control-panel .progress-text {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  font-weight: var(--font-medium);
}

.normalize-control-panel .progress-filename {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.normalize-control-panel .progress-bar-track {
  height: 4px;
  border-radius: 2px;
  background: var(--bg-tertiary);
  overflow: hidden;
}

.normalize-control-panel .progress-bar-fill {
  height: 100%;
  border-radius: 2px;
  background: var(--color-primary-500);
  transition: width var(--duration-fast) var(--ease-out);
}
</style>
