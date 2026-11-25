import { minutesText, rateText, trend, weekdays } from '../domain/report'
import type { CategorySummary, Overview } from '../domain/sharing'

export function CategoryCards({ categories }: { categories: CategorySummary[] }) {
  const visible = categories.filter(({ counts }) => counts.planned > 0 || counts.focusMinutes > 0)
  if (!visible.length) return null
  return <section className="weekly-section"><header><h3>分类完成情况</h3><span>本周安排</span></header><div className="report-categories">{visible.map((category) => <article key={category.name} style={{ borderTopColor: category.color }}>
    <h4><i className="category-dot" style={{ background: category.color }}/>{category.name}</h4>
    <dl><div><dt>完成事项</dt><dd>{category.counts.completed}/{category.counts.planned}</dd></div>
      <div><dt>完成率</dt><dd>{rateText(category.counts.completed, category.counts.planned)}</dd></div>
      <div><dt>专注时间</dt><dd>{minutesText(category.counts.focusMinutes)}</dd></div></dl>
  </article>)}</div></section>
}
export default function ShareOverview({ report: r }: { report: Overview }) {
  const c = r.counts, previous = r.history.at(-1)?.counts
  const change = (current: number, prior?: number) => prior ? Math.round((current - prior) * 100 / prior) : null
  return <div className="weekly-page"><section className="weekly-hero"><small>{r.weekStart} — {r.weekEnd}</small><h3>一周小笺</h3><p>截至 {r.through}</p></section>
    <section className="weekly-numbers"><div><small>完成事项</small><strong>{c.completed}<i>/{c.planned}</i></strong><span>{trend(change(c.completed, previous?.completed))}</span></div><div><small>专注时间</small><strong>{minutesText(c.focusMinutes)}</strong><span>{trend(change(c.focusMinutes, previous?.focusMinutes))}</span></div><div><small>计划完成率</small><strong>{rateText(c.completed, c.planned)}</strong></div></section>
    <CategoryCards categories={r.categories}/>
    <section className="weekly-section"><header><h3>每天的完成情况</h3></header><div className="weekly-table-scroll"><table className="weekly-table"><thead><tr><th>日期</th><th>事项完成</th><th>专注</th></tr></thead><tbody>{r.daily.map((d, i) => <tr key={d.date}><th>{d.date.slice(5)} 周{weekdays[i]}</th><td>{d.date <= r.through ? `${d.counts.completed}/${d.counts.planned}` : '—'}</td><td>{d.date <= r.through ? minutesText(d.counts.focusMinutes) : '—'}</td></tr>)}</tbody></table></div></section>
    <section className="weekly-section"><header><h3>近四周对比</h3><span>与本周相同天数</span></header><table className="weekly-table"><thead><tr><th>周起始</th><th>完成率</th><th>专注</th></tr></thead><tbody>{[...r.history, { date: r.weekStart, counts: c }].map(w => <tr key={w.date}><th>{w.date.slice(5)}{w.date === r.weekStart ? ' 本周' : ''}</th><td>{rateText(w.counts.completed, w.counts.planned)}</td><td>{minutesText(w.counts.focusMinutes)}</td></tr>)}</tbody></table></section>
    <blockquote><p>“{r.quote.text}”</p><footer>—— {r.quote.author} · {r.quote.source}</footer></blockquote>
  </div>
}
