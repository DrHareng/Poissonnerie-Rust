/** Markdown maison → HTML (scénarios, rapports, descriptions, etc.). */

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

    const imageOnly = standaloneImageHtml(trimmed)
    if (imageOnly) {
      flushList()
      html.push(imageOnly)
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
  const slots: string[] = []
  const keep = (html: string) => {
    const i = slots.length
    slots.push(html)
    return `\u0001${i}\u0001`
  }

  let work = text.replace(/\[img\]([^\[\]]+)\[img\]/gi, (_, raw) =>
    keep(renderMarkdownImage(String(raw))),
  )

  work = work.replace(
    /\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g,
    (_, slugRaw, labelRaw) => {
      const slug = String(slugRaw).trim()
      const label =
        String(labelRaw ?? '').trim() || options.ruleLabel?.(slug) || slug
      return keep(
        `<button type="button" class="md-rule-ref" data-rule-slug="${escapeHtml(slug)}">${escapeHtml(label)}</button>`,
      )
    },
  )

  work = work.replace(
    /!\[([^\]]*)\]\((https?:\/\/[^)\s]+)\)/gi,
    (full, altRaw, urlRaw) => {
      const url = safeHttpUrl(String(urlRaw))
      if (!url) return full
      return keep(renderRemoteImage(url, String(altRaw)))
    },
  )

  work = work.replace(
    /\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/g,
    (full, labelRaw, urlRaw) => {
      const url = safeHttpUrl(String(urlRaw))
      if (!url) return full
      const label = applyInlineMarkup(escapeHtml(String(labelRaw)))
      return keep(renderLink(url, label))
    },
  )

  work = work.replace(
    /(?:https?:\/\/|www\.)[^\s<>\[\]`'"]+/gi,
    (raw) => {
      const { href, trailing } = splitAutolink(raw)
      const url = safeHttpUrl(normalizeHrefCandidate(href))
      if (!url) return raw
      return keep(renderLink(url, escapeHtml(href))) + trailing
    },
  )

  return unstash(applyInlineMarkup(escapeHtml(work)), slots)
}

function unstash(text: string, slots: string[]): string {
  return text.replace(/\u0001(\d+)\u0001/g, (_, i) => slots[Number(i)] ?? '')
}

function applyInlineMarkup(escaped: string): string {
  return escaped
    .replace(/###(.+?)###/g, '<span class="md-inline-title">$1</span>')
    .replace(/=b=(.+?)=b=/g, '<mark class="md-c-b">$1</mark>')
    .replace(/=v=(.+?)=v=/g, '<mark class="md-c-v">$1</mark>')
    .replace(/=r=(.+?)=r=/g, '<mark class="md-c-r">$1</mark>')
    .replace(/=o=(.+?)=o=/g, '<mark class="md-c-o">$1</mark>')
    .replace(/==(.+?)==/g, '<mark class="md-c-b">$1</mark>')
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/\*(.+?)\*/g, '<em>$1</em>')
}

function standaloneImageHtml(trimmed: string): string | null {
  const imgTag = trimmed.match(/^\[img\]([^\[\]]+)\[img\]$/i)
  if (imgTag) return renderMarkdownImage(imgTag[1])

  const bang = trimmed.match(/^!\[([^\]]*)\]\((https?:\/\/[^)\s]+)\)$/i)
  if (bang) {
    const url = safeHttpUrl(bang[2])
    if (!url) return null
    return renderRemoteImage(url, bang[1])
  }
  return null
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

function splitAutolink(raw: string): { href: string; trailing: string } {
  const match = raw.match(/^(.*?)([.,;:!?]+)$/)
  if (!match || !match[1]) return { href: raw, trailing: '' }
  return { href: match[1], trailing: match[2] }
}

function normalizeHrefCandidate(raw: string): string {
  const trimmed = raw.trim()
  if (/^www\./i.test(trimmed)) return `https://${trimmed}`
  return trimmed
}

/** http(s) only — rejects javascript:, data:, credentials, etc. */
function safeHttpUrl(raw: string): string | null {
  const trimmed = normalizeHrefCandidate(raw)
  if (!trimmed || trimmed.length > 2000 || /\s/.test(trimmed)) return null
  let parsed: URL
  try {
    parsed = new URL(trimmed)
  } catch {
    return null
  }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return null
  if (parsed.username || parsed.password) return null
  return parsed.href
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

function altFromUrl(url: string): string {
  try {
    const name = new URL(url).pathname.split('/').filter(Boolean).pop()
    if (name) return decodeURIComponent(name)
  } catch {
    /* ignore */
  }
  return ''
}

function renderImgTag(src: string, alt: string): string {
  return `<img class="md-img" src="${escapeHtml(src)}" alt="${escapeHtml(alt)}" loading="lazy" referrerpolicy="no-referrer" />`
}

function renderRemoteImage(url: string, altRaw?: string): string {
  const alt = altRaw?.trim() || altFromUrl(url)
  return renderImgTag(url, alt)
}

function renderLink(href: string, innerHtml: string): string {
  return `<a class="md-link" href="${escapeHtml(href)}" target="_blank" rel="noopener noreferrer">${innerHtml}</a>`
}

function renderMarkdownImage(rawFilename: string): string {
  const trimmed = rawFilename.trim()
  const remote = safeHttpUrl(trimmed)
  if (remote) return renderRemoteImage(remote)

  const file = safeScenarioImageFilename(trimmed)
  if (!file) {
    return escapeHtml(`[img]${rawFilename}[img]`)
  }
  const src = withBase(`/scenario/${encodeURIComponent(file).replace(/%2F/gi, '')}`)
  return renderImgTag(src, file)
}
