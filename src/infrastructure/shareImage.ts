import { minutesText, rateText } from '../domain/report'
import type { Overview } from '../domain/sharing'
import { wrapLines } from './reportImage'

// This renderer receives only the sanitized overview, never the private report.
export async function renderOverviewPng(r: Overview): Promise<Blob> {
  await document.fonts.ready
  const canvas = document.createElement('canvas'); canvas.width = 1200; canvas.height = 1
  const ctx = canvas.getContext('2d'); if (!ctx) throw new Error('无法创建图片')
  function draw() {
    const c = ctx!; let y = 70
    c.fillStyle = '#f4f7f1'; c.fillRect(0, 0, canvas.width, canvas.height); c.textBaseline = 'top'
    function text(value: string, size = 23, color = '#3b5948') { c.font = `${size}px "Microsoft YaHei", sans-serif`; c.fillStyle = color; for (const line of wrapLines(c, value, 1040)) { c.fillText(line, 80, y); y += size * 1.65 } }
    function section(title: string, color = '#5b9b7d') { y += 30; c.fillStyle = color; c.fillRect(80, y, 1040, 3); y += 22; text(title, 30); y += 8 }
    function row(values: string[]) { c.font = '20px "Microsoft YaHei"'; const w = 1040 / values.length; const lines = values.map(s => wrapLines(c, s, w - 20)); const h = Math.max(...lines.map(s => s.length)) * 32 + 20; c.fillStyle = '#e6eee5'; c.fillRect(80, y, 1040, h); c.fillStyle = '#3b5948'; lines.forEach((ls, i) => ls.forEach((s, j) => c.fillText(s, 90 + i * w, y + 10 + j * 32))); y += h + 2 }
    text('WORKLOG / 一周小笺', 38); text(`${r.weekStart} — ${r.weekEnd}`, 23); text(`截至 ${r.through}`, 19); y += 22
    row(['完成事项', '专注时间', '完成率']); row([`${r.counts.completed}/${r.counts.planned}`, minutesText(r.counts.focusMinutes), rateText(r.counts.completed, r.counts.planned)])
    for (const category of r.categories.filter(({ counts }) => counts.planned > 0 || counts.focusMinutes > 0)) {
      const counts = category.counts
      section(category.name, category.color)
      row(['完成事项', '完成率', '专注'])
      row([`${counts.completed}/${counts.planned}`, rateText(counts.completed, counts.planned), minutesText(counts.focusMinutes)])
    }
    section('每天的完成情况'); row(['日期', '完成事项', '专注'])
    for (const d of r.daily) row([d.date.slice(5), d.date <= r.through ? `${d.counts.completed}/${d.counts.planned}` : '—', d.date <= r.through ? minutesText(d.counts.focusMinutes) : '—'])
    section('近四周对比'); text('与本周相同天数', 19); row(['周起始', '完成率', '专注'])
    for (const d of [...r.history, { date: r.weekStart, counts: r.counts }]) row([`${d.date.slice(5)}${d.date === r.weekStart ? ' 本周' : ''}`, rateText(d.counts.completed, d.counts.planned), minutesText(d.counts.focusMinutes)])
    section(''); text(`“${r.quote.text}”`, 28); text(`—— ${r.quote.author} · ${r.quote.source}`, 20)
    return Math.ceil(y + 70)
  }
  const height = draw(); if (height > 30000) throw new Error('记录过多，无法导出为单张图片')
  canvas.height = height; draw()
  return new Promise((resolve, reject) => canvas.toBlob(blob => blob ? resolve(blob) : reject(new Error('图片生成失败')), 'image/png'))
}
