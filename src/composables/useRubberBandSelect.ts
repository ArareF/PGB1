import { ref } from 'vue'
import type { Ref } from 'vue'

export interface RubberBandRect {
  left: number
  top: number
  right: number
  bottom: number
}

export interface UseRubberBandOptions {
  /** 卡片所在的滚动容器 ref */
  containerRef: Ref<HTMLElement | null>
  /** 卡片元素 CSS 选择器，必须带 data-path 属性 */
  cardSelector: string
  /** 是否允许框选（false 时完全跳过） */
  isEnabled: Ref<boolean>
  /** 框选过程中命中集合变化时回调（传入当前命中的 path Set） */
  onSelect: (paths: Set<string>) => void
}

const MOVE_THRESHOLD = 5 // 像素，超过阈值才算"真正在框选"

export function useRubberBandSelect(options: UseRubberBandOptions) {
  const isSelecting = ref(false)
  const selectionRect = ref<RubberBandRect | null>(null)
  /** 框选刚结束标志：用于屏蔽随后触发的 click 事件 */
  const justFinished = ref(false)

  let startX = 0
  let startY = 0
  let didMove = false

  function onContainerMouseDown(e: MouseEvent) {
    if (!options.isEnabled.value) return
    if (e.button !== 0) return
    // 点在卡片上时跳过，让卡片自己的 mousedown 处理
    if ((e.target as HTMLElement).closest(options.cardSelector)) return

    startX = e.clientX
    startY = e.clientY
    didMove = false

    document.addEventListener('mousemove', onMouseMove)
    document.addEventListener('mouseup', onMouseUp)
  }

  function onMouseMove(e: MouseEvent) {
    const dx = e.clientX - startX
    const dy = e.clientY - startY

    if (!didMove) {
      if (Math.sqrt(dx * dx + dy * dy) < MOVE_THRESHOLD) return
      didMove = true
      isSelecting.value = true
    }

    const left = Math.min(startX, e.clientX)
    const top = Math.min(startY, e.clientY)
    const right = Math.max(startX, e.clientX)
    const bottom = Math.max(startY, e.clientY)
    selectionRect.value = { left, top, right, bottom }

    // 碰撞检测：遍历容器内所有卡片
    const container = options.containerRef.value
    if (!container) {
      onMouseUp()
      return
    }
    const cards = container.querySelectorAll<HTMLElement>(options.cardSelector)
    const hit = new Set<string>()
    for (const card of cards) {
      const r = card.getBoundingClientRect()
      if (r.right > left && r.left < right && r.bottom > top && r.top < bottom) {
        const path = card.dataset.path
        if (path) hit.add(path)
      }
    }
    options.onSelect(hit)
  }

  function onMouseUp() {
    document.removeEventListener('mousemove', onMouseMove)
    document.removeEventListener('mouseup', onMouseUp)

    if (didMove) {
      justFinished.value = true
      // mouseup 后 click 事件同步触发，setTimeout(0) 保证 click handler 能读到 true 再重置
      setTimeout(() => { justFinished.value = false }, 0)
    }

    isSelecting.value = false
    selectionRect.value = null
    didMove = false
  }

  /** 供容器绑定：滚动时立即终止框选，防止起点失效 */
  function onContainerScroll() {
    if (isSelecting.value) onMouseUp()
  }

  return { isSelecting, selectionRect, justFinished, onContainerMouseDown, onContainerScroll }
}
