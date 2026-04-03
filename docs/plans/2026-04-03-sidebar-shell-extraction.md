# SidebarShell 提取重构方案

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 FileDetailSidebar.vue 和 TaskPage.vue 中重复的侧边栏外壳（容器、拖拽调宽、全屏 FLIP、进出场动画、宽度持久化）提取为 `SidebarShell.vue` 组件，内容区通过 slot 注入。

**Architecture:** SidebarShell 封装所有外壳逻辑（~150 行 JS + ~200 行 CSS），暴露 `<slot>` 给内容区 + `<slot name="actions">` 给底部按钮。FileDetailSidebar 和 TaskPage 各自只保留内容渲染逻辑。两处进出场动画名统一为 `sidebar`。

**Tech Stack:** Vue 3 Composition API, CSS custom properties, Teleport, Transition

---

## 重复代码清单（提取前）

| 功能 | FileDetailSidebar.vue | TaskPage.vue |
|------|----------------------|-------------|
| 容器样式 | `.file-detail-sidebar` L485-500 | `.detail-sidebar` L1489-1504 |
| resize 拖拽 | L236-271 | L757-785 |
| 全屏 FLIP | L171-222 | L787-836 |
| 宽度持久化 | L238-241 | L758-760 |
| Esc 退出全屏 | L224-228 | L832-836 |
| header 模板 | L288-290 | L1180-1182 |
| resize-handle 模板 | L285 | L1179 |
| 进出场动画 CSS | L522-537 | L1526-1541 |
| 全屏模式 CSS | L783-857 | L1586-1627 |
| sidebar-body CSS | L555-563 | L1557-1565 |

共计 ~350 行重复代码。

---

### Task 1: 创建 SidebarShell.vue

**Files:**
- Create: `src/components/SidebarShell.vue`

**Step 1: 创建组件文件**

从 FileDetailSidebar.vue 提取外壳逻辑，内容区用 slot 替代。

```vue
<script setup lang="ts">
import { ref, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'

const props = withDefaults(defineProps<{
  /** 控制侧边栏显示/隐藏 */
  show: boolean
  /** 标题文字 */
  title: string
  /** 宽度百分比 */
  widthPercent?: number
  /** Teleport 目标选择器 */
  teleportTarget?: string
  /** 禁用 Teleport */
  teleportDisabled?: boolean
}>(), {
  teleportTarget: '#content-row',
})

const emit = defineEmits<{
  'update:widthPercent': [value: number]
}>()

// ─── 全屏 ──────────────────────────────────────────

const isFullscreen = ref(false)
const fsLeft = ref('0px')
const sidebarEl = ref<HTMLElement | null>(null)

async function toggleFullscreen() {
  const el = sidebarEl.value

  if (!isFullscreen.value) {
    const mainEl = document.querySelector('.main-content') as HTMLElement | null
    const crEl = document.getElementById('content-row') as HTMLElement | null
    if (mainEl && crEl) {
      fsLeft.value = `${mainEl.getBoundingClientRect().left - crEl.getBoundingClientRect().left}px`
    } else {
      fsLeft.value = '0px'
    }
  }

  const startRect = el?.getBoundingClientRect()
  isFullscreen.value = !isFullscreen.value
  await nextTick()

  if (!el || !startRect) return
  const endRect = el.getBoundingClientRect()
  const dx = startRect.left - endRect.left
  const dy = startRect.top - endRect.top
  const scaleX = startRect.width / endRect.width
  const scaleY = startRect.height / endRect.height

  el.style.transformOrigin = 'top left'
  el.style.transform = `translate(${dx}px, ${dy}px) scale(${scaleX}, ${scaleY})`
  el.style.transition = 'none'
  void el.offsetWidth

  el.style.transition = `transform var(--duration-normal) var(--ease-out)`
  el.style.transform = ''

  el.addEventListener('transitionend', () => {
    el.style.transform = ''
    el.style.transition = ''
    el.style.transformOrigin = ''
  }, { once: true })
}

function onGlobalKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && isFullscreen.value) {
    isFullscreen.value = false
  }
}

onMounted(() => window.addEventListener('keydown', onGlobalKeyDown))
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onGlobalKeyDown)
  resizeCleanup?.()
})

// ─── 拖拽调整宽度 ────────────────────────────────────

const isResizing = ref(false)
const SIDEBAR_WIDTH_KEY = 'pgb1-sidebar-width'
const savedWidth = parseFloat(localStorage.getItem(SIDEBAR_WIDTH_KEY) || '')
const currentWidth = ref(isFinite(savedWidth) ? savedWidth : (props.widthPercent ?? 30))

watch(() => props.widthPercent, (v) => {
  if (v != null) currentWidth.value = v
})

let resizeCleanup: (() => void) | null = null

function startResize(e: MouseEvent) {
  e.preventDefault()
  isResizing.value = true
  const startX = e.clientX
  const startWidth = currentWidth.value

  function onMouseMove(ev: MouseEvent) {
    const windowWidth = window.innerWidth
    const deltaPercent = ((startX - ev.clientX) / windowWidth) * 100
    currentWidth.value = Math.min(60, Math.max(20, startWidth + deltaPercent))
    emit('update:widthPercent', currentWidth.value)
  }
  function onMouseUp() {
    isResizing.value = false
    localStorage.setItem(SIDEBAR_WIDTH_KEY, String(currentWidth.value))
    window.removeEventListener('mousemove', onMouseMove)
    window.removeEventListener('mouseup', onMouseUp)
    resizeCleanup = null
  }
  window.addEventListener('mousemove', onMouseMove)
  window.addEventListener('mouseup', onMouseUp)
  resizeCleanup = onMouseUp
}

// 暴露给父组件（模板内 slot 可能需要）
defineExpose({ isFullscreen, toggleFullscreen })
</script>

<template>
  <Teleport :to="teleportTarget" :disabled="teleportDisabled">
    <Transition name="sidebar-shell">
      <div
        v-if="show"
        ref="sidebarEl"
        class="sidebar-shell"
        :class="{ 'is-resizing': isResizing, 'is-fullscreen': isFullscreen }"
        :style="isFullscreen ? { left: fsLeft } : { width: currentWidth + '%' }"
      >
        <div class="sidebar-shell__resize" @mousedown="startResize" />

        <div class="sidebar-shell__header">
          <span class="sidebar-shell__title">{{ title }}</span>
        </div>

        <div class="sidebar-shell__body">
          <slot :is-fullscreen="isFullscreen" :toggle-fullscreen="toggleFullscreen" />
        </div>

        <div v-if="$slots.actions" class="sidebar-actions">
          <slot name="actions" />
        </div>

        <slot name="overlay" />
      </div>
    </Transition>
  </Teleport>
</template>
```

**Step 2: 编写 SidebarShell CSS**

将 FileDetailSidebar.vue 和 TaskPage.vue 中重复的外壳 CSS 统一写入 SidebarShell.vue 的 `<style>` 块（非 scoped，因为 Teleport）。

CSS class 映射：
- `.file-detail-sidebar` / `.detail-sidebar` → `.sidebar-shell`
- `.resize-handle` → `.sidebar-shell__resize`
- `.sidebar-header` → `.sidebar-shell__header`
- `.sidebar-title` → `.sidebar-shell__title`
- `.sidebar-body` → `.sidebar-shell__body`
- 进出场动画 `file-sidebar-*` / `sidebar-*` → `sidebar-shell-*`
- 全屏 `.is-fullscreen` 保持不变

注意事项：
- `.sidebar-shell__body` 的 `padding-bottom` 仅当有 `actions` slot 时才需要。可通过 CSS `:has(.sidebar-actions)` 或者 prop 控制。简单做法：actions slot 有内容时加 class。
- TaskPage 的 sidebar-body 有额外的 `padding-bottom: calc(var(--button-md-height) + var(--spacing-4) * 2)`（因为 actions 悬浮定位），这个在 SidebarShell 中统一处理。

**Step 3: 验证组件可独立渲染**

手动在任一页面临时引入 SidebarShell，确认 Teleport + 动画 + 拖拽 + 全屏都正常工作。

---

### Task 2: 重构 FileDetailSidebar.vue 使用 SidebarShell

**Files:**
- Modify: `src/components/FileDetailSidebar.vue`

**改动要点：**

1. **删除**：全屏逻辑（L171-228）、resize 逻辑（L236-271）、Esc 监听（L224-234）、宽度持久化（L238-241）
2. **删除**：外壳 CSS（`.file-detail-sidebar` 容器、`.resize-handle`、`.sidebar-header`、`.sidebar-title`、`.sidebar-body`、进出场动画、全屏模式外壳样式）
3. **保留**：所有内容渲染（预览区、基本信息、版本列表、笔记、弹窗）+ 内容相关 CSS（`.preview-image-wrap`、`.preview-psd-wrap`、`.preview-video-wrap`、版本卡片等）
4. **模板改造**：

```vue
<template>
  <SidebarShell
    ref="shellRef"
    :show="!!file"
    :title="$t('fileDetail.detail')"
    :width-percent="widthPercent"
    :teleport-target="teleportTarget"
    :teleport-disabled="teleportDisabled"
    @update:width-percent="emit('update:widthPercent', $event)"
  >
    <template #default="{ isFullscreen, toggleFullscreen }">
      <!-- 所有预览/信息/版本/笔记内容不变，只是全屏按钮改用 toggleFullscreen -->
    </template>

    <template v-if="allowActions" #actions>
      <button class="sidebar-action-btn" @click="openRenameDialog">{{ $t('common.rename') }}</button>
      <button class="sidebar-action-btn danger" @click="openDeleteDialog">{{ $t('common.delete') }}</button>
    </template>

    <template #overlay>
      <!-- 内联弹窗 overlay -->
    </template>
  </SidebarShell>
</template>
```

5. **全屏相关 CSS 调整**：全屏时隐藏 section/actions 的规则改为 `.sidebar-shell.is-fullscreen` 前缀，预览铺满规则同理。这些是"内容对全屏的响应"，属于内容组件的职责，保留在 FileDetailSidebar.vue 中。

---

### Task 3: 重构 TaskPage.vue 使用 SidebarShell

**Files:**
- Modify: `src/views/TaskPage.vue`

**改动要点：**

1. **删除**：L757-785（resize）、L787-836（全屏 FLIP）、L832-836（Esc 监听）、`sidebarEl` ref、`isResizing` ref、`isFullscreen` ref、`fsLeft` ref、`formatSize` 函数（改用 `import { formatSize } from '../utils/format'`，和 FileDetailSidebar 一致）
2. **删除**：L1487-1753 的外壳 CSS（`.detail-sidebar`、`.resize-handle`、`.sidebar-header`、`.sidebar-title`、`.sidebar-body`、进出场动画、全屏模式外壳样式、`.sidebar-section`、`.section-title`、`.info-list`、`.info-row`、`.info-label`、`.info-value`、`.version-list`、`.version-card`、`.version-*`）。其中 `.sidebar-section` 等在 FileDetailSidebar 中也有重复定义，统一到 SidebarShell 或 design-system.css。
3. **保留**：`.sidebar-preview`、`.sidebar-no-preview`、帧率编辑 CSS（`.fps-*`）

4. **模板改造**：

```vue
<!-- 替换 L1168-1353 -->
<SidebarShell
  ref="materialShellRef"
  :show="!!selectedMaterial"
  :title="$t('task.detail')"
  @update:width-percent="v => sidebarWidthPercent = v"
>
  <template #default="{ isFullscreen, toggleFullscreen }">
    <div class="sidebar-preview">
      <!-- 预览内容不变 -->
      <button class="preview-fullscreen-btn" @click="toggleFullscreen">...</button>
    </div>
    <!-- 基本信息、笔记、版本列表不变 -->
  </template>

  <template #actions>
    <!-- 操作按钮不变 -->
  </template>

  <template #overlay>
    <!-- 弹窗不变 -->
  </template>
</SidebarShell>
```

---

### Task 4: 处理共享子样式（sidebar-section / info / version）

**分析**：`.sidebar-section`、`.section-title`、`.info-list`、`.info-row`、`.info-label`、`.info-value`、`.version-list`、`.version-card`、`.version-*` 在两处都有，样式几乎一致（微小差异：TaskPage 的 `.section-title` 多 `padding-bottom + border-bottom`，`.info-row` 用 `baseline` 对齐）。

**方案**：这些属于"侧边栏内通用信息展示样式"，统一放入 SidebarShell.vue 的全局 `<style>` 块（以 `.sidebar-shell` 为命名空间前缀），或放入 design-system.css。两处消费者直接使用同一套 class。

差异点通过额外 class 覆盖：TaskPage 的 `.section-title` 如需 border-bottom 可在 TaskPage scoped 样式中加。

---

### Task 5: 验证 + 回归测试

**检查清单：**

1. **FileDetailSidebar 消费者**（MaterialsPage / GameIntroPage / FolderBrowserDialog / TaskPage 预览视频侧边栏）：
   - 侧边栏打开/关闭动画正常
   - 拖拽调宽正常，松手后 localStorage 持久化
   - 全屏 FLIP 动画正常（图片/视频/PSD/PDF）
   - Esc 退出全屏
   - 重命名/删除弹窗正常
   - `teleportDisabled` 模式（FolderBrowserDialog）正常

2. **TaskPage 素材侧边栏**：
   - 侧边栏打开/关闭动画正常
   - 拖拽调宽正常
   - 全屏正常（序列帧/静帧）
   - 帧率内联编辑正常
   - 重命名/删除弹窗正常
   - 操作按钮（修改 TPS / 重命名 / 删除）正常

3. **两侧边栏互斥**（TaskPage 中素材侧边栏 vs 预览视频侧边栏）

4. **预览高度限制**：`--sidebar-preview-max-height: 50vh` 在两处均生效，全屏时解除

---

### Task 6: 更新 CODE_INDEX.md

新增 `SidebarShell.vue` 条目，更新 `FileDetailSidebar.vue` 和 `TaskPage.vue` 的描述（移除外壳职责说明，标注使用 SidebarShell）。

---

## 预期效果

- 外壳代码从 ~350 行重复 → 0 行重复（SSOT 在 SidebarShell.vue）
- 未来调整侧边栏外壳（宽度范围、动画、全屏行为、预览高度限制）只改一处
- FileDetailSidebar.vue 和 TaskPage 各减少 ~150 行
