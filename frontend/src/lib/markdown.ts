/** Minimal Markdown → HTML for scenario rules content. */

import { withBase } from '@/lib/basePath'

export type MarkdownOptions = {
  /** Resolve display label for `[[slug]]` when no custom label is given. */
  ruleLabel?: (slug: string) => string
}

export function renderMarkdown(
  source: string,
  options: MarkdownOptions = {},
): string {
  const lines = source.replace(/\r\n/g, '\n').split('\n')
  const html: string[] = []
  /** Current open `<ul>` nesting; `-1` = not in a list. */
  let listDepth = -1

  const flushList = () => {
    if (listDepth < 0) return
    html.push('</li>')
    while (listDepth > 0) {
      html.push('</ul></li>')
      listDepth -= 1
    }
    html.push('</ul>')
    listDepth = -1
  }

  for (const raw of lines) {
    const line = raw.trimEnd()
    const trimmed = line.trim()

    if (!trimmed) {
      flushList()
      continue
    }

    const imageOnly = trimmed.match(/^\[img\]([^\[\]]+)\[img\]$/i)
    if (imageOnly) {
      flushList()
      html.push(renderScenarioImage(imageOnly[1]))
      continue
    }

    if (trimmed.startsWith('### ')) {
      flushList()
      html.push(`<h3>${inline(trimmed.slice(4), options)}</h3>`)
      continue
    }
    if (trimmed.startsWith('## ')) {
      flushList()
      html.push(`<h2>${inline(trimmed.slice(3), options)}</h2>`)
      continue
    }
    if (trimmed.startsWith('# ')) {
      flushList()
      html.push(`<h2>${inline(trimmed.slice(2), options)}</h2>`)
      continue
    }

    const listMatch = line.match(/^(\s*)[-*]\s+(.*)$/)
    if (listMatch) {
      const indent = listMatch[1].length
      // 2–4 spaces → 1 niveau, 5–8 → 2, etc.
      const depth =
        indent <= 0 ? 0 : Math.min(8, Math.ceil(indent / 4))
      const content = inline(listMatch[2], options)

      if (listDepth === -1) {
        html.push('<ul>')
        listDepth = 0
        while (listDepth < depth) {
          html.push('<li><ul>')
          listDepth += 1
        }
        html.push(`<li>${content}`)
      } else if (depth > listDepth) {
        while (listDepth < depth) {
          html.push('<ul>')
          listDepth += 1
          if (listDepth < depth) {
            html.push('<li>')
          }
        }
        html.push(`<li>${content}`)
      } else if (depth === listDepth) {
        html.push(`</li><li>${content}`)
      } else {
        html.push('</li>')
        while (listDepth > depth) {
          html.push('</ul>')
          listDepth -= 1
          if (listDepth > depth) {
            html.push('</li>')
          }
        }
        html.push(`</li><li>${content}`)
      }
      continue
    }

    flushList()
    if (trimmed.startsWith('*') && trimmed.endsWith('*') && trimmed.length > 2) {
      html.push(
        `<p class="md-note"><em>${inline(trimmed.slice(1, -1), options)}</em></p>`,
      )
    } else {
      html.push(`<p>${inline(trimmed, options)}</p>`)
    }
  }

  flushList()
  return html.join('\n')
}

function inline(text: string, options: MarkdownOptions = {}): string {
  return escapeHtml(text)
    .replace(/\[img\]([^\[\]]+)\[img\]/gi, (_, filename) =>
      renderScenarioImage(filename),
    )
    .replace(/\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g, (_, slugRaw, labelRaw) => {
      const slug = String(slugRaw).trim()
      const label = String(labelRaw ?? '').trim() || options.ruleLabel?.(slug) || slug
      return `<button type="button" class="md-rule-ref" data-rule-slug="${slug}">${label}</button>`
    })
    .replace(/###(.+?)###/g, '<span class="md-inline-title">$1</span>')
    .replace(/=b=(.+?)=b=/g, '<mark class="md-c-b">$1</mark>')
    .replace(/=v=(.+?)=v=/g, '<mark class="md-c-v">$1</mark>')
    .replace(/=r=(.+?)=r=/g, '<mark class="md-c-r">$1</mark>')
    .replace(/=o=(.+?)=o=/g, '<mark class="md-c-o">$1</mark>')
    .replace(/==(.+?)==/g, '<mark class="md-c-b">$1</mark>')
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/\*(.+?)\*/g, '<em>$1</em>')
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

function safeScenarioImageFilename(name: string): string | null {
  const trimmed = name.trim()
  if (!trimmed || trimmed.includes('/') || trimmed.includes('\\') || trimmed.includes('..')) {
    return null
  }
  if (!/\.(png|jpe?g|gif|webp|svg)$/i.test(trimmed)) {
    return null
  }
  return trimmed
}

function renderScenarioImage(rawFilename: string): string {
  const file = safeScenarioImageFilename(rawFilename)
  if (!file) {
    return escapeHtml(`[img]${rawFilename}[img]`)
  }
  const src = withBase(`/scenario/${encodeURIComponent(file).replace(/%2F/gi, '')}`)
  return `<img class="md-scenario-img" src="${src}" alt="${escapeHtml(file)}" loading="lazy" />`
}
