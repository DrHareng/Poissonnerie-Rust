#!/usr/bin/env bash
set -euo pipefail

APP_DIR="/opt/poissonnerie"
BACKUP_DIR="$APP_DIR/.deploy-backup/previous"
SERVER_BIN="$APP_DIR/target/release/poissonnerie-server"
FRONTEND_DIST="$APP_DIR/frontend/dist"
HEALTH_URL="${DEPLOY_HEALTH_URL:-http://127.0.0.1:3000/api/health}"

ROLLBACK_ENABLED=0

log() {
    printf '[deploy] %s\n' "$*"
}

warn() {
    printf '[deploy] ATTENTION: %s\n' "$*" >&2
}

die() {
    printf '[deploy] ERREUR: %s\n' "$*" >&2
    exit 1
}

backup_current_release() {
    log "Sauvegarde de la version en cours…"
    rm -rf "$BACKUP_DIR"
    mkdir -p "$BACKUP_DIR"

    git -C "$APP_DIR" rev-parse HEAD >"$BACKUP_DIR/commit"
    git -C "$APP_DIR" rev-parse --abbrev-ref HEAD >"$BACKUP_DIR/branch"

    if [[ -x "$SERVER_BIN" ]]; then
        cp "$SERVER_BIN" "$BACKUP_DIR/poissonnerie-server"
    fi

    if [[ -d "$FRONTEND_DIST" ]]; then
        cp -a "$FRONTEND_DIST" "$BACKUP_DIR/dist"
    fi
}

rollback() {
    if [[ "$ROLLBACK_ENABLED" != 1 ]]; then
        return 0
    fi

    warn "Rollback vers la version précédente…"

    if [[ -f "$BACKUP_DIR/poissonnerie-server" ]]; then
        cp "$BACKUP_DIR/poissonnerie-server" "$SERVER_BIN"
        chmod +x "$SERVER_BIN"
    fi

    if [[ -d "$BACKUP_DIR/dist" ]]; then
        rm -rf "$FRONTEND_DIST"
        cp -a "$BACKUP_DIR/dist" "$FRONTEND_DIST"
    fi

    if [[ -f "$BACKUP_DIR/commit" ]]; then
        local prev_commit
        prev_commit="$(cat "$BACKUP_DIR/commit")"
        git -C "$APP_DIR" reset --hard "$prev_commit"
        log "Code source restauré sur $prev_commit"
    fi

    if systemctl is-active --quiet poissonnerie 2>/dev/null; then
        sudo systemctl restart poissonnerie
    elif [[ -x "$SERVER_BIN" ]]; then
        sudo systemctl restart poissonnerie
    fi

    if command -v nginx >/dev/null 2>&1; then
        sudo systemctl reload nginx || true
    fi

    die "Déploiement annulé — version précédente restaurée."
}

on_error() {
    rollback
}

verify_frontend_dist() {
    [[ -f "$FRONTEND_DIST/index.html" ]] || die "frontend/dist/index.html introuvable après build"
    local asset_count
    asset_count="$(find "$FRONTEND_DIST/assets" -type f 2>/dev/null | wc -l | tr -d ' ')"
    [[ "$asset_count" -gt 0 ]] || die "frontend/dist/assets est vide après build"
    log "Frontend OK ($asset_count fichiers dans dist/assets)"
}

verify_health() {
    local retries=15
    local i
    for ((i = 1; i <= retries; i++)); do
        if curl -sf "$HEALTH_URL" >/dev/null 2>&1; then
            log "API OK ($HEALTH_URL)"
            return 0
        fi
        sleep 1
    done
    die "L'API ne répond pas après redémarrage ($HEALTH_URL)"
}

reload_nginx_if_present() {
    if command -v nginx >/dev/null 2>&1 && systemctl is-active --quiet nginx 2>/dev/null; then
        log "reload nginx"
        sudo nginx -t
        sudo systemctl reload nginx
    fi
}

log "Déploiement Poissonnerie dans $APP_DIR"
cd "$APP_DIR"

prev_commit="$(git rev-parse HEAD)"
prev_branch="$(git rev-parse --abbrev-ref HEAD)"
backup_current_release

ROLLBACK_ENABLED=1
trap on_error ERR

log "Branche: $prev_branch — commit actuel: ${prev_commit:0:8}"

log "git pull"
git pull --ff-only

new_commit="$(git rev-parse HEAD)"
if [[ "$new_commit" == "$prev_commit" ]]; then
    warn "git pull n'a apporté aucun nouveau commit."
    warn "Les changements visuels nécessitent un push git avant deploy."
    warn "Continuer quand même (rebuild frontend + backend)…"
else
    log "Nouveaux commits:"
    git --no-pager log --oneline "$prev_commit..$new_commit" | sed 's/^/[deploy]   /'
fi

log "cargo build --release"
cargo build --release -p poissonnerie-elo --bin poissonnerie-server

log "build frontend"
(
    cd frontend
    npm ci
    npm run build
)
verify_frontend_dist

log "restart poissonnerie"
sudo systemctl restart poissonnerie
verify_health
reload_nginx_if_present

ROLLBACK_ENABLED=0
trap - ERR

log "Terminé — commit ${new_commit:0:8}, frontend $(stat -c '%y' "$FRONTEND_DIST/index.html" 2>/dev/null || stat -f '%Sm' "$FRONTEND_DIST/index.html")"
log "Si le visuel n'a pas changé dans le navigateur : Ctrl+Shift+R (hard refresh)."
