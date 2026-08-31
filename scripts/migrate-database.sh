#!/usr/bin/env bash
set -Eeuo pipefail

mode="${1:-check}"
confirmation="${2:-}"

if [[ "$mode" != "check" && "$mode" != "run" ]]; then
  echo "Usage: $0 check | run --confirm-backup" >&2
  exit 2
fi

: "${SUBLINKX_MIGRATION_SOURCE_URL:?Set SUBLINKX_MIGRATION_SOURCE_URL first}"
: "${SUBLINKX_MIGRATION_TARGET_URL:?Set SUBLINKX_MIGRATION_TARGET_URL first}"

if [[ "$mode" == "run" ]]; then
  if [[ "$confirmation" != "--confirm-backup" ]]; then
    echo "Run mode requires --confirm-backup after creating and verifying a current backup." >&2
    exit 2
  fi
  export SUBLINKX_MIGRATION_CONFIRM='I_HAVE_A_CURRENT_BACKUP'
fi

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root/backend"
cargo run --locked -- migrate-database "$mode"
