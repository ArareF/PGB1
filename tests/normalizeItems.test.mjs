import test from 'node:test'
import assert from 'node:assert/strict'
import { partitionNormalizeItems } from '../src/utils/normalizeItems.ts'

test('按命名规范化状态稳定分组并保留原始索引', () => {
  const items = [
    { name: '已规范 A', needs_rename: false },
    { name: '待规范 A', needs_rename: true },
    { name: '已规范 B', needs_rename: false },
    { name: '待规范 B', needs_rename: true },
  ]

  const groups = partitionNormalizeItems(items)

  assert.deepEqual(groups.pending.map(({ item, index }) => [item.name, index]), [
    ['待规范 A', 1],
    ['待规范 B', 3],
  ])
  assert.deepEqual(groups.normalized.map(({ item, index }) => [item.name, index]), [
    ['已规范 A', 0],
    ['已规范 B', 2],
  ])
})
