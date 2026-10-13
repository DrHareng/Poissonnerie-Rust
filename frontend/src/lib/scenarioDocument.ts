import { withBase } from '@/lib/basePath'
import { resolveMarkdownImage } from '@/lib/markdown'
import type { CommonRule, ScenarioDetail } from '@/types/elo'

export type MarkColor = 'b' | 'v' | 'r' | 'o'

export type TextStyle = {
  bold?: boolean
  italic?: boolean
  color?: MarkColor
}

export type Inline =
  | { kind: 'text'; text: string; style: TextStyle }
  | { kind: 'rule'; slug: string; text: string; style: TextStyle }
  | { kind: 'break' }
  | { kind: 'image'; src: string; alt: string }

export type ListItem = {
  depth: number
  inlines: Inline[]
}

export type Block =
  | {
      kind: 'heading'
      level: 1 | 2 | 3
      inlines: Inline[]
      /** Cible des liens internes (table des matières, mots-clés de règle). */
      anchor?: string
      pageBreakBefore?: boolean
    }
  | { kind: 'paragraph'; inlines: Inline[]; note?: boolean }
  | { kind: 'list'; items: ListItem[] }
  | { kind: 'image'; src: string; alt: string }
  | { kind: 'spacer' }

export type DocSection = {
  title: string
  blocks: Block[]
}

export type ScenarioDoc = {
  title: string
  flavor: string | null
  sections: DocSection[]
}

export type ImageAsset = {
  png: Uint8Array
  dataUrl: string
  width: number
  height: number
}

const MARK_COLORS: Record<MarkColor, string> = {
  b: '007A8A',
  v: '0D7A3A',
  r: 'B4185A',
  o: 'B45400',
}

export function markColorHex(color: MarkColor | undefined): string | undefined {
  return color ? MARK_COLORS[color] : undefined
}

export function markColorRgb(color: MarkColor | undefined): [number, number, number] | null {
  const hex = markColorHex(color)
  if (!hex) return null
  return [
    Number.parseInt(hex.slice(0, 2), 16),
    Number.parseInt(hex.slice(2, 4), 16),
    Number.parseInt(hex.slice(4, 6), 16),
  ]
}

/** Nom d’ancre Word/PDF : lettre initiale, 40 caractères maximum. */
export function ruleAnchorId(slug: string): string {
  const safe = slug.replace(/[^A-Za-z0-9_]/g, '_').replace(/_+/g, '_')
  return `rule_${safe || 'x'}`.slice(0, 40)
}

export function scenarioFilename(title: string): string {
  const cleaned = title
    .replace(/[<>:"/\\|?*\u0000-\u001f]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
  return cleaned || 'scenario'
}

export function fittedSize(
  width: number,
  height: number,
  maxWidth: number,
  maxHeight: number,
): { width: number; height: number } {
  if (width <= 0 || height <= 0) {
    return { width: Math.round(maxWidth), height: Math.round(maxHeight) }
  }
  const scale = Math.min(maxWidth / width, maxHeight / height, 1)
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
  }
}

export function buildScenarioDocument(
  scenario: ScenarioDetail,
  rules: CommonRule[],
  options?: { includeCommonRules?: boolean },
): ScenarioDoc {
  const labels = new Map<string, string>()
  for (const rule of rules) labels.set(rule.slug, rule.name)
  if (scenario.exclusion_rule) {
    labels.set(scenario.exclusion_rule.slug, scenario.exclusion_rule.name)
  }
  for (const rule of scenario.common_rules) labels.set(rule.slug, rule.name)
  const ruleLabel = (slug: string) => labels.get(slug) ?? slug

  const sections: DocSection[] = []
  const add = (title: string, markdown: string | null | undefined) => {
    const blocks = parseScenarioMarkdown(markdown ?? '', ruleLabel)
    if (blocks.length > 0) sections.push({ title, blocks })
  }

  add('Fin de partie', scenario.end_condition_md)
  add('Objectifs', scenario.objectives_md)

  if (scenario.map_filename) {
    sections.push({
      title: 'Carte',
      blocks: [
        {
          kind: 'image',
          src: withBase(`/scenario-maps/${scenario.map_filename}`),
          alt: `Carte — ${scenario.name}`,
        },
      ],
    })
  }

  const deployment: Block[] = []
  if (scenario.deployment_notes_md?.trim()) {
    deployment.push(plainHeading('Éléments à déployer'))
    deployment.push(
      ...parseScenarioMarkdown(scenario.deployment_notes_md, ruleLabel),
    )
  }
  if (scenario.exclusion_zones_md?.trim() || scenario.exclusion_rule?.body_md) {
    deployment.push(
      plainHeading(
        scenario.exclusion_rule?.name ?? 'Zones d’exclusion',
        scenario.exclusion_rule
          ? ruleAnchorId(scenario.exclusion_rule.slug)
          : undefined,
      ),
    )
    if (scenario.exclusion_zones_md?.trim()) {
      deployment.push(
        ...parseScenarioMarkdown(scenario.exclusion_zones_md, ruleLabel),
      )
    }
    if (scenario.exclusion_rule?.body_md.trim()) {
      for (const block of parseScenarioMarkdown(
        scenario.exclusion_rule.body_md,
        ruleLabel,
      )) {
        deployment.push(
          block.kind === 'paragraph' ? { ...block, note: true } : block,
        )
      }
    }
  }
  if (deployment.length > 0) sections.push({ title: 'Déploiement', blocks: deployment })

  add('Éléments de scénario', scenario.elements_md)
  add('Règles spéciales', scenario.special_rules_md)

  if (options?.includeCommonRules !== false && scenario.common_rules.length > 0) {
    const blocks: Block[] = []
    const sorted = [...scenario.common_rules].sort((a, b) =>
      a.name.localeCompare(b.name, 'fr', { sensitivity: 'base' }),
    )
    for (const rule of sorted) {
      blocks.push(plainHeading(rule.name, ruleAnchorId(rule.slug)))
      blocks.push(...parseScenarioMarkdown(rule.body_md, ruleLabel))
    }
    if (blocks.length > 0) sections.push({ title: 'Règles communes', blocks })
  }

  return {
    title: scenario.name,
    flavor: scenario.flavor_text?.trim() || null,
    sections,
  }
}

export async function loadScenarioImages(doc: ScenarioDoc): Promise<{
  images: Map<string, ImageAsset>
  missing: number
}> {
  return loadBlocksImages(doc.sections.flatMap((section) => section.blocks))
}

export async function loadBlocksImages(blocks: Block[]): Promise<{
  images: Map<string, ImageAsset>
  missing: number
}> {
  const images = new Map<string, ImageAsset>()
  const srcs = collectBlocksImageSrcs(blocks)
  const loaded = await Promise.all(
    srcs.map(async (src) => ({ src, asset: await loadImageAsset(src) })),
  )
  let missing = 0
  for (const item of loaded) {
    if (item.asset) images.set(item.src, item.asset)
    else missing += 1
  }
  return { images, missing }
}

export function visibleBlocks(
  blocks: Block[],
  images: Map<string, ImageAsset>,
): Block[] {
  return blocks.filter(
    (block) => block.kind !== 'image' || images.has(block.src),
  )
}

export function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  link.click()
  URL.revokeObjectURL(url)
}

function plainHeading(text: string, anchor?: string): Block {
  return {
    kind: 'heading',
    level: 3,
    anchor,
    inlines: [{ kind: 'text', text, style: { bold: true } }],
  }
}

export function parseScenarioMarkdown(
  source: string,
  ruleLabel?: (slug: string) => string,
): Block[] {
  const blocks: Block[] = []
  let listItems: ListItem[] | null = null

  const flushList = () => {
    if (!listItems?.length) {
      listItems = null
      return
    }
    blocks.push({ kind: 'list', items: listItems })
    listItems = null
  }

  for (const raw of source.replace(/\r\n/g, '\n').split('\n')) {
    const line = raw.trimEnd()
    const trimmed = line.trim()
    if (!trimmed) {
      flushList()
      continue
    }
    if (/^#br#$/i.test(trimmed)) {
      flushList()
      blocks.push({ kind: 'spacer' })
      continue
    }

    const image = standaloneImage(trimmed)
    if (image) {
      flushList()
      blocks.push({ kind: 'image', src: image.src, alt: image.alt })
      continue
    }

    if (trimmed.startsWith('### ')) {
      flushList()
      pushHeading(blocks, trimmed.slice(4), ruleLabel)
      continue
    }
    if (trimmed.startsWith('## ')) {
      flushList()
      pushHeading(blocks, trimmed.slice(3), ruleLabel)
      continue
    }
    if (trimmed.startsWith('# ')) {
      flushList()
      pushHeading(blocks, trimmed.slice(2), ruleLabel)
      continue
    }

    const listMatch = line.match(/^(\s*)[-*]\s+(.*)$/)
    if (listMatch) {
      const indent = listMatch[1]?.length ?? 0
      const depth = indent <= 0 ? 0 : Math.min(8, Math.ceil(indent / 4))
      const inlines = parseInline(listMatch[2] ?? '', ruleLabel)
      if (!listItems) listItems = []
      listItems.push({ depth, inlines })
      continue
    }

    flushList()
    if (
      trimmed.startsWith('*') &&
      trimmed.endsWith('*') &&
      !trimmed.startsWith('**') &&
      trimmed.length > 2
    ) {
      const inlines = parseInline(trimmed.slice(1, -1), ruleLabel)
      if (inlines.length > 0) {
        blocks.push({ kind: 'paragraph', note: true, inlines })
      }
    } else {
      const inlines = parseInline(trimmed, ruleLabel)
      if (inlines.length > 0) blocks.push({ kind: 'paragraph', inlines })
    }
  }

  flushList()
  return blocks
}

function pushHeading(
  blocks: Block[],
  text: string,
  ruleLabel?: (slug: string) => string,
) {
  const inlines = parseInline(text, ruleLabel)
  if (inlines.length === 0) return
  blocks.push({ kind: 'heading', level: 3, inlines })
}

function standaloneImage(trimmed: string): { src: string; alt: string } | null {
  const imgTag = trimmed.match(/^\[img\]([^[\]]+)\[img\]$/i)
  if (imgTag?.[1]) return resolveMarkdownImage(imgTag[1])
  const bang = trimmed.match(/^!\[([^\]]*)\]\((https?:\/\/[^)\s]+)\)$/i)
  if (bang?.[2]) return resolveMarkdownImage(bang[2], bang[1])
  return null
}

function parseInline(
  text: string,
  ruleLabel?: (slug: string) => string,
): Inline[] {
  const slots: Inline[] = []
  const keep = (node: Inline) => {
    const index = slots.length
    slots.push(node)
    return `\u0001${index}\u0001`
  }

  let work = text.replace(/\[img\]([^[\]]+)\[img\]/gi, (full, raw: string) => {
    const resolved = resolveMarkdownImage(raw)
    return resolved ? keep({ kind: 'image', ...resolved }) : full
  })

  work = work.replace(/#br#/gi, () => keep({ kind: 'break' }))

  work = work.replace(
    /\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g,
    (_full, slugRaw: string, labelRaw?: string) => {
      const slug = slugRaw.trim()
      const label = labelRaw?.trim() || ruleLabel?.(slug) || slug
      return keep({ kind: 'rule', slug, text: label, style: { bold: true } })
    },
  )

  work = work.replace(
    /!\[([^\]]*)\]\((https?:\/\/[^)\s]+)\)/gi,
    (full, altRaw: string, urlRaw: string) => {
      const resolved = resolveMarkdownImage(urlRaw, altRaw)
      return resolved ? keep({ kind: 'image', ...resolved }) : full
    },
  )

  work = work.replace(
    /\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/g,
    (_full, labelRaw: string, urlRaw: string) => {
      const label = labelRaw.trim()
      const href = urlRaw.trim()
      const text = label && label !== href ? `${label} (${href})` : href
      return keep({ kind: 'text', text, style: {} })
    },
  )

  work = work.replace(/(?:https?:\/\/|www\.)[^\s<>[\]`'"]+/gi, (raw) => {
    const href = raw.replace(/[.,;:!?]+$/, '')
    const trailing = raw.slice(href.length)
    const url = /^www\./i.test(href) ? `https://${href}` : href
    return keep({ kind: 'text', text: url, style: {} }) + trailing
  })

  work = applyMarkupMarkers(work)
  return tokenizeMarked(work, slots)
}

function applyMarkupMarkers(input: string): string {
  return input
    .replace(/###(.+?)###/g, '\u0002T\u0002$1\u0003')
    .replace(/=b=(.+?)=b=/g, '\u0002b\u0002$1\u0003')
    .replace(/=v=(.+?)=v=/g, '\u0002v\u0002$1\u0003')
    .replace(/=r=(.+?)=r=/g, '\u0002r\u0002$1\u0003')
    .replace(/=o=(.+?)=o=/g, '\u0002o\u0002$1\u0003')
    .replace(/==(.+?)==/g, '\u0002b\u0002$1\u0003')
    .replace(/\*\*(.+?)\*\*/g, '\u0002B\u0002$1\u0003')
    .replace(/\*(.+?)\*/g, '\u0002I\u0002$1\u0003')
}

function styleFromCode(code: string): TextStyle {
  if (code === 'T' || code === 'B') return { bold: true }
  if (code === 'I') return { italic: true }
  if (code === 'b' || code === 'v' || code === 'r' || code === 'o') {
    return { bold: true, color: code }
  }
  return {}
}

function mergeStyles(stack: TextStyle[]): TextStyle {
  const style: TextStyle = {}
  for (const item of stack) {
    if (item.bold) style.bold = true
    if (item.italic) style.italic = true
    if (item.color) style.color = item.color
  }
  return style
}

function tokenizeMarked(work: string, slots: Inline[]): Inline[] {
  const out: Inline[] = []
  const stack: TextStyle[] = []
  let buffer = ''

  const flush = () => {
    if (!buffer) return
    out.push({ kind: 'text', text: buffer, style: mergeStyles(stack) })
    buffer = ''
  }

  for (let index = 0; index < work.length; index += 1) {
    const placeholder = work.slice(index).match(/^\u0001(\d+)\u0001/)
    if (placeholder) {
      flush()
      const node = slots[Number(placeholder[1])]
      if (node) out.push(node)
      index += placeholder[0].length - 1
      continue
    }
    const opener = work.slice(index).match(/^\u0002([BITvbro])\u0002/)
    if (opener?.[1]) {
      flush()
      stack.push(styleFromCode(opener[1]))
      index += opener[0].length - 1
      continue
    }
    if (work[index] === '\u0003') {
      flush()
      stack.pop()
      continue
    }
    buffer += work[index]
  }
  flush()
  return out
}

function collectBlocksImageSrcs(blocks: Block[]): string[] {
  const srcs = new Set<string>()
  const visit = (inlines: Inline[]) => {
    for (const inline of inlines) {
      if (inline.kind === 'image') srcs.add(inline.src)
    }
  }
  for (const block of blocks) {
    if (block.kind === 'image') srcs.add(block.src)
    if (block.kind === 'paragraph' || block.kind === 'heading') {
      visit(block.inlines)
    }
    if (block.kind === 'list') {
      for (const item of block.items) visit(item.inlines)
    }
  }
  return [...srcs]
}

async function loadImageAsset(src: string): Promise<ImageAsset | null> {
  try {
    const response = await fetch(src)
    if (!response.ok) return null
    const blob = await response.blob()
    const bitmap = await createImageBitmap(blob)
    const canvas = document.createElement('canvas')
    canvas.width = bitmap.width
    canvas.height = bitmap.height
    const context = canvas.getContext('2d')
    if (!context) {
      bitmap.close()
      return null
    }
    context.drawImage(bitmap, 0, 0)
    bitmap.close()
    const dataUrl = canvas.toDataURL('image/png')
    return {
      png: dataUrlToBytes(dataUrl),
      dataUrl,
      width: canvas.width,
      height: canvas.height,
    }
  } catch {
    return null
  }
}

function dataUrlToBytes(dataUrl: string): Uint8Array {
  const base64 = dataUrl.slice(dataUrl.indexOf(',') + 1)
  const binary = atob(base64)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index)
  }
  return bytes
}
