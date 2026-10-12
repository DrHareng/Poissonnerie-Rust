import { jsPDF } from 'jspdf'
import {
  downloadBlob,
  loadScenarioImages,
  markColorRgb,
  scenarioFilename,
  visibleBlocks,
  type ImageAsset,
  type Inline,
  type ScenarioDoc,
  type TextStyle,
} from '@/lib/scenarioDocument'

const PAGE_WIDTH = 210
const PAGE_HEIGHT = 297
const MARGIN = 14
const BOTTOM = 18
const CONTENT_WIDTH = PAGE_WIDTH - MARGIN * 2

export async function downloadScenarioPdf(doc: ScenarioDoc): Promise<number> {
  const { images, missing } = await loadScenarioImages(doc)
  const pdf = renderPdf(doc, images)
  downloadBlob(pdf.output('blob'), `${scenarioFilename(doc.title)}.pdf`)
  return missing
}

function renderPdf(doc: ScenarioDoc, images: Map<string, ImageAsset>): jsPDF {
  const pdf = new jsPDF({ unit: 'mm', format: 'a4' })
  pdf.setProperties({ title: doc.title, creator: 'Poissonnerie' })
  const layout = new Layout(pdf, images)

  layout.drawBlock(textInline(doc.title), {
    size: 18,
    bold: true,
    gap: 2,
  })
  if (doc.flavor) {
    const quoted = doc.flavor.startsWith('«') ? doc.flavor : `« ${doc.flavor} »`
    layout.drawBlock(textInline(quoted), {
      size: 11,
      note: true,
      color: [92, 101, 112],
      gap: 4,
    })
  }

  for (const section of doc.sections) {
    const blocks = visibleBlocks(section.blocks, images)
    if (blocks.length === 0) continue
    layout.drawBlock(textInline(section.title), {
      size: 14,
      bold: true,
      color: [10, 90, 102],
      gap: 2,
      before: 4,
    })
    for (const block of blocks) {
      if (block.kind === 'spacer') {
        layout.gap(3)
        continue
      }
      if (block.kind === 'image') {
        layout.drawImage(images.get(block.src), CONTENT_WIDTH, 110)
        continue
      }
      if (block.kind === 'heading') {
        layout.drawBlock(block.inlines, {
          size: 12,
          bold: true,
          gap: 1.5,
          before: 2,
        })
        continue
      }
      if (block.kind === 'list') {
        for (const item of block.items) {
          layout.drawBlock(item.inlines, {
            size: 11,
            gap: 1,
            indent: 4 + item.depth * 5,
            bullet: true,
          })
        }
        layout.gap(1)
        continue
      }
      layout.drawBlock(block.inlines, {
        size: 11,
        note: block.note,
        gap: 1.6,
      })
    }
  }

  const pages = pdf.getNumberOfPages()
  for (let page = 1; page <= pages; page += 1) {
    pdf.setPage(page)
    pdf.setFont('helvetica', 'normal')
    pdf.setFontSize(9)
    pdf.setTextColor(102, 112, 122)
    pdf.text(`${page} / ${pages}`, PAGE_WIDTH - MARGIN, PAGE_HEIGHT - 8, {
      align: 'right',
    })
  }
  return pdf
}

function textInline(text: string): Inline[] {
  return [{ kind: 'text', text, style: {} }]
}

type DrawOptions = {
  size: number
  bold?: boolean
  note?: boolean
  color?: [number, number, number]
  gap?: number
  before?: number
  indent?: number
  bullet?: boolean
}

class Layout {
  private y = MARGIN + 5
  private readonly pdf: jsPDF
  private readonly images: Map<string, ImageAsset>

  constructor(pdf: jsPDF, images: Map<string, ImageAsset>) {
    this.pdf = pdf
    this.images = images
  }

  gap(amount: number) {
    this.y += amount
  }

  drawBlock(inlines: Inline[], options: DrawOptions) {
    if (options.before) this.y += options.before
    const lineHeight = options.size * 0.45
    const indent = options.indent ?? 0
    const bulletWidth = options.bullet ? 4 : 0
    const x0 = MARGIN + indent
    const wrapX = x0 + bulletWidth
    const maxWidth = CONTENT_WIDTH - indent - bulletWidth
    let x = wrapX
    let lineOpen = false

    const nextLine = () => {
      this.y += lineHeight
      this.ensure(lineHeight)
      x = wrapX
      lineOpen = false
    }

    this.ensure(lineHeight)
    if (options.bullet) {
      this.pdf.setFont('helvetica', 'normal')
      this.pdf.setFontSize(options.size)
      this.pdf.setTextColor(26, 31, 36)
      this.pdf.text('-', x0, this.y)
    }

    for (const inline of inlines) {
      if (inline.kind === 'break') {
        nextLine()
        continue
      }
      if (inline.kind === 'image') {
        if (lineOpen) this.y += lineHeight
        this.drawImage(this.images.get(inline.src), CONTENT_WIDTH, 70)
        x = wrapX
        lineOpen = false
        this.ensure(lineHeight)
        continue
      }
      const style: TextStyle = {
        ...inline.style,
        bold: inline.style.bold || options.bold,
        italic: inline.style.italic || options.note,
      }
      const color = markColorRgb(style.color) ?? options.color ?? [26, 31, 36]
      this.pdf.setFont('helvetica', fontStyle(style))
      this.pdf.setFontSize(options.size)
      this.pdf.setTextColor(color[0], color[1], color[2])
      for (const part of pdfParts(inline.text)) {
        if (/^\s+$/.test(part)) {
          if (!lineOpen) continue
          const space = this.pdf.getTextWidth(' ')
          if (x + space > wrapX + maxWidth) nextLine()
          else x += space
          continue
        }
        const width = this.pdf.getTextWidth(part)
        if (lineOpen && x + width > wrapX + maxWidth) nextLine()
        this.pdf.text(part, x, this.y)
        x += width
        lineOpen = true
      }
    }
    this.y += lineHeight + (options.gap ?? 1)
  }

  drawImage(
    asset: ImageAsset | undefined,
    maxWidth: number,
    maxHeight: number,
  ) {
    if (!asset) return
    const aspect = asset.height / asset.width
    let width = maxWidth
    let height = width * aspect
    if (height > maxHeight) {
      height = maxHeight
      width = height / aspect
    }
    const available = PAGE_HEIGHT - BOTTOM - this.y
    if (height > available) {
      this.pdf.addPage()
      this.y = MARGIN + 5
    }
    const pageRoom = PAGE_HEIGHT - BOTTOM - this.y
    if (height > pageRoom) {
      height = pageRoom
      width = height / aspect
    }
    this.pdf.addImage(asset.dataUrl, 'PNG', MARGIN, this.y, width, height)
    this.y += height + 4
  }

  private ensure(height: number) {
    if (this.y + height <= PAGE_HEIGHT - BOTTOM) return
    this.pdf.addPage()
    this.y = MARGIN + 5
  }
}

function fontStyle(style: TextStyle): 'normal' | 'bold' | 'italic' | 'bolditalic' {
  if (style.bold && style.italic) return 'bolditalic'
  if (style.bold) return 'bold'
  if (style.italic) return 'italic'
  return 'normal'
}

function pdfParts(text: string): string[] {
  const normalized = text
    .replace(/\u00a0/g, ' ')
    .replace(/[\u2018\u2019\u201b]/g, "'")
    .replace(/[\u201c\u201d]/g, '"')
    .replace(/[\u2013\u2014]/g, '-')
    .replace(/\u2026/g, '...')
    .replace(/\u202f/g, ' ')
  return normalized.split(/(\s+)/).filter(Boolean)
}
