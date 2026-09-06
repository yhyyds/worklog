import { describe, expect, it } from 'vitest'
import { NAV_ITEMS, navigationAction } from './navigation'

describe('sidebar navigation', () => {
  it('keeps focus and work thoughts inside My Day', () => {
    expect(NAV_ITEMS).toEqual(['我的一天', '待办箱', '随笔', 'Obsidian', '成长', '周报', '设置'])
    expect(NAV_ITEMS.map(navigationAction)).toEqual([
      'top',
      'worklog:open-inbox',
      'worklog:open-notes',
      'worklog:open-obsidian',
      'worklog:open-growth',
      'worklog:open-weekly-report',
      'worklog:open-settings',
    ])
  })
})
