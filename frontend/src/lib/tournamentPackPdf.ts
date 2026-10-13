import { jsPDF } from 'jspdf'
import {
  downloadBlob,
  loadBlocksImages,
  scenarioFilename,
  type Block,
  type ImageAsset,
} from '@/lib/scenarioDocument'
import {
  BOTTOM,
  CONTENT_WIDTH,
  MARGIN,
  PAGE_HEIGHT,
  PAGE_WIDTH,
  PdfLayout,
  pdfSafeText,
} from '@/lib/scenarioPdf'
import {
  preparePackBlocks,
  type TocEntry,
  type TournamentPackDoc,
} from '@/lib/tournamentPackDocument'

const TOC_LINE = 5.4

type TocSlot = {
  entry: TocEntry
  page: number
  x: number
  y: number
  lines: string[]
}

export async function downloadTournamentPackPdf(
  doc: TournamentPackDoc,
): Promise<number> {
  const { images, missing } = await loadBlocksImages(doc.blocks)
  const pdf = renderPdf(doc, images)
  downloadBlob(
    pdf.output('blob'),
    `${scenarioFilename(doc.title)} - ${doc.subtitle}.pdf`,
  )
  return missing
}

function renderPdf(
  doc: TournamentPackDoc,
  images: Map<string, ImageAsset>,
): jsPDF {
  const pdf = new jsPDF({ unit: 'mm', format: 'a4' })
  pdf.setProperties({ title: doc.title, creator: 'Poissonnerie' })
  drawCover(pdf, doc.title, doc.subtitle)

  const tocStart = 2
  const slots = planToc(pdf, doc.toc, tocStart)
  const lastTocPage = slots.at(-1)?.page ?? tocStart
  const tocPages = lastTocPage - tocStart + 1
  for (let index = 0; index < tocPages; index += 1) pdf.addPage()
  pdf.addPage()

  const blocks = preparePackBlocks(doc.blocks, images)
  const layout = new PdfLayout(pdf, images)
  for (const block of blocks) appendBlock(layout, block, images)
  layout.finishLinks()
  drawToc(pdf, slots, layout)

  const pages = pdf.getNumberOfPages()
  for (let page = 2; page <= pages; page += 1) {
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

function drawCover(pdf: jsPDF, title: string, subtitle: string) {
  pdf.setFont('helvetica', 'bold')
  pdf.setFontSize(32)
  const lines = pdf.splitTextToSize(pdfSafeText(title), CONTENT_WIDTH - 16) as string[]
  const titleHeight = lines.length * 13
  let y = (PAGE_HEIGHT - titleHeight - 22) / 2
  pdf.setTextColor(18, 52, 58)
  pdf.text(lines, PAGE_WIDTH / 2, y, { align: 'center' })
  y += titleHeight + 2
  pdf.setDrawColor(10, 90, 102)
  pdf.setLineWidth(0.7)
  pdf.line(PAGE_WIDTH / 2 - 28, y, PAGE_WIDTH / 2 + 28, y)
  y += 10
  pdf.setFont('helvetica', 'normal')
  pdf.setFontSize(14)
  pdf.setTextColor(10, 90, 102)
  pdf.text(pdfSafeText(subtitle), PAGE_WIDTH / 2, y, { align: 'center' })
}

function planToc(pdf: jsPDF, entries: TocEntry[], startPage: number): TocSlot[] {
  let page = startPage
  let y = MARGIN + 18
  const slots: TocSlot[] = []
  for (const entry of entries) {
    const level1 = entry.level === 1
    const indent = level1 ? 0 : 8
    const lineH = level1 ? 6.2 : TOC_LINE
    if (level1 && y > MARGIN + 18) y += 2.4
    pdf.setFont('helvetica', level1 ? 'bold' : 'normal')
    pdf.setFontSize(level1 ? 12 : 11)
    const lines = pdf.splitTextToSize(
      pdfSafeText(entry.label),
      CONTENT_WIDTH - indent - 16,
    ) as string[]
    const height = Math.max(lineH, lines.length * lineH)
    if (y + height > PAGE_HEIGHT - BOTTOM) {
      page += 1
      y = MARGIN + 5
    }
    slots.push({
      entry,
      page,
      x: MARGIN + indent,
      y,
      lines,
    })
    y += height
  }
  return slots
}

function drawToc(pdf: jsPDF, slots: TocSlot[], layout: PdfLayout) {
  const firstPage = slots[0]?.page ?? 2
  pdf.setPage(firstPage)
  pdf.setFont('helvetica', 'bold')
  pdf.setFontSize(20)
  pdf.setTextColor(10, 90, 102)
  pdf.text('Table des matières', MARGIN, MARGIN + 8)
  pdf.setDrawColor(10, 90, 102)
  pdf.setLineWidth(0.6)
  pdf.line(MARGIN, MARGIN + 11, PAGE_WIDTH - MARGIN, MARGIN + 11)

  for (const slot of slots) {
    pdf.setPage(slot.page)
    const level1 = slot.entry.level === 1
    pdf.setFont('helvetica', level1 ? 'bold' : 'normal')
    pdf.setFontSize(level1 ? 12 : 11)
    if (level1) pdf.setTextColor(10, 90, 102)
    else pdf.setTextColor(26, 31, 36)
    const lineH = level1 ? 6.2 : TOC_LINE
    let y = slot.y
    for (const line of slot.lines) {
      pdf.text(line, slot.x, y)
      y += lineH
    }
    const target = layout.anchorTarget(slot.entry.anchor)
    if (!target) continue
    pdf.setFont('helvetica', 'normal')
    pdf.setFontSize(11)
    pdf.setTextColor(102, 112, 122)
    pdf.text(String(target.page), PAGE_WIDTH - MARGIN, slot.y, {
      align: 'right',
    })
    const linkH = slot.lines.length * (slot.entry.level === 1 ? 6.2 : TOC_LINE)
    pdf.link(MARGIN, slot.y - linkH + 1, CONTENT_WIDTH, linkH, {
      pageNumber: target.page,
      top: target.top,
      magFactor: 'XYZ',
      zoom: 0,
    })
  }
}

function appendBlock(
  layout: PdfLayout,
  block: Block,
  images: Map<string, ImageAsset>,
) {
  if (block.kind === 'spacer') {
    layout.gap(3)
    return
  }
  if (block.kind === 'image') {
    layout.drawImage(images.get(block.src), CONTENT_WIDTH, 110)
    return
  }
  if (block.kind === 'heading') {
    if (block.pageBreakBefore) layout.requestPageBreak()
    layout.drawBlock(block.inlines, {
      size: block.level === 1 ? 20 : block.level === 2 ? 14 : 12,
      bold: true,
      color:
        block.level === 1
          ? [10, 90, 102]
          : block.level === 2
            ? [18, 52, 58]
            : [26, 31, 36],
      gap: block.level === 1 ? 3 : 2,
      before: block.pageBreakBefore ? 1 : block.level === 1 ? 5 : block.level === 2 ? 4 : 3,
      anchor: block.anchor,
      rule: block.level === 1 ? 'strong' : block.level === 2 ? 'soft' : undefined,
      accent: block.level === 2,
    })
    return
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
    return
  }
  layout.drawBlock(block.inlines, {
    size: 11,
    note: block.note,
    color: block.note ? [92, 101, 112] : undefined,
    gap: 1.6,
  })
}
