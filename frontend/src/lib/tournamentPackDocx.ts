import {
  AlignmentType,
  BorderStyle,
  Document,
  Footer,
  HeadingLevel,
  Packer,
  PageNumber,
  Paragraph,
  SectionType,
  ShadingType,
  TableOfContents,
  TextRun,
  VerticalAlignSection,
  type FileChild,
} from 'docx'
import { estimatePageNumbers } from 'docx/layout'
import {
  downloadBlob,
  loadBlocksImages,
  scenarioFilename,
  type Block,
  type ImageAsset,
} from '@/lib/scenarioDocument'
import {
  bookmark,
  docxImageParagraph,
  docxRuns,
} from '@/lib/scenarioDocx'
import {
  preparePackBlocks,
  type TournamentPackDoc,
} from '@/lib/tournamentPackDocument'

const FONT = 'Calibri'
const TEAL = '0A5A66'
const INK = '12343A'
const BODY = '1A1F24'

export async function downloadTournamentPackDocx(
  doc: TournamentPackDoc,
): Promise<number> {
  const { images, missing } = await loadBlocksImages(doc.blocks)
  const blob = await Packer.toBlob(renderDocx(doc, images)).catch(() =>
    Packer.toBlob(renderDocx(doc, images, false)),
  )
  downloadBlob(blob, `${scenarioFilename(doc.title)} - ${doc.subtitle}.docx`)
  return missing
}

function renderDocx(
  doc: TournamentPackDoc,
  images: Map<string, ImageAsset>,
  withPageNumbers = true,
): Document {
  const blocks = preparePackBlocks(doc.blocks, images)
  const anchors = new Set<string>()
  for (const block of blocks) {
    if (block.kind === 'heading' && block.anchor) anchors.add(block.anchor)
  }

  const body: FileChild[] = [
    new Paragraph({
      spacing: { after: 240 },
        border: {
          bottom: {
            color: TEAL,
            space: 1,
            style: BorderStyle.SINGLE,
            size: 12,
          },
        },
        children: [
        new TextRun({
          text: 'Table des matières',
          bold: true,
          font: FONT,
          size: 44,
          color: TEAL,
        }),
      ],
    }),
    new TableOfContents('Table des matières', {
      hyperlink: true,
      headingStyleRange: '1-2',
    }),
  ]

  for (const block of blocks) {
    appendBlock(body, block, images, anchors)
  }

  return new Document({
    title: doc.title,
    creator: 'Poissonnerie',
    ...(withPageNumbers ? { pageNumbers: estimatePageNumbers } : {}),
    styles: {
      default: {
        heading1: {
          run: { font: FONT, size: 44, bold: true, color: TEAL },
          paragraph: {
            spacing: { before: 120, after: 200 },
          },
        },
        heading2: {
          run: { font: FONT, size: 32, bold: true, color: INK },
          paragraph: {
            spacing: { before: 280, after: 80 },
          },
        },
        heading3: {
          run: { font: FONT, size: 26, bold: true, color: BODY },
          paragraph: {
            spacing: { before: 240, after: 60 },
          },
        },
      },
    },
    sections: [
      {
        properties: {
          verticalAlign: VerticalAlignSection.CENTER,
          page: {
            margin: { top: 1000, bottom: 1000, left: 1000, right: 1000 },
          },
        },
        children: [
          new Paragraph({
            alignment: AlignmentType.CENTER,
            spacing: { after: 200 },
            border: {
              bottom: {
                color: TEAL,
                space: 10,
                style: BorderStyle.SINGLE,
                size: 12,
              },
            },
            children: [
              new TextRun({
                text: doc.title,
                bold: true,
                font: FONT,
                size: 72,
                color: INK,
              }),
            ],
          }),
          new Paragraph({
            alignment: AlignmentType.CENTER,
            spacing: { before: 200 },
            children: [
              new TextRun({
                text: doc.subtitle,
                font: FONT,
                size: 32,
                color: TEAL,
                characterSpacing: 80,
              }),
            ],
          }),
        ],
      },
      {
        properties: {
          type: SectionType.NEXT_PAGE,
          page: {
            margin: { top: 900, bottom: 900, left: 900, right: 900 },
          },
        },
        footers: {
          default: new Footer({
            children: [
              new Paragraph({
                alignment: AlignmentType.RIGHT,
                children: [
                  new TextRun({
                    children: [PageNumber.CURRENT],
                    font: FONT,
                    size: 18,
                    color: '66707A',
                  }),
                ],
              }),
            ],
          }),
        },
        children: body,
      },
    ],
  })
}

function appendBlock(
  body: FileChild[],
  block: Block,
  images: Map<string, ImageAsset>,
  anchors: Set<string>,
) {
  if (block.kind === 'spacer') {
    body.push(new Paragraph({ spacing: { after: 120 }, children: [] }))
    return
  }
  if (block.kind === 'image') {
    body.push(docxImageParagraph(block.src, block.alt, images, false))
    return
  }
  if (block.kind === 'heading') {
    const size = block.level === 1 ? 44 : block.level === 2 ? 32 : 26
    const color = block.level === 1 ? TEAL : block.level === 2 ? INK : BODY
    body.push(
      new Paragraph({
        heading:
          block.level === 1
            ? HeadingLevel.HEADING_1
            : block.level === 2
              ? HeadingLevel.HEADING_2
              : HeadingLevel.HEADING_3,
        pageBreakBefore: block.pageBreakBefore,
        spacing: {
          before: block.pageBreakBefore ? 80 : block.level === 1 ? 320 : block.level === 2 ? 300 : 240,
          after:
            (block.level === 1 ? 160 : block.level === 2 ? 100 : 60) +
            (block.blankAfter ? 240 : 0),
        },
        shading:
          block.level === 1
            ? { type: ShadingType.CLEAR, fill: 'E5F3F5' }
            : undefined,
        border:
          block.level === 1
            ? {
                bottom: {
                  color: TEAL,
                  space: 1,
                  style: BorderStyle.SINGLE,
                  size: 16,
                },
              }
            : block.level === 2
              ? {
                  left: {
                    color: TEAL,
                    space: 8,
                    style: BorderStyle.SINGLE,
                    size: 24,
                  },
                }
              : undefined,
        indent: block.level === 2 ? { left: 120 } : undefined,
        children: bookmark(
          block.anchor,
          docxRuns(block.inlines, images, anchors, {
            size,
            bold: true,
            color,
          }),
        ),
      }),
    )
    return
  }
  if (block.kind === 'list') {
    for (const item of block.items) {
      body.push(
        new Paragraph({
          spacing: { after: 60 },
          indent: { left: 420 * (item.depth + 1) },
          children: [
            new TextRun({ text: '• ', font: FONT, size: 22 }),
            ...docxRuns(item.inlines, images, anchors, { size: 22 }),
          ],
        }),
      )
    }
    return
  }
  body.push(
    new Paragraph({
      spacing: { after: 120 },
      children: docxRuns(block.inlines, images, anchors, {
        size: 22,
        note: block.note,
        color: block.note ? '5C6570' : undefined,
      }),
    }),
  )
}
