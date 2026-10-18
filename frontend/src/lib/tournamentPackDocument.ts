import {
  buildScenarioDocument,
  parseScenarioMarkdown,
  ruleAnchorId,
  ruleSlugsIn,
  visibleBlocks,
  type Block,
  type ImageAsset,
} from '@/lib/scenarioDocument'
import type {
  CommonRule,
  ScenarioDetail,
  SecondaryObjective,
} from '@/types/elo'

export const PACK_SUBTITLE = 'Pack de missions'

export type TocEntry = {
  label: string
  anchor: string
  level: 1 | 2
}

export type TournamentPackDoc = {
  title: string
  subtitle: string
  toc: TocEntry[]
  blocks: Block[]
}

const TP_MARKER = '## Calcul des points de tournoi'

export function buildTournamentPack(input: {
  tournamentName: string
  preambleMd: string
  secondaries: SecondaryObjective[]
  scenarios: ScenarioDetail[]
  rules: CommonRule[]
}): TournamentPackDoc {
  const labels = new Map<string, string>()
  const bySlug = new Map<string, CommonRule>()
  for (const rule of input.rules) {
    labels.set(rule.slug, rule.name)
    bySlug.set(rule.slug, rule)
  }
  for (const scenario of input.scenarios) {
    for (const rule of scenario.common_rules) {
      labels.set(rule.slug, rule.name)
      bySlug.set(rule.slug, rule)
    }
    if (scenario.exclusion_rule) {
      labels.set(scenario.exclusion_rule.slug, scenario.exclusion_rule.name)
      bySlug.set(scenario.exclusion_rule.slug, scenario.exclusion_rule)
    }
  }
  const ruleLabel = (slug: string) => labels.get(slug) ?? slug

  const blocks: Block[] = []
  const toc: TocEntry[] = []

  pushHeading(blocks, toc, {
    level: 1,
    text: 'Règles générales',
    anchor: 'regles_generales',
    pageBreakBefore: true,
  })

  const markerAt = input.preambleMd.indexOf(TP_MARKER)
  const before =
    markerAt < 0 ? input.preambleMd : input.preambleMd.slice(0, markerAt)
  const after = markerAt < 0 ? '' : input.preambleMd.slice(markerAt)
  blocks.push(...parseScenarioMarkdown(before, ruleLabel))

  if (input.secondaries.length > 0) {
    pushHeading(blocks, toc, {
      level: 2,
      text: 'Récapitulatif des objectifs secondaires',
      anchor: 'recap_secondaires',
    })
    for (const secondary of input.secondaries) {
      pushHeading(blocks, toc, { level: 3, text: secondary.name })
      blocks.push(...parseScenarioMarkdown(secondary.body_md, ruleLabel))
    }
  }

  if (after.trim()) blocks.push(...parseScenarioMarkdown(after, ruleLabel))

  for (const scenario of input.scenarios) {
    const detail = buildScenarioDocument(scenario, input.rules, {
      includeCommonRules: false,
    })
    pushHeading(blocks, toc, {
      level: 1,
      text: scenario.name,
      anchor: anchorId('scn', scenario.slug),
      pageBreakBefore: true,
    })
    if (detail.flavor) {
      const quoted = detail.flavor.startsWith('«')
        ? detail.flavor
        : `« ${detail.flavor} »`
      blocks.push({
        kind: 'paragraph',
        note: true,
        inlines: [{ kind: 'text', text: quoted, style: { italic: true } }],
      })
    }
    for (const section of detail.sections) {
      pushHeading(blocks, toc, { level: 3, text: section.title })
      blocks.push(...stripAnchors(section.blocks))
    }
  }

  const used = collectUsedRules(blocks, input.scenarios, bySlug, ruleLabel)
  if (used.length > 0) {
    pushHeading(blocks, toc, {
      level: 1,
      text: 'Règles communes',
      anchor: 'regles_communes',
      pageBreakBefore: true,
    })
    for (const rule of used) {
      pushHeading(blocks, toc, {
        level: 2,
        text: rule.name,
        anchor: ruleAnchorId(rule.slug),
        blankAfter: true,
      })
      blocks.push(...parseScenarioMarkdown(rule.body_md, ruleLabel))
    }
  }

  return {
    title: input.tournamentName.trim(),
    subtitle: PACK_SUBTITLE,
    toc,
    blocks,
  }
}

export function preparePackBlocks(
  blocks: Block[],
  images: Map<string, ImageAsset>,
): Block[] {
  const visible = visibleBlocks(blocks, images)
  const kept: Block[] = []
  for (let index = 0; index < visible.length; index += 1) {
    const block = visible[index]
    if (!block) continue
    if (block.kind === 'heading' && block.level === 3) {
      const next = visible[index + 1]
      if (!next || next.kind === 'heading') continue
    }
    kept.push(block)
  }
  return kept
}

function pushHeading(
  blocks: Block[],
  toc: TocEntry[],
  options: {
    level: 1 | 2 | 3
    text: string
    anchor?: string
    pageBreakBefore?: boolean
    blankAfter?: boolean
  },
) {
  blocks.push({
    kind: 'heading',
    level: options.level,
    anchor: options.anchor,
    pageBreakBefore: options.pageBreakBefore,
    blankAfter: options.blankAfter,
    inlines: [{ kind: 'text', text: options.text, style: { bold: true } }],
  })
  if (
    options.anchor &&
    (options.level === 1 || options.level === 2)
  ) {
    toc.push({
      label: options.text,
      anchor: options.anchor,
      level: options.level,
    })
  }
}

function stripAnchors(blocks: Block[]): Block[] {
  return blocks.map((block) =>
    block.kind === 'heading' && block.anchor
      ? { ...block, anchor: undefined }
      : block,
  )
}

function collectUsedRules(
  blocks: Block[],
  scenarios: ScenarioDetail[],
  bySlug: Map<string, CommonRule>,
  ruleLabel: (slug: string) => string,
): CommonRule[] {
  const used = new Map<string, CommonRule>()
  const queue: CommonRule[] = []
  const consider = (slug: string) => {
    if (used.has(slug)) return
    const rule = bySlug.get(slug)
    if (!rule) return
    used.set(slug, rule)
    queue.push(rule)
  }

  for (const scenario of scenarios) {
    for (const rule of scenario.common_rules) consider(rule.slug)
    if (scenario.exclusion_rule) consider(scenario.exclusion_rule.slug)
  }
  for (const slug of ruleSlugsIn(blocks)) consider(slug)

  while (queue.length > 0) {
    const rule = queue.shift()
    if (!rule) break
    const body = parseScenarioMarkdown(rule.body_md, ruleLabel)
    for (const slug of ruleSlugsIn(body)) consider(slug)
  }

  return [...used.values()].sort((left, right) =>
    left.name.localeCompare(right.name, 'fr', { sensitivity: 'base' }),
  )
}

function anchorId(prefix: string, slug: string): string {
  const safe = slug.replace(/[^A-Za-z0-9_]/g, '_').replace(/_+/g, '_')
  return `${prefix}_${safe || 'x'}`.slice(0, 40)
}
