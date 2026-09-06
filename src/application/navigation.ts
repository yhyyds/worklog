export const NAV_ITEMS = ['我的一天', '待办箱', '随笔', 'Obsidian', '成长', '周报', '设置'] as const

export type NavItem = typeof NAV_ITEMS[number]
export type NavigationAction = 'top' | 'worklog:open-inbox' | 'worklog:open-notes' | 'worklog:open-obsidian' | 'worklog:open-growth' | 'worklog:open-weekly-report' | 'worklog:open-settings'

const actions: Record<NavItem, NavigationAction> = {
  '我的一天': 'top',
  '待办箱': 'worklog:open-inbox',
  '随笔': 'worklog:open-notes',
  'Obsidian': 'worklog:open-obsidian',
  '成长': 'worklog:open-growth',
  '周报': 'worklog:open-weekly-report',
  '设置': 'worklog:open-settings',
}

export const navigationAction = (item: NavItem) => actions[item]
