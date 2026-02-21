<script setup lang="ts">
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'
import ShortcutDialog from './ShortcutDialog.vue'

interface Shortcut {
  id: string
  shortcut_type: 'app' | 'folder' | 'web'
  name: string
  path: string
  icon_cache: string | null
  order: number
}

const shortcuts = ref<Shortcut[]>([])
const showDialog = ref(false)
const isEditing = ref(false)

// ─── 数据加载 / 保存 ───────────────────────────────────────────

onMounted(async () => {
  await loadShortcuts()
  document.addEventListener('pointerdown', onDocPointerDown)
})

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onDocPointerDown)
})

async function loadShortcuts() {
  try {
    shortcuts.value = await invoke<Shortcut[]>('load_shortcuts')
  } catch (e) {
    console.error('加载快捷方式失败', e)
  }
}

async function saveShortcuts(list: Shortcut[]) {
  try {
    await invoke('save_shortcuts', { shortcuts: list })
    shortcuts.value = list
  } catch (e) {
    console.error('保存快捷方式失败', e)
  }
}

// ─── 编辑模式 ─────────────────────────────────────────────────

const sidebarEl = ref<HTMLElement | null>(null)

/** 点击 sidebar 外部时退出编辑模式 */
function onDocPointerDown(e: PointerEvent) {
  if (!isEditing.value) return
  if (sidebarEl.value && !sidebarEl.value.contains(e.target as Node)) {
    isEditing.value = false
  }
}

function enterEditMode() {
  isEditing.value = true
}

function exitEditMode() {
  isEditing.value = false
}

// ─── 删除 ─────────────────────────────────────────────────────

async function deleteShortcut(id: string) {
  const updated = shortcuts.value
    .filter(s => s.id !== id)
    .map((s, i) => ({ ...s, order: i }))
  await saveShortcuts(updated)
}

// ─── 添加 ─────────────────────────────────────────────────────

async function handleSave(data: { shortcut_type: string; name: string; path: string; custom_icon: string | null }) {
  showDialog.value = false
  const id = crypto.randomUUID()
  // 用户自定义图标优先；否则按类型自动获取
  let iconCache: string | null = data.custom_icon ?? null
  if (!iconCache) {
    if (data.shortcut_type === 'app') {
      iconCache = await tryExtractIcon(data.path, id)
    } else if (data.shortcut_type === 'web') {
      iconCache = await tryFetchFavicon(data.path, id)
    }
  }
  const newShortcut: Shortcut = {
    id,
    shortcut_type: data.shortcut_type as Shortcut['shortcut_type'],
    name: data.name,
    path: data.path,
    icon_cache: iconCache,
    order: shortcuts.value.length,
  }
  await saveShortcuts([...shortcuts.value, newShortcut])
}

async function tryExtractIcon(exePath: string, iconId: string): Promise<string | null> {
  try {
    return await invoke<string>('extract_exe_icon', { exePath, iconId })
  } catch (e) {
    console.warn('图标提取失败', e)
    return null
  }
}

async function tryFetchFavicon(url: string, iconId: string): Promise<string | null> {
  try {
    return await invoke<string | null>('fetch_favicon', { url, iconId })
  } catch (e) {
    console.warn('favicon 获取失败', e)
    return null
  }
}

function toAssetUrl(filePath: string): string {
  return convertFileSrc(filePath.replace(/\\/g, '/'))
}

// ─── 单击启动 ─────────────────────────────────────────────────

async function launchShortcut(shortcut: Shortcut) {
  if (isEditing.value) return
  try {
    await invoke('launch_shortcut', {
      shortcutType: shortcut.shortcut_type,
      path: shortcut.path,
    })
  } catch (e) {
    console.error('启动失败', e)
  }
}

// ─── 长按检测 ─────────────────────────────────────────────────

let pressTimer: number | null = null
let didLongPress = false

function onItemPointerDown(_e: PointerEvent, shortcut: Shortcut) {
  if (isEditing.value) {
    startDrag(shortcut)
    return
  }
  didLongPress = false
  pressTimer = window.setTimeout(() => {
    didLongPress = true
    enterEditMode()
  }, 500)
}

function onItemPointerUp(shortcut: Shortcut) {
  if (pressTimer !== null) {
    clearTimeout(pressTimer)
    pressTimer = null
  }
  if (!didLongPress && !isEditing.value) {
    launchShortcut(shortcut)
  }
}

function onItemPointerLeave() {
  if (pressTimer !== null) {
    clearTimeout(pressTimer)
    pressTimer = null
  }
}

// ─── 拖拽排序 ─────────────────────────────────────────────────

const draggingId = ref<string | null>(null)
// displayOrder 在拖拽过程中实时更新，驱动列表视觉顺序
const displayOrder = ref<string[]>([])

// 同步 shortcuts 到 displayOrder（非拖拽时）
watch(() => shortcuts.value, (list) => {
  if (!draggingId.value) {
    displayOrder.value = list.map(s => s.id)
  }
}, { immediate: true })

// 按 displayOrder 排列的列表（用于模板渲染）
const orderedShortcuts = computed(() =>
  displayOrder.value
    .map(id => shortcuts.value.find(s => s.id === id))
    .filter((s): s is Shortcut => !!s)
)

function startDrag(shortcut: Shortcut) {
  draggingId.value = shortcut.id

  // pointer-events: none 由 CSS .is-dragging 负责（见样式）

  const onMove = (ev: PointerEvent) => {
    const el = document.elementFromPoint(ev.clientX, ev.clientY)
    const wrapper = el?.closest('[data-shortcut-id]') as HTMLElement | null
    const hoverId = wrapper?.dataset.shortcutId ?? null
    if (!hoverId || hoverId === draggingId.value) return

    // 实时重排 displayOrder，驱动平滑动画
    const order = [...displayOrder.value]
    const fromIdx = order.indexOf(draggingId.value!)
    const toIdx   = order.indexOf(hoverId)
    if (fromIdx !== -1 && toIdx !== -1) {
      order.splice(fromIdx, 1)
      order.splice(toIdx, 0, draggingId.value!)
      displayOrder.value = order
    }
  }

  const onUp = () => {
    window.removeEventListener('pointermove', onMove)
    window.removeEventListener('pointerup', onUp)

    // 按最终 displayOrder 保存
    const finalList = displayOrder.value
      .map(id => shortcuts.value.find(s => s.id === id))
      .filter((s): s is Shortcut => !!s)
      .map((s, i) => ({ ...s, order: i }))

    draggingId.value = null
    saveShortcuts(finalList)
  }

  window.addEventListener('pointermove', onMove)
  window.addEventListener('pointerup', onUp)
}
</script>

<template>
  <aside ref="sidebarEl" class="sidebar glass-medium" @click.self="exitEditMode">
    <!-- 快捷方式列表 -->
    <TransitionGroup tag="div" class="sidebar-items" name="sort">
      <div
        v-for="shortcut in orderedShortcuts"
        :key="shortcut.id"
        class="shortcut-wrapper"
        :data-shortcut-id="shortcut.id"
        :class="{ 'is-dragging': draggingId === shortcut.id }"
      >
        <!-- 图标按钮 -->
        <div
          class="shortcut-item"
          :class="{ 'editing': isEditing }"
          @pointerdown.stop="onItemPointerDown($event, shortcut)"
          @pointerup.stop="onItemPointerUp(shortcut)"
          @pointerleave="onItemPointerLeave"
          @contextmenu.prevent
        >
          <img
            v-if="shortcut.icon_cache"
            :src="toAssetUrl(shortcut.icon_cache)"
            class="shortcut-img"
            draggable="false"
          />
          <template v-else>
            <svg v-if="shortcut.shortcut_type === 'app'" class="shortcut-svg" viewBox="0 0 24 24" fill="none">
              <path d="M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z" stroke="currentColor" stroke-width="1"/>
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" stroke="currentColor" stroke-width="1"/>
            </svg>
            <svg v-else-if="shortcut.shortcut_type === 'folder'" class="shortcut-svg" viewBox="0 0 24 24" fill="none">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" stroke="currentColor" stroke-width="1" stroke-linejoin="round"/>
            </svg>
            <svg v-else class="shortcut-svg" viewBox="0 0 24 24" fill="none">
              <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="1"/>
              <line x1="2" y1="12" x2="22" y2="12" stroke="currentColor" stroke-width="1"/>
              <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" stroke="currentColor" stroke-width="1"/>
            </svg>
          </template>

          <!-- 编辑模式：右上角删除按钮 -->
          <button
            v-if="isEditing"
            class="delete-badge"
            @pointerdown.stop
            @click.stop="deleteShortcut(shortcut.id)"
          >
            <svg width="10" height="10" viewBox="0 0 10 10">
              <line x1="2" y1="2" x2="8" y2="8" stroke="white" stroke-width="1.5" stroke-linecap="round"/>
              <line x1="8" y1="2" x2="2" y2="8" stroke="white" stroke-width="1.5" stroke-linecap="round"/>
            </svg>
          </button>
        </div>

        <!-- 名称标签 -->
        <span class="shortcut-name">{{ shortcut.name }}</span>
      </div>
    </TransitionGroup>

    <!-- 添加按钮（始终在最底部） -->
    <button class="sidebar-add-btn" title="添加快捷方式" @click.stop="showDialog = true">
      <svg width="20" height="20" viewBox="0 0 20 20">
        <line x1="10" y1="4" x2="10" y2="16" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
        <line x1="4" y1="10" x2="16" y2="10" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
      </svg>
    </button>

    <!-- 添加弹窗 -->
    <ShortcutDialog
      :show="showDialog"
      @save="handleSave"
      @cancel="showDialog = false"
    />
  </aside>
</template>

<style scoped>
.sidebar {
  width: var(--floating-sidebar-width);
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: var(--floating-sidebar-padding);
  border-radius: var(--floating-navbar-radius);
  gap: var(--floating-sidebar-gap);
  flex-shrink: 0;
  overflow-y: auto;
  overflow-x: hidden;
}

.sidebar-add-btn {
  width: var(--floating-sidebar-item-size);
  height: var(--floating-sidebar-item-size);
  display: flex;
  align-items: center;
  justify-content: center;
  border: 2px dashed var(--border-medium);
  background: transparent;
  color: var(--text-tertiary);
  border-radius: var(--radius-lg);
  cursor: pointer;
  transition: background var(--duration-fast) var(--ease-out),
              border-color var(--duration-fast) var(--ease-out),
              transform var(--duration-fast) var(--ease-out),
              box-shadow var(--duration-fast) var(--ease-out),
              color var(--duration-fast) var(--ease-out);
  flex-shrink: 0;
}

.sidebar-add-btn:hover {
  border-color: var(--color-primary-400);
  color: var(--color-primary-500);
  background: var(--bg-active);
  transform: translateY(-2px);
  box-shadow: 0 3px 8px rgba(0, 0, 0, 0.25);
}

.sidebar-items {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--floating-sidebar-gap);
  width: 100%;
}

.shortcut-wrapper {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  width: var(--floating-sidebar-item-size);
}

/* FLIP 动画：拖拽排序时平滑位移 */
.sort-move {
  transition: transform 200ms cubic-bezier(0.25, 0.8, 0.25, 1);
}

.shortcut-wrapper.is-dragging {
  opacity: 0.5;
  transform: scale(1.08);
  z-index: 10;
  pointer-events: none;
}

.shortcut-item {
  position: relative;
  width: var(--floating-sidebar-item-size);
  height: var(--floating-sidebar-item-size);
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-lg);
  cursor: pointer;
  color: var(--text-secondary);
  transition: border-color var(--duration-fast) var(--ease-out),
              transform var(--duration-fast) var(--ease-out),
              box-shadow var(--duration-fast) var(--ease-out),
              color var(--duration-fast) var(--ease-out);
  flex-shrink: 0;
  padding: 0;
  overflow: visible;
  user-select: none;
  isolation: isolate;
}

/* 模糊光晕层 */
.shortcut-item::before {
  content: '';
  position: absolute;
  inset: -2px;
  border-radius: calc(var(--radius-lg) + 2px);
  background: var(--color-primary-400);
  filter: blur(14px);
  opacity: 0;
  transition: opacity var(--duration-fast) var(--ease-out);
  z-index: -1;
  pointer-events: none;
}

.shortcut-item:not(.editing):hover {
  color: var(--text-primary);
  transform: translateY(-2px);
}

.shortcut-item:not(.editing):hover::before {
  opacity: 0.45;
}

.shortcut-item:not(.editing):active {
  transform: translateY(0) scale(0.95);
  box-shadow: none;
}

/* 编辑模式抖动动画 */
.shortcut-item.editing {
  animation: wiggle 0.45s linear infinite;
  cursor: grab;
}

/* 错开各图标的相位，避免整齐同步 */
.shortcut-wrapper:nth-child(2n) .shortcut-item.editing { animation-delay: -0.09s; }
.shortcut-wrapper:nth-child(3n) .shortcut-item.editing { animation-delay: -0.05s; }
.shortcut-wrapper:nth-child(4n) .shortcut-item.editing { animation-delay: -0.13s; }

@keyframes wiggle {
  0%   { transform: rotate(-3deg) translateY(0px); }
  20%  { transform: rotate(3.5deg) translateY(-0.5px); }
  40%  { transform: rotate(-2deg) translateY(0.5px); }
  60%  { transform: rotate(4deg) translateY(0px); }
  80%  { transform: rotate(-3.5deg) translateY(-0.5px); }
  100% { transform: rotate(3deg) translateY(0px); }
}

.shortcut-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  pointer-events: none;
  border-radius: var(--radius-lg);
}

.shortcut-svg {
  width: 100%;
  height: 100%;
  padding: 10px;
  box-sizing: border-box;
  pointer-events: none;
}

/* 右上角删除徽章 */
.delete-badge {
  position: absolute;
  top: -6px;
  right: -6px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: #ff3b30;
  border: none;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  z-index: 10;
  padding: 0;
  transition: transform var(--transition-fast);
}

.delete-badge:hover {
  transform: scale(1.15);
}

.shortcut-name {
  font-size: var(--text-2xs);
  line-height: 1.2;
  text-align: center;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-secondary);
  pointer-events: none;
}
</style>
