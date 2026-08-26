#!/usr/bin/env bash
set -euo pipefail

APP_DIR="/opt/poissonnerie"

log() {
    printf '[deploy] %s\n' "$*"
}

log "Déploiement Poissonnerie dans $APP_DIR"

cd "$APP_DIR"

log "git pull"
git pull

log "cargo build --release"
cargo build --release -p poissonnerie-elo --bin poissonnerie-server

log "build frontend"
(
    cd frontend
    npm ci
    npm run build
)

log "restart poissonnerie"
sudo systemctl restart poissonnerie

log "Terminé."
