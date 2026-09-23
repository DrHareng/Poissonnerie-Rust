<script setup lang="ts">
import { withBase } from '@/lib/basePath'

const upcomingArt = withBase('/resources/fond_in_progress.png')

/** Placeholders — à brancher sur l'édition courante plus tard. */
const upcoming = {
  title: 'Dauphiné 2026',
  dates: 'Dates à confirmer',
  address: 'Adresse à confirmer',
  tagline: 'Tournoi annuel scénarisé, par équipes.',
}

const historyLorem = `
Autrefois, sous les brumes du Dauphiné, des compagnies rivales se croisaient
sur des routes oubliées. Chaque année, le pacte était renoué : une épreuve
scénarisée, des équipes soudées, une narration qui avance au fil des rondes.

Ici viendra le résumé de l'histoire — les éditions passées, les factions,
les moments qui ont marqué le club. Pour l'instant, ce texte tient lieu
de placeholder jusqu'à la rédaction définitive.
`.trim()

const historyParagraphs = historyLorem
  .split(/\n\n+/)
  .map((paragraph) => paragraph.replace(/\s+/g, ' ').trim())
  .filter(Boolean)

const placeholderPhotos = [
  'Photo 1',
  'Photo 2',
  'Photo 3',
  'Photo 4',
  'Photo 5',
]

const placeholderRanking = [
  { place: 1, team: 'Équipe Alpha' },
  { place: 2, team: 'Équipe Bravo' },
  { place: 3, team: 'Équipe Charlie' },
  { place: 4, team: 'Équipe Delta' },
  { place: 5, team: 'Équipe Echo' },
  { place: 6, team: 'Équipe Foxtrot' },
]
</script>

<template>
  <main class="gate">
    <section class="gate-col gate-col--upcoming" aria-labelledby="gate-upcoming-title">
      <h2 id="gate-upcoming-title" class="gate-title">Le Dauphiné à venir</h2>
      <div class="gate-upcoming">
        <div class="gate-upcoming-hero">
          <img
            class="gate-upcoming-art"
            :src="upcomingArt"
            alt=""
          />
        </div>

        <div class="gate-upcoming-content">
          <div class="gate-practical">
            <p class="gate-practical-edition">{{ upcoming.title }}</p>
            <p class="gate-practical-tagline">{{ upcoming.tagline }}</p>
            <dl class="gate-practical-list">
              <div class="gate-practical-row">
                <dt>Dates</dt>
                <dd>{{ upcoming.dates }}</dd>
              </div>
              <div class="gate-practical-row">
                <dt>Lieu</dt>
                <dd>{{ upcoming.address }}</dd>
              </div>
            </dl>
            <RouterLink class="gate-cta" to="/inscription">
              S'inscrire
            </RouterLink>
          </div>

          <div class="gate-history" aria-labelledby="gate-history-title">
            <h3 id="gate-history-title" class="gate-subtitle">L'histoire jusqu'ici</h3>
            <p
              v-for="(paragraph, index) in historyParagraphs"
              :key="index"
              class="gate-prose"
            >
              {{ paragraph }}
            </p>
          </div>
        </div>
      </div>
    </section>

    <section class="gate-col gate-col--previous" aria-labelledby="gate-previous-title">
      <h2 id="gate-previous-title" class="gate-title">Le précédent Dauphiné</h2>
      <div class="gate-previous">
        <div class="gate-photos" aria-label="Photos du précédent Dauphiné">
          <div class="gate-photos-track">
            <div
              v-for="(label, index) in [...placeholderPhotos, ...placeholderPhotos]"
              :key="`${label}-${index}`"
              class="gate-photo-slide"
            >
              {{ label }}
            </div>
          </div>
        </div>

        <div class="gate-ranking" aria-label="Classement du précédent Dauphiné">
          <h3 class="gate-ranking-title">Classement</h3>
          <ol class="gate-ranking-list">
            <li
              v-for="row in placeholderRanking"
              :key="row.place"
              class="gate-ranking-row"
            >
              <span class="gate-ranking-place">{{ row.place }}</span>
              <span class="gate-ranking-team">{{ row.team }}</span>
            </li>
          </ol>
        </div>
      </div>
    </section>
  </main>
</template>
