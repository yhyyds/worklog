export const NAV_ITEMS = ['我的一天', '随笔', 'Obsidian', '设置'] as const

export type NavItem = typeof NAV_ITEMS[number]
export type NavigationAction = 'top' | 'worklog:open-notes' | 'worklog:open-obsidian' | 'worklog:open-settings'

const actions: Record<NavItem, NavigationAction> = {
  '我的一天': 'top',
  '随笔': 'worklog:open-notes',
  'Obsidian': 'worklog:open-obsidian',
  '设置': 'worklog:open-settings',
}

export const navigationAction = (item: NavItem) => actions[item]
