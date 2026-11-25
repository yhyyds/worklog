export const NAV_ITEMS = ['我的一天', '待办与历史', '随笔', 'Obsidian', '日程集成', '周报', '设置'] as const

export type NavItem = typeof NAV_ITEMS[number]
export type NavigationAction = 'top' | 'worklog:open-inbox' | 'worklog:open-notes' | 'worklog:open-obsidian' | 'worklog:open-integrations' | 'worklog:open-weekly-report' | 'worklog:open-settings'

const actions: Record<NavItem, NavigationAction> = {
  '我的一天': 'top',
  '待办与历史': 'worklog:open-inbox',
  '随笔': 'worklog:open-notes',
  'Obsidian': 'worklog:open-obsidian',
  '日程集成': 'worklog:open-integrations',
  '周报': 'worklog:open-weekly-report',
  '设置': 'worklog:open-settings',
}

export const navigationAction = (item: NavItem) => actions[item]
