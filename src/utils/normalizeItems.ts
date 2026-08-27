export interface NormalizeItemWithRenameState {
  needs_rename: boolean
}

export interface IndexedNormalizeItem<T> {
  item: T
  index: number
}

export interface NormalizeItemGroups<T> {
  pending: IndexedNormalizeItem<T>[]
  normalized: IndexedNormalizeItem<T>[]
}

/**
 * 展示层按命名规范化状态稳定分组，同时保留原始索引。
 * 原始索引用于继续访问与扫描结果平行的 selections，避免排序后勾选错位。
 */
export function partitionNormalizeItems<T extends NormalizeItemWithRenameState>(items: T[]): NormalizeItemGroups<T> {
  const groups: NormalizeItemGroups<T> = {
    pending: [],
    normalized: [],
  }

  items.forEach((item, index) => {
    const target = item.needs_rename ? groups.pending : groups.normalized
    target.push({ item, index })
  })

  return groups
}
