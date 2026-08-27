import { ref, computed, type Ref, type ComputedRef } from 'vue'
import { useRubberBandSelect, type RubberBandRect } from './useRubberBandSelect'

interface UseMultiSelectOptions {
  /**
   * 全部可选项的「选中标识」列表，必须与卡片 data-path 同源。
   * 文件类卡片用真实路径；素材卡片用 `materialUid()`（path 对平铺序列帧不唯一）。
   */
  allPaths: ComputedRef<string[]>
  /** 进入多选时的额外副作用 */
  onEnter?: () => void
  /** 退出多选时的额外副作用 */
  onExit?: () => void
  /** 框选配置（可选） */
  rubberBand?: {
    containerRef: Ref<HTMLElement | null>
    cardSelector: string
  }
  /** 始终开启多选模式，不显示开关 */
  alwaysEnabled?: boolean
}

export function useMultiSelect(options: UseMultiSelectOptions) {
  const isMultiSelect = ref(options.alwaysEnabled ?? false)
  const selectedPaths = ref<Set<string>>(new Set())

  const isAllSelected = computed(() => {
    const paths = options.allPaths.value
    return paths.length > 0 && paths.every(p => selectedPaths.value.has(p))
  })

  const selectedItems = computed(() =>
    options.allPaths.value.filter(p => selectedPaths.value.has(p))
  )

  function toggleMultiSelect() {
    if (options.alwaysEnabled) return
    if (isMultiSelect.value) {
      isMultiSelect.value = false
      selectedPaths.value = new Set()
      options.onExit?.()
    } else {
      isMultiSelect.value = true
      options.onEnter?.()
    }
  }

  function toggleSelection(path: string) {
    const s = new Set(selectedPaths.value)
    if (s.has(path)) s.delete(path)
    else s.add(path)
    selectedPaths.value = s
  }

  function toggleSelectAll() {
    if (isAllSelected.value) {
      selectedPaths.value = new Set()
    } else {
      selectedPaths.value = new Set(options.allPaths.value)
    }
  }

  // 框选集成
  let rubberBandResult: ReturnType<typeof useRubberBandSelect> | undefined
  if (options.rubberBand) {
    rubberBandResult = useRubberBandSelect({
      containerRef: options.rubberBand.containerRef,
      cardSelector: options.rubberBand.cardSelector,
      isEnabled: isMultiSelect,
      onSelect: (paths) => { selectedPaths.value = paths },
    })
  }

  return {
    isMultiSelect,
    selectedPaths,
    isAllSelected,
    selectedItems,
    toggleMultiSelect,
    toggleSelection,
    toggleSelectAll,
    // 框选透传（如果配置了）
    isSelecting: rubberBandResult?.isSelecting ?? ref(false),
    selectionRect: rubberBandResult?.selectionRect ?? ref<RubberBandRect | null>(null),
    justFinished: rubberBandResult?.justFinished ?? ref(false),
    onContainerMouseDown: rubberBandResult?.onContainerMouseDown ?? (() => {}),
    onContainerScroll: rubberBandResult?.onContainerScroll ?? (() => {}),
  }
}
