/** 优先度排序权重 SSOT（HomePage / ProjectPage 共用）
 *  排序语义：急(high) → 高(medium) → 未设置(null) → 停(low) */
const PRIORITY_RANK: Record<string, number> = { high: 0, medium: 1, low: 3 }

/** 未设置优先度的排序权重（介于 medium 与 low 之间） */
const PRIORITY_RANK_NONE = 2

/** 取优先度排序权重：null/未知值统一按"未设置"处理 */
export function priorityRank(priority: string | null | undefined): number {
  return priority ? (PRIORITY_RANK[priority] ?? PRIORITY_RANK_NONE) : PRIORITY_RANK_NONE
}
