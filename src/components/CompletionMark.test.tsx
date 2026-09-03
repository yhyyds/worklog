import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { CompletionMark } from '../App'

describe('CompletionMark', () => {
  it('uses a centered SVG path instead of a font glyph', () => {
    const markup = renderToStaticMarkup(<CompletionMark />)

    expect(markup).toContain('viewBox="0 0 12 12"')
    expect(markup).toContain('d="M2.1 6.2 4.8 8.7 9.9 3"')
    expect(markup).not.toContain('✓')
  })
})
