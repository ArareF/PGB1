<script setup lang="ts">
import { ref, watch, onBeforeUnmount } from 'vue'

/** 菜单项定义 */
export interface SidebarActionItem {
  /** 唯一标识，select 事件回传 */
  id: string
  label: string
  /** 危险操作（删除类），走 danger 配色 */
  danger?: boolean
  disabled?: boolean
}

const props = defineProps<{
  items: SidebarActionItem[]
  /** 触发按钮文案 */
  label: string
}>()

const emit = defineEmits<{
  select: [id: string]
}>()

const triggerRef = ref<HTMLElement | null>(null)
const open = ref(false)
/** 弹层定位（fixed 定位到触发按钮正上方；Teleport to body 后无法用相对定位） */
const menuPos = ref({ left: '0px', bottom: '0px' })

/** 触发按钮与弹层之间的间距，与 --spacing-2 对齐 */
const MENU_GAP_PX = 8

function toggle() {
  if (open.value) {
    open.value = false
    return
  }
  const rect = triggerRef.value?.getBoundingClientRect()
  if (!rect) return
  menuPos.value = {
    left: `${rect.left}px`,
    // 向上弹出：用 bottom 锚定按钮上沿，菜单多高都不会盖住按钮
    bottom: `${window.innerHeight - rect.top + MENU_GAP_PX}px`,
  }
  open.value = true
}

function onSelect(item: SidebarActionItem) {
  if (item.disabled) return
  open.value = false
  emit('select', item.id)
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') open.value = false
}

// Esc 关闭：监听挂在 window 上，因为点开后焦点可能已不在触发按钮上
watch(open, (isOpen) => {
  if (isOpen) window.addEventListener('keydown', onKeydown)
  else window.removeEventListener('keydown', onKeydown)
})

// 侧边栏可能在菜单开着时被关掉（选中素材切换 / 侧边栏收起），
// 弹层是 Teleport 到 body 的，不随触发按钮卸载而消失，这里补一刀
onBeforeUnmount(() => {
  open.value = false
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <button
    ref="triggerRef"
    class="sidebar-action-btn"
    :disabled="props.items.length === 0"
    @click="toggle"
  >
    {{ props.label }}
    <svg class="sidebar-action-caret" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="18 15 12 9 6 15" />
    </svg>
  </button>

  <Teleport to="body">
    <Transition name="sidebar-action-menu">
      <div
        v-if="open"
        class="sidebar-action-menu"
        :style="{ left: menuPos.left, bottom: menuPos.bottom }"
      >
        <button
          v-for="item in props.items"
          :key="item.id"
          class="sidebar-action-menu-item"
          :class="{ danger: item.danger }"
          :disabled="item.disabled"
          @click="onSelect(item)"
        >
          {{ item.label }}
        </button>
      </div>
    </Transition>
    <!-- 点击外部关闭遮罩 -->
    <div v-if="open" class="sidebar-action-menu-overlay" @click="open = false" />
  </Teleport>
</template>

<style scoped>
/* .sidebar-action-btn / .sidebar-action-menu* → design-system.css 公共类（弹层 Teleport 到 body，必须走全局） */

.sidebar-action-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--spacing-2);
}

.sidebar-action-caret {
  flex-shrink: 0;
}
</style>
