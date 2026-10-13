import {
  fetchPackCommonRules,
  fetchPackScenario,
  fetchPackSecondaries,
  fetchScenarioPack,
} from '@/lib/api'
import { buildTournamentPack } from '@/lib/tournamentPackDocument'
import { DEFAULT_SCENARIO_PACK_SLUG } from '@/types/elo'

export async function downloadMissionPack(
  format: 'docx' | 'pdf',
  tournamentName: string,
  scenarioSlugs: string[],
): Promise<number> {
  const [page, secondaries, rules, scenarios] = await Promise.all([
    fetchScenarioPack(DEFAULT_SCENARIO_PACK_SLUG),
    fetchPackSecondaries(DEFAULT_SCENARIO_PACK_SLUG),
    fetchPackCommonRules(DEFAULT_SCENARIO_PACK_SLUG),
    Promise.all(
      scenarioSlugs.map((slug) =>
        fetchPackScenario(DEFAULT_SCENARIO_PACK_SLUG, slug),
      ),
    ),
  ])
  const doc = buildTournamentPack({
    tournamentName,
    preambleMd: page.pack.preamble_md,
    secondaries,
    scenarios,
    rules,
  })
  if (format === 'docx') {
    const mod = await import('@/lib/tournamentPackDocx')
    return mod.downloadTournamentPackDocx(doc)
  }
  const mod = await import('@/lib/tournamentPackPdf')
  return mod.downloadTournamentPackPdf(doc)
}
