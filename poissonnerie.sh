#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUN_DIR="$ROOT_DIR/.run"
LOG_DIR="$RUN_DIR/logs"
SERVER_PID_FILE="$RUN_DIR/server.pid"
FRONTEND_PID_FILE="$RUN_DIR/frontend.pid"
SERVER_LOG="$LOG_DIR/server.log"
FRONTEND_LOG="$LOG_DIR/frontend.log"

case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*)
        EXE_EXT=".exe"
        HAS_SETSID=0
        ;;
    *)
        EXE_EXT=""
        HAS_SETSID=1
        command -v setsid >/dev/null 2>&1 || HAS_SETSID=0
        ;;
esac

SERVER_BIN="$ROOT_DIR/target/debug/poissonnerie-server${EXE_EXT}"
SYNC_BIN="$ROOT_DIR/target/debug/poissonnerie-sync-armies${EXE_EXT}"
IMPORT_BIN="$ROOT_DIR/target/debug/poissonnerie-import-coupe${EXE_EXT}"
SERVER_ADDR="127.0.0.1:3000"
FRONTEND_ADDR="127.0.0.1:5173"

usage() {
    cat <<EOF
Usage: $(basename "$0") {start|stop|restart|status|import-coupe|import-coupes}

  start          Synchronise les armées, lance l'API Rust et le frontend Vite
  stop           Arrête l'API et le frontend
  restart        stop puis start
  status         Affiche l'état des services
  import-coupe   Importe une coupe (5-10) : import-coupe 5 [--dry-run] [--force]
  import-coupes  Importe les coupes 5 à 10 dans l'ordre (options cargo passées après --)
EOF
}

log() {
    printf '[poissonnerie] %s\n' "$*"
}

err() {
    printf '[poissonnerie] ERREUR: %s\n' "$*" >&2
}

ensure_dirs() {
    mkdir -p "$LOG_DIR"
}

load_node() {
    if [[ -s "${NVM_DIR:-$HOME/.nvm}/nvm.sh" ]]; then
        # shellcheck source=/dev/null
        source "${NVM_DIR:-$HOME/.nvm}/nvm.sh"
    fi
}

pid_is_running() {
    local pid="$1"
    [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null
}

read_pid() {
    local file="$1"
    if [[ -f "$file" ]]; then
        tr -d '[:space:]' <"$file"
    fi
}

port_is_open() {
    local addr="$1"
    local host="${addr%:*}"
    local port="${addr##*:}"
    timeout 1 bash -c "echo >/dev/tcp/$host/$port" 2>/dev/null
}

wait_for_port() {
    local addr="$1"
    local label="$2"
    local retries="${3:-30}"

    for ((i = 1; i <= retries; i++)); do
        if port_is_open "$addr"; then
            return 0
        fi
        sleep 1
    done

    err "$label ne répond pas sur $addr (voir les logs dans $LOG_DIR)"
    return 1
}

build_rust_bins() {
    log "Compilation des binaires Rust..."
    cargo build --target-dir "$ROOT_DIR/target" --bin poissonnerie-server --bin poissonnerie-sync-armies

    if [[ ! -x "$SERVER_BIN" ]]; then
        err "binaire introuvable après compilation : $SERVER_BIN"
        exit 1
    fi
}

ensure_frontend_deps() {
    local frontend_dir="$ROOT_DIR/frontend"
    local needs_install=0

    if [[ ! -d "$frontend_dir/node_modules" ]]; then
        needs_install=1
    elif [[ "$EXE_EXT" == ".exe" ]] && [[ ! -f "$frontend_dir/node_modules/.bin/vite.cmd" ]]; then
        needs_install=1
    fi

    if [[ "$needs_install" == 1 ]]; then
        log "Installation des dépendances npm..."
        npm --prefix "$frontend_dir" install
    fi
}

sync_armies() {
    log "Synchronisation des armées..."
    "$SYNC_BIN"
}

start_server() {
    local pid
    pid="$(read_pid "$SERVER_PID_FILE")"
    if pid_is_running "$pid"; then
        log "API déjà en cours (PID $pid)"
        return 0
    fi

    if port_is_open "$SERVER_ADDR"; then
        err "le port $SERVER_ADDR est déjà utilisé"
        return 1
    fi

    : >"$SERVER_LOG"
    (
        cd "$ROOT_DIR"
        if [[ "$HAS_SETSID" == 1 ]]; then
            exec setsid "$SERVER_BIN" --listen "$SERVER_ADDR"
        else
            exec "$SERVER_BIN" --listen "$SERVER_ADDR"
        fi
    ) >>"$SERVER_LOG" 2>&1 &
    echo $! >"$SERVER_PID_FILE"

    wait_for_port "$SERVER_ADDR" "API" || return 1
    log "API démarrée (PID $(read_pid "$SERVER_PID_FILE"), http://$SERVER_ADDR)"
}

start_frontend() {
    local pid
    pid="$(read_pid "$FRONTEND_PID_FILE")"
    if pid_is_running "$pid"; then
        log "Frontend déjà en cours (PID $pid)"
        return 0
    fi

    if port_is_open "$FRONTEND_ADDR"; then
        err "le port $FRONTEND_ADDR est déjà utilisé"
        return 1
    fi

    ensure_frontend_deps

    : >"$FRONTEND_LOG"
    load_node
    (
        cd "$ROOT_DIR/frontend"
        if [[ "$HAS_SETSID" == 1 ]]; then
            exec setsid npm run dev -- --host 127.0.0.1 --port 5173 --strictPort
        else
            exec npm run dev -- --host 127.0.0.1 --port 5173 --strictPort
        fi
    ) >>"$FRONTEND_LOG" 2>&1 &
    echo $! >"$FRONTEND_PID_FILE"

    wait_for_port "$FRONTEND_ADDR" "Frontend" 60 || return 1
    log "Frontend démarré (PID $(read_pid "$FRONTEND_PID_FILE"), http://$FRONTEND_ADDR)"
}

stop_process() {
    local pid_file="$1"
    local label="$2"
    local pid

    pid="$(read_pid "$pid_file")"
    if ! pid_is_running "$pid"; then
        rm -f "$pid_file"
        log "$label déjà arrêté"
        return 0
    fi

    log "Arrêt de $label (PID $pid)..."
    if [[ "$HAS_SETSID" == 1 ]]; then
        kill -TERM -- "-$pid" 2>/dev/null || kill -TERM "$pid" 2>/dev/null || true
    else
        kill -TERM "$pid" 2>/dev/null || true
    fi

    for _ in {1..10}; do
        if ! pid_is_running "$pid"; then
            rm -f "$pid_file"
            log "$label arrêté"
            return 0
        fi
        sleep 1
    done

    if [[ "$HAS_SETSID" == 1 ]]; then
        kill -KILL -- "-$pid" 2>/dev/null || kill -KILL "$pid" 2>/dev/null || true
    else
        kill -KILL "$pid" 2>/dev/null || true
    fi
    rm -f "$pid_file"
    log "$label arrêté (SIGKILL)"
}

stop_frontend() {
    stop_process "$FRONTEND_PID_FILE" "Frontend"
    if port_is_open "$FRONTEND_ADDR"; then
        log "Libération du port ${FRONTEND_ADDR##*:}..."
        if command -v fuser >/dev/null 2>&1; then
            fuser -k "${FRONTEND_ADDR##*:}/tcp" 2>/dev/null || true
        fi
        sleep 1
    fi
}

cmd_start() {
    ensure_dirs
    build_rust_bins
    sync_armies
    start_server
    if ! start_frontend; then
        stop_process "$SERVER_PID_FILE" "API"
        stop_frontend
        exit 1
    fi
    log "Tout est prêt. Logs : $LOG_DIR"
}

cmd_stop() {
    stop_frontend
    stop_process "$SERVER_PID_FILE" "API"
}

cmd_restart() {
    cmd_stop
    cmd_start
}

cmd_status() {
    local server_pid frontend_pid

    server_pid="$(read_pid "$SERVER_PID_FILE")"
    frontend_pid="$(read_pid "$FRONTEND_PID_FILE")"

    if pid_is_running "$server_pid" || port_is_open "$SERVER_ADDR"; then
        if pid_is_running "$server_pid"; then
            printf 'API       : en cours (PID %s, http://%s)\n' "$server_pid" "$SERVER_ADDR"
        else
            printf 'API       : port %s ouvert (PID inconnu)\n' "$SERVER_ADDR"
        fi
    else
        printf 'API       : arrêtée\n'
    fi

    if pid_is_running "$frontend_pid" || port_is_open "$FRONTEND_ADDR"; then
        if pid_is_running "$frontend_pid"; then
            printf 'Frontend  : en cours (PID %s, http://%s)\n' "$frontend_pid" "$FRONTEND_ADDR"
        else
            printf 'Frontend  : port %s ouvert (PID inconnu)\n' "$FRONTEND_ADDR"
        fi
    else
        printf 'Frontend  : arrêté\n'
    fi

    if [[ -d "$LOG_DIR" ]]; then
        printf 'Logs      : %s\n' "$LOG_DIR"
    fi
}

cmd_import_coupe() {
    local coupe="${1:?numéro de coupe requis (5-10)}"
    shift
    cargo build --bin poissonnerie-import-coupe --quiet
    "$IMPORT_BIN" --coupe "$coupe" "$@"
}

cmd_import_coupes() {
    cargo build --bin poissonnerie-import-coupe --quiet
    local c
    for c in 5 6 7 8 9 10; do
        log "Import coupe $c…"
        "$IMPORT_BIN" --coupe "$c" "$@"
    done
}

main() {
    local command="${1:-}"

    case "$command" in
        start)
            cmd_start
            ;;
        stop)
            cmd_stop
            ;;
        restart)
            cmd_restart
            ;;
        status)
            cmd_status
            ;;
        import-coupe)
            shift
            cmd_import_coupe "$@"
            ;;
        import-coupes)
            shift
            cmd_import_coupes "$@"
            ;;
        -h | --help | help | '')
            usage
            [[ -z "$command" ]] && exit 1
            ;;
        *)
            err "commande inconnue : $command"
            usage
            exit 1
            ;;
    esac
}

main "$@"
