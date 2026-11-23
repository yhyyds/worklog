import { describe, expect, it } from 'vitest'
import { groupLongTermItems, saveLongTermItem, type LongTermItem, type SaveLongTermItem } from './longTerm'

const root: LongTermItem = { id: 'parent', parentId: null, title: '设备选型', workload: null, completed: false }
const child: LongTermItem = { id: 'child', parentId: 'parent', title: '整理规格', workload: 'light', completed: false }

describe('long-term work', () => {
  it('permits optional estimates and tracks child completion without completing the parent', () => {
    const created = saveLongTermItem([root], { ...child, id: null, title: ' 整理规格 ' }, () => 'child')
    const items = saveLongTermItem(created, { ...created[1], workload: 'very_large', completed: true }, () => 'unused')
    expect(items[1]).toEqual({ ...child, workload: 'very_large', completed: true })
    expect(groupLongTermItems(items)).toEqual([{ item: root, children: [items[1]], completedChildren: 1 }])
    expect(created[1].completed).toBe(false)
  })

  it('rejects over-three-hour estimates, estimates on parents, missing parents and a third level', () => {
    const newItem: SaveLongTermItem = { ...child, id: null, workload: null }
    const save = (changes: Partial<SaveLongTermItem>) => saveLongTermItem([root, child], { ...newItem, ...changes }, () => 'new')
    expect(() => save({ parentId: null, workload: 'light' })).toThrow('只有子任务')
    expect(() => save({ workload: '4h' as 'light' })).toThrow('超过 3 小时')
    expect(() => save({ parentId: 'child' })).toThrow('两级上限')
    expect(() => save({ parentId: 'missing' })).toThrow('父项不存在')
    expect(() => save({ id: 'child', parentId: null })).toThrow('父项不匹配')
  })
})
