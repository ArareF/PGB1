<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import TitleBar from '../components/TitleBar.vue'
import Sidebar from '../components/Sidebar.vue'
import WindowControls from '../components/WindowControls.vue'
import { useNavigation } from '../composables/useNavigation'

const { t } = useI18n()
const { moreMenuItems, routeDirection, setRouteDirection } = useNavigation()
const showMoreMenu = ref(false)
const moreBtnRef = ref<HTMLElement | null>(null)
const dropdownPos = ref({ top: '0px', right: '0px' })

function toggleMoreMenu() {
  showMoreMenu.value = !showMoreMenu.value
  if (showMoreMenu.value && moreBtnRef.value) {
    const rect = moreBtnRef.value.getBoundingClientRect()
    dropdownPos.value = {
      top: `${rect.bottom + 6}px`,
      right: `${window.innerWidth - rect.right}px`,
    }
  }
}

// 路由方向检测：路径深度变浅 = 返回
const router = useRouter()
const route = useRoute()
router.beforeEach((to, from) => {
  const toDepth = to.path.split('/').filter(Boolean).length
  const fromDepth = from.path.split('/').filter(Boolean).length
  setRouteDirection(toDepth < fromDepth ? 'back' : 'forward')
})

/** 全局追加「程序设置」——设置页本身除外 */
const allMoreMenuItems = computed(() => {
  const items = [...moreMenuItems.value]
  if (route.name !== 'settings') {
    items.push({ id: 'settings', label: t('home.appSettings'), handler: () => router.push({ name: 'settings' }) })
  }
  return items
})
</script>

<template>
  <div class="main-layout">
    <!-- 顶部行：标题栏 + 右侧两行区域（窗口控制 + 更多菜单） -->
    <div class="top-row">
      <TitleBar class="title-bar-area" />
      <div class="top-right-column">
        <WindowControls class="window-controls-area" />
        <div class="more-menu-wrapper">
          <button ref="moreBtnRef" class="more-menu-btn" :title="$t('common.more')" @click="toggleMoreMenu">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
              <circle cx="3" cy="8" r="1.5" />
              <circle cx="8" cy="8" r="1.5" />
              <circle cx="13" cy="8" r="1.5" />
            </svg>
          </button>
          <!-- 下拉菜单 — Teleport to body 以安全使用 backdrop-filter -->
          <Teleport to="body">
            <Transition name="dropdown">
              <div v-if="showMoreMenu" class="more-dropdown" :style="{ top: dropdownPos.top, right: dropdownPos.right }">
                <button
                  v-for="item in allMoreMenuItems"
                  :key="item.id"
                  class="dropdown-item"
                  @click="showMoreMenu = false; item.handler()"
                >
                  {{ item.label }}
                </button>
              </div>
            </Transition>
            <!-- 点击外部关闭遮罩 -->
            <div v-if="showMoreMenu" class="more-menu-overlay" @click="showMoreMenu = false" />
          </Teleport>
        </div>
      </div>
    </div>

    <!-- 内容行：侧边栏 + 主功能区 -->
    <div id="content-row" class="content-row">
      <Sidebar class="sidebar-area" />
      <main class="main-content glass-medium">
        <RouterView v-slot="{ Component }">
          <Transition :name="routeDirection === 'back' ? 'page-back' : 'page-forward'" mode="out-in">
            <div :key="$route.path" class="page-wrapper">
              <component :is="Component" />
            </div>
          </Transition>
        </RouterView>
      </main>
    </div>
  </div>
</template>

<style scoped>
.main-layout {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  /* 透明背景 — 系统级 Acrylic 效果作为最底层 */
  background: transparent;
  padding: var(--spacing-3);
  gap: var(--spacing-5);
  /* 最外层圆角 — 与悬浮岛一致 */
  border-radius: var(--radius-floating-island);
  overflow: hidden;
  position: relative;
}

/* 噪点纹理叠加层 — 冷蓝偏移微噪点 */
.main-layout::before {
  content: '';
  position: fixed;
  inset: 0;
  pointer-events: none;
  z-index: 9998;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='200' height='200'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='4' stitchTiles='stitch'/%3E%3CfeColorMatrix type='saturate' values='0'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='1'/%3E%3C/svg%3E");
  opacity: 0.03;
  mix-blend-mode: overlay;
}

/* 顶部行 */
.top-row {
  display: flex;
  align-items: flex-end;
  gap: var(--spacing-3);
  flex-shrink: 0;
}

.title-bar-area {
  flex: 1;
  min-width: 0;
}

/* 右侧两行区域：窗口控制 + 更多菜单 */
.top-right-column {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-3);
  flex-shrink: 0;
  align-self: stretch;
}

.window-controls-area {
  flex-shrink: 0;
}

/* 更多菜单 — 悬浮操作按钮，宽度跟随窗口控制行 */
.more-menu-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: var(--floating-action-height);
  padding: 0 var(--floating-action-padding-x);
  /* 手动复刻 glass-medium 视觉，但不用 backdrop-filter：
     在 Tauri + Windows Acrylic 下 backdrop-filter 会向周围扩散渲染层，
     覆盖到侧边栏和标题区域，产生有边界的灰色溢出。 */
  background: var(--glass-medium-bg);
  border: var(--glass-medium-border);
  box-shadow: var(--glass-medium-shadow);
  border-radius: var(--floating-action-radius);
  color: var(--text-secondary);
  cursor: pointer;
  transition: var(--transition-all);
  /* 不参与窗口拖拽 */
  -webkit-app-region: no-drag;
}

.more-menu-btn:hover {
  color: var(--text-primary);
  border-color: var(--border-medium);
  transform: translateY(-2px);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.25);
}

.more-menu-btn:active {
  transform: translateY(0);
  box-shadow: none;
}

.more-menu-btn svg {
  width: var(--floating-action-icon-size);
  height: var(--floating-action-icon-size);
}

/* 更多菜单容器 — 推到底部，和标题栏底边对齐 */
.more-menu-wrapper {
  position: relative;
  width: 100%;
  margin-top: auto;
}

/* .more-dropdown / .dropdown-item / .more-menu-overlay → 非 scoped 块（Teleport to body） */

/* 内容行 — 水平间距对齐上方垂直间距 */
.content-row {
  display: flex;
  gap: var(--spacing-5);
  flex: 1;
  min-height: 0;
}

.sidebar-area {
  flex-shrink: 0;
}

.main-content {
  flex: 1;
  min-width: 0;
  border-radius: var(--floating-main-radius);
  padding: var(--floating-main-padding);
  position: relative;
  overflow-x: clip;
  overflow-y: hidden;
}

/* 路由包装层 — 固定高度填满 main-content，各页面自己处理内部滚动 */
.page-wrapper {
  width: 100%;
  height: 100%;
}

/* .dropdown-*  transition → 非 scoped 块（Teleport to body） */

/* 路由页面切换动画 — 前进（从右滑入） */
.page-forward-enter-active {
  transition: var(--transition-route-in);
}
.page-forward-leave-active {
  transition: var(--transition-route-out);
}
.page-forward-enter-from {
  transform: translateX(100px);
  opacity: 0;
}
.page-forward-leave-to {
  transform: translateX(-100px);
  opacity: 0;
}

/* 路由页面切换动画 — 返回（从左滑入） */
.page-back-enter-active {
  transition: var(--transition-route-in);
}
.page-back-leave-active {
  transition: var(--transition-route-out);
}
.page-back-enter-from {
  transform: translateX(-100px);
  opacity: 0;
}
.page-back-leave-to {
  transform: translateX(100px);
  opacity: 0;
}
</style>

<!-- 非 scoped — Teleport to body 的更多菜单 -->
<style>
/* 下拉菜单 — Teleport to body，手动 glass（无 backdrop-filter）；
   与 TitleBar glass-medium 视觉重叠，backdrop-filter 会产生黑色伪影 */
.more-dropdown {
  position: fixed;
  min-width: calc(var(--floating-action-height) * 3);
  padding: var(--spacing-2);
  border-radius: var(--floating-action-radius);
  z-index: var(--z-dropdown);
  background: var(--dropdown-menu-bg);
  border: var(--glass-medium-border);
  box-shadow: var(--glass-medium-shadow);
}

.dropdown-item {
  display: flex;
  align-items: center;
  width: 100%;
  padding: var(--spacing-2) var(--spacing-3);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: var(--transition-bg);
  white-space: nowrap;
}

.dropdown-item:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

/* 点击外部关闭遮罩 — 全屏透明 */
.more-menu-overlay {
  position: fixed;
  inset: 0;
  z-index: calc(var(--z-dropdown) - 1);
}

/* 下拉菜单进出动画 */
.dropdown-enter-active,
.dropdown-leave-active {
  transition: var(--transition-dropdown);
  transform-origin: top right;
}
.dropdown-enter-from {
  transform: translateY(-6px) scale(0.95);
  opacity: 0.8;
}
.dropdown-leave-to {
  transform: translateY(-6px) scale(0.95);
  opacity: 0;
}
</style>
