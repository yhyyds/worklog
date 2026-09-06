import { habitLabels, habitSymbols, minutesText, priorityGroups, rateText, streakText, taskLabels, trend, trophies, weekdays, type WeeklyReportData } from '../domain/report'
import { goalRate, type CategorySummary } from '../domain/sharing'

// Measure and render the same layout, so longer titles and additional habits
// expand the image instead of being truncated by a fixed-height template.
export async function renderReportPng(report: WeeklyReportData, categories: CategorySummary[] = []): Promise<Blob> {
  await document.fonts.ready
  const canvas = document.createElement('canvas'); canvas.width = 1200; canvas.height = 1
  const context = canvas.getContext('2d'); if (!context) throw new Error('无法创建图片画布')
  const height = Math.ceil(drawReport(context, report, categories))
  if (height > 30000) throw new Error('本周记录过多，无法保存为单张图片。请在周报中查看完整明细。')
  canvas.height = height
  drawReport(context, report, categories)
  return new Promise((resolve, reject) => canvas.toBlob(blob => blob ? resolve(blob) : reject(new Error('图片生成失败')), 'image/png'))
}

function drawReport(ctx: CanvasRenderingContext2D, r: WeeklyReportData, categories: CategorySummary[]): number {
  const width = 1040; const x = 80; let y = 80
  ctx.fillStyle = '#f4f7f1'; ctx.fillRect(0, 0, 1200, ctx.canvas.height)
  ctx.textBaseline = 'top'
  function text(value: string, size = 22, color = '#55695d', weight = 400) {
    ctx.font = `${weight} ${size}px "Microsoft YaHei", sans-serif`; ctx.fillStyle = color
    const lines = wrapLines(ctx, value, width)
    for (const line of lines) { ctx.fillText(line, x, y); y += size * 1.6 }
    return lines.length
  }
  function section(title: string, subtitle = '') {
    y += 38; ctx.fillStyle = '#d9e4db'; ctx.fillRect(x, y, width, 1); y += 24
    text(title, 30, '#284735', 700); if (subtitle) text(subtitle, 19, '#7b887f'); y += 12
  }
  function table(headers: string[], rows: string[][]) {
    const colWidth = width / headers.length
    for (const [index, row] of [headers, ...rows].entries()) {
      ctx.font = `${index === 0 ? 700 : 400} 20px "Microsoft YaHei", sans-serif`
      const wrapped = row.map(cell => wrapLines(ctx, cell, colWidth - 24))
      const height = Math.max(...wrapped.map(lines => lines.length)) * 32 + 22
      ctx.fillStyle = index === 0 ? '#e1ebe2' : index % 2 ? '#ffffff' : '#edf3ec'
      ctx.fillRect(x, y, width, height); ctx.fillStyle = '#42594b'
      wrapped.forEach((lines, col) => lines.forEach((line, j) => ctx.fillText(line, x + col * colWidth + 12, y + 11 + j * 32)))
      y += height
    }
  }
  text('WORKLOG / 一周小笺', 22, '#577563', 700); y += 12
  text(r.headline, 42, '#263c2e', 700)
  text(`${r.weekStart} — ${r.weekEnd}`, 22); y += 12
  if (r.observation) text(r.observation)
  text(`截至 ${r.daily[r.comparisonDays - 1].date} · 与上周同期比较`, 18, '#7b887f'); y += 24
  table(['完成事项', '专注时间', '计划完成率', '习惯打卡'], [[`${r.completedTasks}/${r.plannedTasks}`, minutesText(r.focusMinutes), rateText(r.completedTasks, r.plannedTasks), `${r.habitEffective}/${r.habitTotal}`]])
  y += 12; text(`完成事项：${trend(r.completedChangePercent)}；专注时间：${trend(r.focusChangePercent)}`, 19)

  if (categories.length) { section('分类完成情况', '本周安排'); for (const category of categories) { const c = category.counts; text(category.name, 26, category.color, 700); table(['习惯打卡', '目标计划', '本周最长连续', '中断'], [[`${c.habitDone}/${c.habitReviewed} · ${rateText(c.habitDone, c.habitReviewed)}`, goalRate(c) === null ? '—' : `${goalRate(c)}%`, `${c.habitBestStreak} 天`, `${c.habitBreaks} 次`]]); text(`目标必做 ${c.goalDone}/${c.goalRequired} · 附加 ${c.goalOptionalDone}/${c.goalOptional}`, 19); y += 16 } }
  section('习惯记录', `${r.habits.filter(h => h.breaks > 0).length} 个习惯中断 · 共 ${r.habits.reduce((n, h) => n + h.breaks, 0)} 次中断 · ${r.habitPending} 次待回顾`)
  text(Object.keys(habitLabels).map(key => `${habitSymbols[key]} ${habitLabels[key]}`).join('    '), 18)
  if (!r.habits.length) text('本周没有打卡项。')
  for (const h of r.habits) {
    y += 20; text(h.title, 26, '#284735', 700)
    table(weekdays.map(day => `周${day}`), [h.days.map(status => habitSymbols[status])])
    y += 12
    text(`连续完成 ${streakText(h)} · 本周最长 ${h.weekLongestStreak} 天 · 历史最长 ${h.longestStreak} 天 · 本周中断 ${h.breaks} 次`, 19)
    text(`本周完成 ${h.completed} 次，未完成 ${h.missed + h.prerequisiteMissed} 次；上周同期 ${h.previousReviewed ? `${h.previousCompleted}/${h.previousReviewed} 次` : '无记录'}`, 19)
    if (h.prerequisiteMissed) text(`有 ${h.prerequisiteMissed} 天做到了这一项，但前置打卡未完成。`, 19, '#956d38')
  }
  section('专注与每日安排')
  const f = r.focusDetail
  text(`完成 ${f.completedSessions} 轮 · 中途结束 ${f.abandonedSessions} 轮 · 平均 ${f.averageMinutes} 分钟 · 最长 ${f.longestMinutes} 分钟`, 21)
  text(`暂停 ${f.pauses} 次 · 切换任务 ${f.switches} 次 · ${r.daily.filter(d => d.focusMinutes > 0).length} 天有专注记录`, 21); y += 18
  const max = Math.max(1, ...r.daily.map(d => d.focusMinutes)); const chartY = y
  r.daily.forEach((day, i) => {
    const left = x + i * width / 7 + 26; const barHeight = day.focusMinutes / max * 180
    ctx.fillStyle = '#e2eae0'; ctx.fillRect(left, chartY, 68, 180)
    ctx.fillStyle = '#78a78a'; ctx.fillRect(left, chartY + 180 - barHeight, 68, barHeight)
    ctx.fillStyle = '#526b5b'; ctx.font = '18px "Microsoft YaHei"'; ctx.fillText(`周${weekdays[i]}`, left, chartY + 196)
  }); y += 240
  table(['日期', '完成 / 计划', '完成率', '专注', '打卡完成'], r.daily.map((d, i) => [d.date.slice(5), i < r.comparisonDays ? `${d.completedTasks}/${d.plannedTasks}` : '—', rateText(d.completedTasks, d.plannedTasks), i < r.comparisonDays ? minutesText(d.focusMinutes) : '—', d.habitTotal ? `${d.habitEffective}/${d.habitTotal}` : d.habitPending ? '待回顾' : '—']))
  section('任务分布')
  table(priorityGroups.map(g => g.title), [priorityGroups.map(g => { const tasks = r.tasks.filter(t => t.importance === g.importance && t.urgency === g.urgency); return `${tasks.filter(t => t.status === 'completed').length}/${tasks.length}` })])
  for (const completed of [true, false]) {
    const tasks = r.tasks.filter(t => (t.status === 'completed') === completed)
    y += 24; text(`${completed ? '已完成' : '未完成'}的事项 · ${tasks.length} 项`, 24, '#284735', 700)
    for (const task of tasks) text(`${task.date.slice(5)}  ${task.title}  · ${taskLabels[task.status] ?? task.status}`, 20)
  }
  section('近四周对比', r.comparisonDays < 7 ? `每周前 ${r.comparisonDays} 天` : '完整周')
  const weeks = [...r.history, { weekStart: r.weekStart, planned: r.plannedTasks, completed: r.completedTasks, focusMinutes: r.focusMinutes, habitCompleted: r.habitEffective, habitReviewed: r.habitTotal }]
  table(['周起始日', '完成 / 计划', '完成率', '专注', '打卡完成'], weeks.map(w => [w.weekStart.slice(5) + (w.weekStart === r.weekStart ? ' 本周' : ''), `${w.completed}/${w.planned}`, rateText(w.completed, w.planned), minutesText(w.focusMinutes), w.habitReviewed ? `${w.habitCompleted}/${w.habitReviewed}` : '—']))
  if (r.goals.length) { section('长期目标', '当前进度'); for (const g of r.goals) text(`${g.title} · ${g.progressPercent}% ${g.trophy ? trophies[g.trophy] : ''}`, 23) }
  section(''); text(`“${r.quote.text}”`, 28, '#354f3c'); text(`—— ${r.quote.author} · ${r.quote.source}`, 20)
  y += 24; text('连续天数统计至周末或昨天；未回顾不算中断。专注时长为已记录的计时。', 17, '#7b887f')
  return y + 70
}

export function wrapLines(context: Pick<CanvasRenderingContext2D, 'measureText'>, text: string, width: number): string[] {
  const lines: string[] = []; let line = ''
  for (const character of text) {
    if (character === '\n') { lines.push(line); line = ''; continue }
    if (line && context.measureText(line + character).width > width) { lines.push(line); line = character }
    else line += character
  }
  lines.push(line); return lines
}
