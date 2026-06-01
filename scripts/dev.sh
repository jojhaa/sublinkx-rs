#!/usr/bin/env sh
set -eu

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
BACKEND_DIR="$ROOT_DIR/backend"
FRONTEND_DIR="$ROOT_DIR/frontend"

DATABASE_URL="${DATABASE_URL:-sqlite://data/app.db}"
BACKEND_PORT="${BACKEND_PORT:-8080}"
FRONTEND_PORT="${FRONTEND_PORT:-5173}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing command '$1'. Please install it first and make sure it is in PATH." >&2
    exit 1
  fi
}

need_cmd cargo
need_cmd npm

mkdir -p "$BACKEND_DIR/data"

if [ ! -d "$FRONTEND_DIR/node_modules" ]; then
  echo "Installing frontend dependencies..."
  (cd "$FRONTEND_DIR" && npm install)
fi

cleanup() {
  echo ""
  echo "Stopping local services..."
  if [ "${BACKEND_PID:-}" ]; then kill "$BACKEND_PID" 2>/dev/null || true; fi
  if [ "${FRONTEND_PID:-}" ]; then kill "$FRONTEND_PID" 2>/dev/null || true; fi
  wait 2>/dev/null || true
}
trap cleanup INT TERM EXIT

echo ""
echo "Starting sublinkx-rs locally..."
echo "  Backend : http://127.0.0.1:$BACKEND_PORT"
echo "  Frontend: http://127.0.0.1:$FRONTEND_PORT"
echo "  Database: $DATABASE_URL"
echo ""
echo "Press Ctrl+C to stop both services."
echo ""

(
  cd "$BACKEND_DIR"
  APP_ENV=development \
  APP_PORT="$BACKEND_PORT" \
  DATABASE_URL="$DATABASE_URL" \
  SUBLINKX_RUNTIME_MODE=local \
  JWT_SECRET=local-dev-secret-change-before-production \
  BOOTSTRAP_ADMIN_USERNAME=admin \
  BOOTSTRAP_ADMIN_PASSWORD=admin123456 \
  cargo run
) &
BACKEND_PID="$!"

(
  cd "$FRONTEND_DIR"
  VITE_API_BASE_URL="http://127.0.0.1:$BACKEND_PORT" \
  npm run dev -- --host 0.0.0.0 --port "$FRONTEND_PORT"
) &
FRONTEND_PID="$!"

wait
