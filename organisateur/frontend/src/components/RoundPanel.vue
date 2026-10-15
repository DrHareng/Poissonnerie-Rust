<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import PlayerLine from '@/components/PlayerLine.vue'
import RoundResults from '@/components/RoundResults.vue'
import { scoreTone } from '@/format'
import type { MatchCard, PlayerCard, RoundCard, Sectorial } from '@/types'

const props = defineProps<{
  rounds: RoundCard[]
  currentRound: number
  totalRounds: number
  canValidate: boolean
  busy: boolean
  sectorials: Sectorial[]
  saveResult: (payload: {
    player1: number
    p1_op: number
    p1_vp: number
    p2_op: number
    p2_vp: number
  }) => Promise<boolean>
}>()

const emit = defineEmits<{
  drop: [player: PlayerCard, bye: boolean]
  undo: [player: PlayerCard]
  validate: []
}>()

interface Draft {
  p1op: string
  p1vp: string
  p2op: string
  p2vp: string
}

const drafts = reactive<Record<string, Draft>>({})
const editing = ref<Record<string, boolean>>({})

function key(game: MatchCard) {
  return `${game.round}-${game.player1.id}`
}

function serverDraft(game: MatchCard): Draft {
  return {
    p1op: game.p1_op == null ? '' : String(game.p1_op),
    p1vp: game.p1_vp == null ? '' : String(game.p1_vp),
    p2op: game.p2_op == null ? '' : String(game.p2_op),
    p2vp: game.p2_vp == null ? '' : String(game.p2_vp),
  }
}

function writeDraft(draft: Draft, next: Draft) {
  draft.p1op = next.p1op
  draft.p1vp = next.p1vp
  draft.p2op = next.p2op
  draft.p2vp = next.p2vp
}

function dirty(draft: Draft, next: Draft) {
  return draft.p1op !== next.p1op
    || draft.p1vp !== next.p1vp
    || draft.p2op !== next.p2op
    || draft.p2vp !== next.p2vp
}

function syncDrafts() {
  for (const round of props.rounds) {
    for (const game of round.matches) {
      if (game.bye || game.forfeit_player) continue
      const id = key(game)
      const server = serverDraft(game)
      const draft = drafts[id]
      if (!draft) {
        drafts[id] = server
        continue
      }
      const open = !game.scored || editing.value[id] === true
      if (open && dirty(draft, server)) continue
      writeDraft(draft, server)
    }
  }
}

watch(() => props.rounds, syncDrafts, { immediate: true, deep: true })

function parseBounded(raw: string, max: number) {
  if (raw.trim() === '') return null
  const value = Number(raw)
  if (!Number.isInteger(value) || value < 0 || value > max) return null
  return value
}

function draftOf(game: MatchCard): Draft {
  const id = key(game)
  if (!drafts[id]) {
    drafts[id] = { p1op: '', p1vp: '', p2op: '', p2vp: '' }
  }
  return drafts[id]
}

function canSave(game: MatchCard) {
  const draft = draftOf(game)
  return (
    parseBounded(draft.p1op, 10) != null
    && parseBounded(draft.p1vp, 300) != null
    && parseBounded(draft.p2op, 10) != null
    && parseBounded(draft.p2vp, 300) != null
  )
}

function formOpen(game: MatchCard) {
  return !game.scored || editing.value[key(game)] === true
}

async function save(game: MatchCard) {
  const draft = draftOf(game)
  if (!canSave(game)) return false
  const ok = await props.saveResult({
    player1: game.player1.id,
    p1_op: parseBounded(draft.p1op, 10) ?? 0,
    p1_vp: parseBounded(draft.p1vp, 300) ?? 0,
    p2_op: parseBounded(draft.p2op, 10) ?? 0,
    p2_vp: parseBounded(draft.p2vp, 300) ?? 0,
  })
  if (ok) editing.value[key(game)] = false
  return ok
}

function cancelEdit(game: MatchCard) {
  const draft = drafts[key(game)]
  if (draft) writeDraft(draft, serverDraft(game))
  editing.value[key(game)] = false
}

const past = computed(() => props.rounds.filter((round) => round.round !== props.currentRound))
const current = computed(() => props.rounds.find((round) => round.round === props.currentRound))
const closing = computed(() => props.currentRound >= props.totalRounds)

const readyToValidate = computed(() => {
  const games = current.value?.matches ?? []
  if (games.length === 0) return false
  return games.every((game) => {
    if (game.bye || game.forfeit_player) return true
    if (!formOpen(game)) return game.scored
    return canSave(game)
  })
})

async function validateRound() {
  for (const game of current.value?.matches ?? []) {
    if (game.bye || game.forfeit_player || !formOpen(game)) continue
    if (!await save(game)) return
  }
  emit('validate')
}
</script>

<template>
  <section class="stack">
    <h2>Ronde {{ currentRound }} / {{ totalRounds }}</h2>

    <article v-for="game in current?.matches ?? []" :key="key(game)" class="card match">
      <template v-if="game.bye">
        <strong>Bye</strong>
        <PlayerLine :player="game.player1" :sectorials="sectorials">
          <button
            v-if="game.player1.can_undo_drop"
            class="btn compact"
            type="button"
            @click="emit('undo', game.player1)"
          >
            Annuler
          </button>
          <button
            v-else-if="!game.player1.dropped"
            class="btn compact danger"
            type="button"
            @click="emit('drop', game.player1, true)"
          >
            Forfait
          </button>
        </PlayerLine>
      </template>

      <template v-else-if="game.forfeit_player">
        <div class="duel">
          <PlayerLine :player="game.player1" :sectorials="sectorials">
            <button
              v-if="game.player1.can_undo_drop"
              class="btn compact"
              type="button"
              @click="emit('undo', game.player1)"
            >
              Annuler
            </button>
          </PlayerLine>
          <PlayerLine v-if="game.player2" :player="game.player2" :sectorials="sectorials">
            <button
              v-if="game.player2.can_undo_drop"
              class="btn compact"
              type="button"
              @click="emit('undo', game.player2)"
            >
              Annuler
            </button>
          </PlayerLine>
        </div>
      </template>

      <template v-else-if="formOpen(game)">
        <div class="duel">
          <div class="side">
            <PlayerLine :player="game.player1" :sectorials="sectorials">
              <button class="btn compact danger" type="button" @click="emit('drop', game.player1, false)">
                Forfait
              </button>
            </PlayerLine>
            <div class="score-inputs">
              <label>
                <span>OP</span>
                <input v-model="draftOf(game).p1op" class="op" inputmode="numeric" maxlength="2" autocomplete="off" />
              </label>
              <label>
                <span>VP</span>
                <input v-model="draftOf(game).p1vp" class="vp" inputmode="numeric" maxlength="3" autocomplete="off" />
              </label>
            </div>
          </div>
          <div class="side">
            <PlayerLine v-if="game.player2" :player="game.player2" :sectorials="sectorials">
              <button class="btn compact danger" type="button" @click="emit('drop', game.player2!, false)">
                Forfait
              </button>
            </PlayerLine>
            <div class="score-inputs">
              <label>
                <span>OP</span>
                <input v-model="draftOf(game).p2op" class="op" inputmode="numeric" maxlength="2" autocomplete="off" />
              </label>
              <label>
                <span>VP</span>
                <input v-model="draftOf(game).p2vp" class="vp" inputmode="numeric" maxlength="3" autocomplete="off" />
              </label>
            </div>
          </div>
        </div>
        <div class="spread record-row">
          <strong>Table {{ game.table }}</strong>
          <div class="row">
            <button class="btn compact primary" type="button" :disabled="busy || !canSave(game)" @click="save(game)">
              Enregistrer
            </button>
            <button
              v-if="game.scored"
              class="btn compact"
              type="button"
              @click="cancelEdit(game)"
            >
              Annuler
            </button>
          </div>
        </div>
      </template>

      <template v-else>
        <div class="result-line">
          <span>T{{ game.table }}</span>
          <PlayerLine :player="game.player1" :sectorials="sectorials" />
          <span class="score-badge" :class="scoreTone(game, 1)">{{ game.p1_op }} - {{ game.p1_vp }}</span>
          <PlayerLine v-if="game.player2" :player="game.player2" :sectorials="sectorials" />
          <span class="score-badge" :class="scoreTone(game, 2)">{{ game.p2_op }} - {{ game.p2_vp }}</span>
        </div>
        <button class="btn compact" type="button" @click="editing[key(game)] = true">
          Modifier le résultat
        </button>
      </template>
    </article>

    <button class="btn primary" type="button" :disabled="busy || !readyToValidate" @click="validateRound">
      {{ closing ? 'Clôturer le tournoi' : 'Valider la ronde' }}
    </button>

    <details v-for="round in past" :key="round.round">
      <summary>Ronde {{ round.round }}</summary>
      <div class="card">
        <RoundResults :matches="round.matches" />
      </div>
    </details>
  </section>
</template>
