const FONT_SCALE_KEY = 'worklog.appearance.fontScale'

export const MIN_FONT_SCALE = 85
export const MAX_FONT_SCALE = 130

export function normalizeFontScale(value: number): number {
  if (!Number.isFinite(value)) return 100
  return Math.min(MAX_FONT_SCALE, Math.max(MIN_FONT_SCALE, Math.round(value / 5) * 5))
}

export function loadFontScale(): number {
  try {
    return normalizeFontScale(Number(localStorage.getItem(FONT_SCALE_KEY) ?? 100))
  } catch {
    return 100
  }
}

export function applyFontScale(value: number): number {
  const normalized = normalizeFontScale(value)
  document.documentElement.style.setProperty('--worklog-ui-scale', String(normalized / 100))
  return normalized
}

export function saveFontScale(value: number): number {
  const normalized = applyFontScale(value)
  localStorage.setItem(FONT_SCALE_KEY, String(normalized))
  return normalized
}
