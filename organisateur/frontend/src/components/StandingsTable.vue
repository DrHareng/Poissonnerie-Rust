<script setup lang="ts">
import { armyLogoUrl, formatPoints } from '@/format'
import type { Sectorial, StandingRow } from '@/types'

const props = defineProps<{
  rows: StandingRow[]
  finalRanking: boolean
  sectorials: Sectorial[]
}>()

function logo(name: string) {
  const slug = props.sectorials.find((item) => item.name === name)?.slug
  return slug ? armyLogoUrl(slug) : ''
}
</script>

<template>
  <div class="card">
    <h2>{{ finalRanking ? 'Classement final' : 'Classement' }}</h2>
    <table>
      <thead>
        <tr>
          <th>#</th>
          <th>Joueur</th>
          <th>TP</th>
          <th>{{ finalRanking ? 'OP fin.' : 'OP' }}</th>
          <th>{{ finalRanking ? 'VP fin.' : 'VP' }}</th>
          <th>V/N/D</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in rows" :key="row.player_id">
          <td>{{ row.rank }}</td>
          <td>
            <span class="player-line">
              <img
                v-if="logo(row.sectorial_name)"
                class="army-logo"
                :src="logo(row.sectorial_name)"
                :alt="row.sectorial_name"
                :title="row.sectorial_name"
              />
              <span>{{ row.name }}</span>
            </span>
            <span v-if="row.dropped" class="badge">Forfait</span>
            <span v-if="row.byes" class="badge">{{ row.byes }} bye</span>
          </td>
          <td>{{ row.tp }}</td>
          <td>{{ finalRanking ? formatPoints(row.op_final) : row.op }}</td>
          <td>{{ finalRanking ? formatPoints(row.vp_final) : row.vp }}</td>
          <td>{{ row.wins }}/{{ row.draws }}/{{ row.losses }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
