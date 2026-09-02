import { describe, expect, it } from 'vitest'
import { NAV_ITEMS, navigationAction } from './navigation'

describe('sidebar navigation', () => {
  it('maps every visible item to a concrete action', () => {
    expect(NAV_ITEMS.map(navigationAction)).toEqual([
      'top',
      'focus',
      'thoughts',
      'worklog:open-notes',
      'worklog:open-obsidian',
      'worklog:open-settings',
    ])
  })
})
