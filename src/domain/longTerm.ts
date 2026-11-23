export const WORKLOAD_LABELS = {
  light: '轻松（<1 h）',
  large: '较大（1–2 h）',
  very_large: '极大（2–3 h）',
} as const

export type Workload = keyof typeof WORKLOAD_LABELS

export interface LongTermItem {
  id: string
  parentId: string | null
  title: string
  workload: Workload | null
  completed: boolean
}

export type SaveLongTermItem = Omit<LongTermItem, 'id'> & { id: string | null }

export function validateWorkload(isChild: boolean, workload: Workload | null | undefined) {
  if (workload == null) return
  if (!isChild) throw new Error('只有子任务可以设置工作量')
  if (!Object.hasOwn(WORKLOAD_LABELS, workload)) throw new Error('无效工作量；超过 3 小时请拆成更小的子任务')
}

export function saveLongTermItem(items: LongTermItem[], input: SaveLongTermItem, newId: () => string): LongTermItem[] {
  const title = input.title.trim()
  if (!title || [...title].length > 200) throw new Error('长期任务内容须为 1–200 字')
  validateWorkload(input.parentId !== null, input.workload)
  if (input.parentId !== null && !items.some((item) => item.id === input.parentId && item.parentId === null)) {
    throw new Error('长期任务父项不存在，或已达到两级上限')
  }
  if (input.id !== null && !items.some((item) => item.id === input.id && item.parentId === input.parentId)) {
    throw new Error('长期任务不存在或父项不匹配')
  }
  const next = { ...input, title, id: input.id ?? newId() }
  return input.id === null ? [...items, next] : items.map((item) => item.id === input.id ? next : item)
}

export function groupLongTermItems(items: LongTermItem[]) {
  const childrenByParent = new Map<string, LongTermItem[]>()
  const sorted = [...items].sort((a, b) => Number(a.completed) - Number(b.completed))
  for (const item of sorted) {
    if (item.parentId === null) continue
    const children = childrenByParent.get(item.parentId) ?? []
    children.push(item)
    childrenByParent.set(item.parentId, children)
  }
  return sorted.filter((item) => item.parentId === null).map((item) => {
    const children = childrenByParent.get(item.id) ?? []
    return { item, children, completedChildren: children.filter((child) => child.completed).length }
  })
}
