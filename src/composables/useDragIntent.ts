const DRAG_THRESHOLD = 5

/**
 * 创建一个拖拽意图检测 handler
 * mousedown → 监听 mousemove → 超过阈值 → 执行 onDragStart → cleanup
 */
export function createDragHandler(
  onDragStart: (startEvent: MouseEvent) => void,
  shouldIgnore?: (e: MouseEvent) => boolean,
  threshold = DRAG_THRESHOLD
): (e: MouseEvent) => void {
  return (e: MouseEvent) => {
    if (shouldIgnore?.(e)) return
    const startX = e.clientX
    const startY = e.clientY
    let dragStarted = false

    const onMove = (me: MouseEvent) => {
      if (dragStarted) return
      const dx = me.clientX - startX
      const dy = me.clientY - startY
      if (Math.sqrt(dx * dx + dy * dy) > threshold) {
        dragStarted = true
        cleanup()
        onDragStart(e)
      }
    }
    const onUp = () => cleanup()
    const cleanup = () => {
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup', onUp)
    }
    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)
  }
}
