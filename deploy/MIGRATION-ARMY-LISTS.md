# Migration `army_lists` — déploiement production

Cette migration est **automatique** : elle s'exécute au premier accès SQLite après déploiement du binaire (via `migrate()` dans `ArmyStore::open`, `TournamentStore::open`, `ArmyListStore::open`, `Leaderboard::load`).

## Ce que fait la migration

1. Crée la table `army_lists` (`id`, `code` UNIQUE, `army_id` → `armies`)
2. Ajoute les colonnes FK sur `matches`, `tournament_matches`, `tournament_registrations`
3. **Backfill** : pour chaque code existant dans les colonnes `*_army_list_code` / `army_list_*` / `bracket_list_*`, get-or-create dans `army_lists` et remplit les FK

Les colonnes texte historiques sont **conservées** (compatibilité API / rollback).

## Procédure recommandée (VPS `/opt/poissonnerie`)

### 1. Sauvegarder la base **avant** le déploiement

```bash
sudo systemctl stop poissonnerie   # optionnel mais plus sûr
cp /opt/poissonnerie/data/poissonnerie.db \
   /opt/poissonnerie/data/poissonnerie.db.bak-$(date +%Y%m%d-%H%M)
sudo systemctl start poissonnerie    # si arrêté
```

Conserver au moins une copie hors disque (rsync, snapshot VPS, etc.).

### 2. Déployer normalement

```bash
cd /opt/poissonnerie
sudo -u poissonnerie ./deploy/deploy.sh
```

Le script fait `git pull`, `cargo build --release`, build frontend, `systemctl restart poissonnerie`.

### 3. Vérifier la migration

```bash
# Logs au démarrage (aucune erreur SQL attendue)
sudo journalctl -u poissonnerie -n 50 --no-pager

# Santé API
curl -sf http://127.0.0.1:3000/api/health

# Spot-check listes
sqlite3 /opt/poissonnerie/data/poissonnerie.db <<'SQL'
SELECT COUNT(*) AS army_lists FROM army_lists;
SELECT COUNT(*) AS matches_with_list FROM matches
  WHERE player1_army_list_id IS NOT NULL OR player2_army_list_id IS NOT NULL;
SELECT COUNT(*) AS orphan_codes FROM matches m
  WHERE (m.player1_army_list_code IS NOT NULL AND m.player1_army_list_code != '' AND m.player1_army_list_id IS NULL)
     OR (m.player2_army_list_code IS NOT NULL AND m.player2_army_list_code != '' AND m.player2_army_list_id IS NULL);
SQL
```

`orphan_codes` devrait être **0** (codes non parsables ignorés silencieusement lors du backfill).

Nouvel endpoint :

```bash
curl -sf 'http://127.0.0.1:3000/api/army-lists/armies' | head
curl -sf 'http://127.0.0.1:3000/api/army-lists?army_ids=1' | head
```

Frontend : onglet **Matchs → Listes** (`/matchs/listes`).

### 4. Rollback (si problème)

`deploy/deploy.sh` restaure automatiquement le binaire et le frontend précédents en cas d'échec **pendant** le script.

Rollback manuel complet :

```bash
sudo systemctl stop poissonnerie
cp /opt/poissonnerie/data/poissonnerie.db.bak-YYYYMMDD-HHMM \
   /opt/poissonnerie/data/poissonnerie.db
git -C /opt/poissonnerie reset --hard <commit-avant-deploy>
cargo build --release -p poissonnerie-elo --bin poissonnerie-server
sudo systemctl start poissonnerie
```

> **Note** : SQLite ne supprime pas les colonnes/tables ajoutées si vous ne restaurez pas la sauvegarde `.db`. Pour un rollback schéma propre, restaurer le fichier `.db` sauvegardé.

## Déploiement local / dev

Aucune action manuelle : lancer le serveur suffit.

```bash
cargo run --bin poissonnerie-server
```

La base `data/poissonnerie.db` est migrée au démarrage.

## Rejouer le backfill manuellement (rare)

Si des codes ont été insérés en SQL brut sans passer par l'API :

```bash
sqlite3 /opt/poissonnerie/data/poissonnerie.db
# Puis redémarrer le serveur — migrate() rappelle backfill_army_list_references
# qui ne remplit que les FK encore NULL.
sudo systemctl restart poissonnerie
```

Ou via Rust (depuis le repo, avec `POISSONNERIE_DB` pointant vers la prod si besoin) :

```bash
POISSONNERIE_DB=/opt/poissonnerie/data/poissonnerie.db \
  cargo run --bin poissonnerie-server &
# Arrêter après démarrage réussi — migrate() a été exécutée.
```

## Checklist rapide

- [ ] Backup `poissonnerie.db` horodaté
- [ ] `./deploy/deploy.sh`
- [ ] `curl /api/health` OK
- [ ] `COUNT(*)` sur `army_lists` > 0 (si matchs historiques avec listes)
- [ ] `orphan_codes` = 0
- [ ] Onglet Listes visible et filtre sectorielles OK
