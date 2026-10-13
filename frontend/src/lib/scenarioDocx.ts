import {
  AlignmentType,
  Bookmark,
  Document,
  Footer,
  HeadingLevel,
  ImageRun,
  InternalHyperlink,
  Packer,
  PageNumber,
  Paragraph,
  TextRun,
  UnderlineType,
  type ParagraphChild,
} from 'docx'
import {
  downloadBlob,
  fittedSize,
  loadScenarioImages,
  markColorHex,
  ruleAnchorId,
  scenarioFilename,
  visibleBlocks,
  type ImageAsset,
  type Inline,
  type ScenarioDoc,
  type TextStyle,
} from '@/lib/scenarioDocument'

const FONT = 'Calibri'

export async function downloadScenarioDocx(doc: ScenarioDoc): Promise<number> {
  const { images, missing } = await loadScenarioImages(doc)
  const blob = await Packer.toBlob(renderDocx(doc, images))
  downloadBlob(blob, `${scenarioFilename(doc.title)}.docx`)
  return missing
}

function renderDocx(doc: ScenarioDoc, images: Map<string, ImageAsset>): Document {
  const anchors = collectAnchors(doc)
  const children: Paragraph[] = [
    new Paragraph({
      heading: HeadingLevel.TITLE,
      spacing: { after: 160 },
      children: [
        new TextRun({
          text: doc.title,
          bold: true,
          font: FONT,
          size: 40,
        }),
      ],
    }),
  ]

  if (doc.flavor) {
    const quoted = doc.flavor.startsWith('«') ? doc.flavor : `« ${doc.flavor} »`
    children.push(
      new Paragraph({
        spacing: { after: 240 },
        children: [
          new TextRun({
            text: quoted,
            italics: true,
            font: FONT,
            size: 22,
            color: '5C6570',
          }),
        ],
      }),
    )
  }

  for (const section of doc.sections) {
    const blocks = visibleBlocks(section.blocks, images)
    if (blocks.length === 0) continue
    children.push(
      new Paragraph({
        heading: HeadingLevel.HEADING_1,
        spacing: { before: 280, after: 80 },
        children: [
          new TextRun({
            text: section.title,
            bold: true,
            font: FONT,
            size: 28,
            color: '0A5A66',
          }),
        ],
      }),
    )
    for (const block of blocks) {
      if (block.kind === 'spacer') {
        children.push(new Paragraph({ spacing: { after: 120 }, children: [] }))
        continue
      }
      if (block.kind === 'image') {
        children.push(docxImageParagraph(block.src, block.alt, images, false))
        continue
      }
      if (block.kind === 'heading') {
        children.push(
          new Paragraph({
            heading: HeadingLevel.HEADING_2,
            spacing: { before: 200, after: 60 },
            children: bookmark(
              block.anchor,
              docxRuns(block.inlines, images, anchors, { size: 24, bold: true }),
            ),
          }),
        )
        continue
      }
      if (block.kind === 'list') {
        for (const item of block.items) {
          children.push(
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
        continue
      }
      children.push(
        new Paragraph({
          spacing: { after: 120 },
          children: docxRuns(block.inlines, images, anchors, {
            size: 22,
            note: block.note,
          }),
        }),
      )
    }
  }

  return new Document({
    title: doc.title,
    creator: 'Poissonnerie',
    sections: [
      {
        properties: {
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
        children,
      },
    ],
  })
}

function collectAnchors(doc: ScenarioDoc): Set<string> {
  const anchors = new Set<string>()
  for (const section of doc.sections) {
    for (const block of section.blocks) {
      if (block.kind === 'heading' && block.anchor) anchors.add(block.anchor)
    }
  }
  return anchors
}

export function bookmark(
  anchor: string | undefined,
  children: ParagraphChild[],
): ParagraphChild[] {
  if (!anchor) return children
  return [new Bookmark({ id: anchor, children })]
}

export function docxImageParagraph(
  src: string,
  alt: string,
  images: Map<string, ImageAsset>,
  inline: boolean,
): Paragraph {
  const asset = images.get(src)
  if (!asset) {
    return new Paragraph({
      spacing: { after: 120 },
      children: [
        new TextRun({
          text: alt || 'Image',
          italics: true,
          font: FONT,
          size: 20,
          color: '66707A',
        }),
      ],
    })
  }
  const size = fittedSize(
    asset.width,
    asset.height,
    inline ? 480 : 620,
    inline ? 220 : 720,
  )
  return new Paragraph({
    spacing: { before: 80, after: 160 },
    children: [
      new ImageRun({
        type: 'png',
        data: asset.png,
        transformation: size,
        altText: { name: alt, description: alt },
      }),
    ],
  })
}

export function docxRuns(
  inlines: Inline[],
  images: Map<string, ImageAsset>,
  anchors: Set<string>,
  options: { size: number; note?: boolean; bold?: boolean; color?: string },
): ParagraphChild[] {
  const result: ParagraphChild[] = []
  for (const inline of inlines) {
    if (inline.kind === 'break') {
      result.push(new TextRun({ break: 1 }))
      continue
    }
    if (inline.kind === 'image') {
      const asset = images.get(inline.src)
      if (!asset) {
        result.push(
          new TextRun({
            text: inline.alt || 'Image',
            italics: true,
            font: FONT,
            size: options.size,
            color: '66707A',
          }),
        )
        continue
      }
      const size = fittedSize(asset.width, asset.height, 480, 220)
      result.push(
        new ImageRun({
          type: 'png',
          data: asset.png,
          transformation: size,
          altText: { name: inline.alt, description: inline.alt },
        }),
      )
      continue
    }
    if (inline.kind === 'rule') {
      result.push(ruleRun(inline.text, inline.slug, anchors, options))
      continue
    }
    const style: TextStyle = {
      ...inline.style,
      italic: inline.style.italic || options.note,
      bold: inline.style.bold || options.bold,
    }
    result.push(
      new TextRun({
        text: inline.text,
        bold: style.bold,
        italics: style.italic,
        font: FONT,
        size: options.size,
        color: markColorHex(style.color) ?? options.color ?? '1A1F24',
      }),
    )
  }
  if (result.length === 0) {
    result.push(new TextRun({ text: '', font: FONT, size: options.size }))
  }
  return result
}

function ruleRun(
  text: string,
  slug: string,
  anchors: Set<string>,
  options: { size: number; note?: boolean },
): ParagraphChild {
  const anchor = ruleAnchorId(slug)
  const linked = anchors.has(anchor)
  const run = new TextRun({
    text,
    bold: true,
    italics: options.note,
    font: FONT,
    size: options.size,
    color: linked ? '5C6570' : '1A1F24',
    underline: linked
      ? { type: UnderlineType.DOTTED, color: '5C6570' }
      : undefined,
  })
  if (!linked) return run
  return new InternalHyperlink({ anchor, children: [run] })
}
