<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { toast } from 'vue-sonner'
import { ArrowLeft, BookmarkPlus, FileText, Trash2 } from '@lucide/vue'
import {
  createReportTemplate,
  deleteReportTemplate,
  fetchMatch,
  fetchReportTemplates,
  updateMatchReport,
  updateReportTemplate,
} from '@/lib/api'
import type { MatchRecord, ReportStatus, ReportTemplate } from '@/types/elo'
import ArmyLogo from '@/components/ArmyLogo.vue'
import MarkdownEditor from '@/components/MarkdownEditor.vue'
import MatchResultBadges from '@/components/MatchResultBadges.vue'
import PlayerLink from '@/components/PlayerLink.vue'
import PageTitleTabs from '@/components/PageTitleTabs.vue'
import { useAuth } from '@/composables/useAuth'
import { matchsTabs } from '@/lib/pageTitleTabs'
import {
  applyReportTemplate,
  BUILTIN_REPORT_TEMPLATES,
  samePlayerName,
} from '@/lib/reportTemplates'
import { formatMatchRecordedDate } from '@/lib/tournamentMatchDisplay'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

const route = useRoute()
const router = useRouter()
const { player, isAuthenticated, initialized, login } = useAuth()

const match = ref<MatchRecord | null>(null)
const loading = ref(true)
const body = ref('')
const savedBody = ref('')
const savedStatus = ref<ReportStatus | null>(null)
const templates = ref<ReportTemplate[]>([])
const selectedKey = ref('builtin:classique')
const templateName = ref('')
const saving = ref(false)
const publishing = ref(false)

const matchId = computed(() => Number(route.params.id))

const myReport = computed(() => {
  if (!match.value || !player.value) return null
  if (samePlayerName(player.value.name, match.value.player1)) {
    return match.value.player1_report ?? null
  }
  if (samePlayerName(player.value.name, match.value.player2)) {
    return match.value.player2_report ?? null
  }
  return null
})

const isParticipant = computed(() => {
  if (!match.value || !player.value) return false
  return (
    samePlayerName(player.value.name, match.value.player1) ||
    samePlayerName(player.value.name, match.value.player2)
  )
})

const matchCompleted = computed(
  () => Boolean(match.value) && match.value?.status !== 'in_progress',
)

const canEdit = computed(
  () => isAuthenticated.value && isParticipant.value && matchCompleted.value,
)

const selectedUserTemplate = computed(() => {
  if (!selectedKey.value.startsWith('user:')) return null
  const id = Number(selectedKey.value.slice(5))
  return templates.value.find((item) => item.id === id) ?? null
})

const selectedBuiltin = computed(() => {
  if (!selectedKey.value.startsWith('builtin:')) return null
  const id = selectedKey.value.slice(8)
  return BUILTIN_REPORT_TEMPLATES.find((item) => item.id === id) ?? null
})

const dirty = computed(() => body.value !== savedBody.value)

function applyLoadedReport(record: MatchRecord) {
  match.value = record
  const report = myReport.value
  body.value = report?.body_md ?? ''
  savedBody.value = body.value
  savedStatus.value = report?.status === 'draft' ? 'draft' : report ? 'published' : null
}

async function load() {
  loading.value = true
  try {
    const [record, userTemplates] = await Promise.all([
      fetchMatch(matchId.value),
      isAuthenticated.value
        ? fetchReportTemplates().catch(() => [] as ReportTemplate[])
        : Promise.resolve([] as ReportTemplate[]),
    ])
    templates.value = userTemplates
    applyLoadedReport(record)
  } catch (error) {
    match.value = null
    toast.error(error instanceof Error ? error.message : 'Match introuvable')
  } finally {
    loading.value = false
  }
}

function redirectToMatch(message: string) {
  toast.error(message)
  router.replace({ name: 'match', params: { id: String(matchId.value) } })
}

watch(
  initialized,
  (ready) => {
    if (ready) void load()
  },
  { immediate: true },
)

let redirected = false
watch(
  [initialized, loading, match, isAuthenticated, player],
  () => {
    if (redirected || !initialized.value || loading.value || !match.value) return
    if (match.value.status === 'in_progress') {
      redirected = true
      redirectToMatch('Le compte rendu est disponible une fois le match terminé.')
      return
    }
    if (isAuthenticated.value && player.value && !isParticipant.value) {
      redirected = true
      redirectToMatch('Seul un participant peut rédiger un compte rendu.')
    }
  },
)

function sourceForSelection(): string | null {
  if (selectedBuiltin.value) return selectedBuiltin.value.body_md
  if (selectedUserTemplate.value) return selectedUserTemplate.value.body_md
  return null
}

function applyTemplate() {
  if (!match.value || !player.value) return
  const source = sourceForSelection()
  if (source == null) {
    toast.error('Choisissez un modèle')
    return
  }
  if (body.value.trim() && !window.confirm('Remplacer le texte actuel par ce modèle ?')) {
    return
  }
  body.value = applyReportTemplate(source, match.value, player.value.name)
}

async function persist(status: ReportStatus): Promise<boolean> {
  if (!match.value) return false
  const busy = status === 'published' ? publishing : saving
  busy.value = true
  try {
    const wasPublished = savedStatus.value === 'published'
    const record = await updateMatchReport(match.value.id, body.value, status)
    applyLoadedReport(record)
    toast.success(
      status === 'published'
        ? 'Compte rendu publié'
        : wasPublished
          ? 'Compte rendu dépublié'
          : 'Brouillon enregistré',
    )
    return true
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Enregistrement impossible')
    return false
  } finally {
    busy.value = false
  }
}

async function saveDraft() {
  await persist('draft')
}

async function publish() {
  if (!body.value.trim()) {
    toast.error('Le compte rendu ne peut pas être vide')
    return
  }
  await persist('published')
}

async function unpublish() {
  if (!window.confirm('Dépublier ce compte rendu ? Il ne sera plus visible publiquement.')) {
    return
  }
  await persist('draft')
}

async function refreshTemplates(selectId?: number) {
  templates.value = await fetchReportTemplates()
  if (selectId != null) {
    selectedKey.value = `user:${selectId}`
  }
}

async function saveNewTemplate() {
  const name = templateName.value.trim()
  if (!name) {
    toast.error('Indiquez un nom de modèle')
    return
  }
  if (!body.value.trim()) {
    toast.error('Le modèle est vide')
    return
  }
  try {
    const created = await createReportTemplate({ name, body_md: body.value })
    templateName.value = ''
    await refreshTemplates(created.id)
    toast.success('Modèle enregistré')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible d’enregistrer le modèle')
  }
}

async function overwriteTemplate() {
  const current = selectedUserTemplate.value
  if (!current) return
  const name = templateName.value.trim() || current.name
  try {
    const updated = await updateReportTemplate(current.id, {
      name,
      body_md: body.value,
    })
    templateName.value = ''
    await refreshTemplates(updated.id)
    toast.success('Modèle mis à jour')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Impossible de mettre à jour le modèle')
  }
}

async function removeTemplate() {
  const current = selectedUserTemplate.value
  if (!current) return
  if (!window.confirm(`Supprimer le modèle « ${current.name} » ?`)) return
  try {
    await deleteReportTemplate(current.id)
    selectedKey.value = 'builtin:classique'
    await refreshTemplates()
    toast.success('Modèle supprimé')
  } catch (error) {
    toast.error(error instanceof Error ? error.message : 'Suppression impossible')
  }
}
</script>

<template>
  <div class="page-stack">
    <PageTitleTabs
      :tabs="matchsTabs"
      ariaLabel="Sections des matchs"
      :current="{ label: 'Compte rendu' }"
    />

    <section class="page-header">
      <div class="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          size="sm"
          variant="outline"
          as-child
        >
          <RouterLink :to="{ name: 'match', params: { id: String(matchId) } }">
            <ArrowLeft class="size-4" />
            Retour au match
          </RouterLink>
        </Button>
        <Badge v-if="savedStatus === 'draft'" variant="secondary">Brouillon</Badge>
        <Badge v-else-if="savedStatus === 'published'" variant="outline">Publié</Badge>
        <span v-if="dirty" class="text-xs text-muted-foreground">Modifications non enregistrées</span>
      </div>
    </section>

    <Alert v-if="!loading && !match" variant="destructive" class="neon-panel-accent">
      <AlertTitle>Match introuvable</AlertTitle>
      <AlertDescription>
        Ce match n'existe pas ou l'API est indisponible.
      </AlertDescription>
    </Alert>

    <div v-else-if="loading || !initialized" class="text-sm text-muted-foreground">
      Chargement…
    </div>

    <div v-else-if="match && !isAuthenticated" class="space-y-3">
      <p class="text-sm text-muted-foreground">
        Connectez-vous avec Discord pour rédiger le compte rendu de ce match.
      </p>
      <Button type="button" size="sm" @click="login">Connexion</Button>
    </div>

    <div v-else-if="match && canEdit" class="page-panel-scroll min-h-0 flex-1 space-y-4 overflow-y-auto">
      <Card class="neon-panel">
        <CardHeader class="pb-3">
          <CardTitle class="flex items-center gap-2 text-base">
            <FileText class="size-5 text-primary" />
            {{ match.player1_display_name ?? match.player1 }}
            vs
            {{ match.player2_display_name ?? match.player2 }}
          </CardTitle>
          <CardDescription>
            {{ formatMatchRecordedDate(match.recorded_at) ?? 'Date inconnue' }}
            <template v-if="match.scenario_name">
              · {{ match.scenario_name }}
            </template>
          </CardDescription>
        </CardHeader>
        <CardContent class="flex flex-wrap items-center gap-3">
          <div class="flex min-w-0 items-center gap-1.5">
            <PlayerLink
              :name="match.player1"
              :display-name="match.player1_display_name"
              class="truncate text-sm"
            />
            <ArmyLogo :army-id="match.player1_army_id" />
          </div>
          <MatchResultBadges :match="match" />
          <div class="flex min-w-0 items-center gap-1.5">
            <ArmyLogo :army-id="match.player2_army_id" />
            <PlayerLink
              :name="match.player2"
              :display-name="match.player2_display_name"
              class="truncate text-sm"
            />
          </div>
        </CardContent>
      </Card>

      <Card class="neon-panel">
        <CardHeader>
          <CardTitle class="text-base">Rédiger le compte rendu</CardTitle>
          <CardDescription>
            Un seul bloc markdown. Appliquez un modèle pour pré-remplir les sections
            (listes, déploiement, tours) avec les pseudos du match.
          </CardDescription>
        </CardHeader>
        <CardContent class="space-y-4">
          <div class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto]">
            <div class="space-y-1.5">
              <Label>Modèle</Label>
              <Select v-model="selectedKey">
                <SelectTrigger class="w-full">
                  <SelectValue placeholder="Choisir un modèle" />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectLabel>Proposés</SelectLabel>
                    <SelectItem
                      v-for="item in BUILTIN_REPORT_TEMPLATES"
                      :key="item.id"
                      :value="`builtin:${item.id}`"
                    >
                      {{ item.name }}
                    </SelectItem>
                  </SelectGroup>
                  <SelectGroup v-if="templates.length">
                    <SelectLabel>Mes modèles</SelectLabel>
                    <SelectItem
                      v-for="item in templates"
                      :key="item.id"
                      :value="`user:${item.id}`"
                    >
                      {{ item.name }}
                    </SelectItem>
                  </SelectGroup>
                </SelectContent>
              </Select>
            </div>
            <div class="flex items-end">
              <Button type="button" variant="outline" @click="applyTemplate">
                Appliquer
              </Button>
            </div>
          </div>

          <p class="text-xs text-muted-foreground">
            Pour réutiliser un modèle, laissez les jetons
            <code>[joueur1]</code>, <code>[joueur2]</code>,
            <code>[deploy1]</code>, <code>[deploy2]</code>,
            <code>[moi]</code> et <code>[adversaire]</code> dans le texte
            avant de l’enregistrer.
          </p>

          <div class="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-end">
            <div class="space-y-1.5">
              <Label for="template-name">Enregistrer comme modèle</Label>
              <Input
                id="template-name"
                v-model="templateName"
                placeholder="Nom du modèle"
                autocomplete="off"
              />
            </div>
            <div class="flex flex-wrap gap-2">
              <Button type="button" variant="outline" size="sm" @click="saveNewTemplate">
                <BookmarkPlus class="size-4" />
                Nouveau
              </Button>
              <Button
                v-if="selectedUserTemplate"
                type="button"
                variant="outline"
                size="sm"
                @click="overwriteTemplate"
              >
                Mettre à jour
              </Button>
              <Button
                v-if="selectedUserTemplate"
                type="button"
                variant="ghost"
                size="sm"
                @click="removeTemplate"
              >
                <Trash2 class="size-4" />
                Supprimer
              </Button>
            </div>
          </div>

          <div class="match-report-editor">
            <MarkdownEditor
              v-model="body"
              :rows="22"
              simple
              placeholder="Racontez la partie…"
            />
          </div>

          <div class="flex flex-wrap justify-end gap-2">
            <Button
              v-if="savedStatus === 'published'"
              type="button"
              variant="ghost"
              :disabled="saving || publishing"
              @click="unpublish"
            >
              Dépublier
            </Button>
            <Button
              type="button"
              variant="outline"
              :disabled="saving || publishing"
              @click="saveDraft"
            >
              {{ saving ? 'Enregistrement…' : 'Enregistrer le brouillon' }}
            </Button>
            <Button
              type="button"
              :disabled="saving || publishing"
              @click="publish"
            >
              {{ publishing ? 'Publication…' : 'Publier' }}
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>

    <p v-else-if="match" class="text-sm text-muted-foreground">
      Seul un joueur ayant participé à ce match terminé peut en rédiger le compte rendu.
    </p>
  </div>
</template>

<style scoped>
.match-report-editor :deep(textarea) {
  min-height: min(60vh, 36rem);
}
</style>
