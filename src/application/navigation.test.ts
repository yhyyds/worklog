import { describe, expect, it } from 'vitest'
import { NAV_ITEMS, navigationAction } from './navigation'

describe('sidebar navigation', () => {
  it('keeps focus and work thoughts inside My Day', () => {
    expect(NAV_ITEMS).toEqual(['我的一天', '随笔', 'Obsidian', '设置'])
    expect(NAV_ITEMS.map(navigationAction)).toEqual([
      'top',
      'worklog:open-notes',
      'worklog:open-obsidian',
      'worklog:open-settings',
    ])
  })
})
