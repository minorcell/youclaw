#!/usr/bin/env bash

set -euo pipefail

APP_ID="com.mcell.youclaw"
APP_NAME="YouClaw"
APP_SLUG="youclaw"
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

IS_DRY_RUN=false

print_usage() {
  printf 'Usage: %s [--dry-run]\n' "${0##*/}"
}

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --dry-run)
      IS_DRY_RUN=true
      ;;
    --help|-h)
      print_usage
      exit 0
      ;;
    --)
      ;;
    *)
      print_usage >&2
      exit 1
      ;;
  esac
  shift
done

if [[ -z "${HOME:-}" ]]; then
  printf "HOME is not set, cannot resolve cache directories.\n" >&2
  exit 1
fi

XDG_DATA_HOME_VALUE="${XDG_DATA_HOME:-$HOME/.local/share}"
XDG_CACHE_HOME_VALUE="${XDG_CACHE_HOME:-$HOME/.cache}"
WINDOWS_ROAMING_DIR="${APPDATA:-$HOME/AppData/Roaming}"
WINDOWS_LOCAL_DIR="${LOCALAPPDATA:-$HOME/AppData/Local}"

TARGET_DIRS=(
  "$REPO_ROOT/.${APP_SLUG}-data"
  "$HOME/Library/Application Support/${APP_ID}"
  "$HOME/Library/Caches/${APP_ID}"
  "$HOME/Library/Application Support/${APP_NAME}"
  "$HOME/Library/Caches/${APP_NAME}"
  "$XDG_DATA_HOME_VALUE/${APP_ID}"
  "$XDG_CACHE_HOME_VALUE/${APP_ID}"
  "$XDG_DATA_HOME_VALUE/${APP_NAME}"
  "$XDG_CACHE_HOME_VALUE/${APP_NAME}"
  "$WINDOWS_ROAMING_DIR/${APP_ID}"
  "$WINDOWS_LOCAL_DIR/${APP_ID}"
  "$WINDOWS_ROAMING_DIR/${APP_NAME}"
  "$WINDOWS_LOCAL_DIR/${APP_NAME}"
)

printf "YouClaw cache cleanup%s\n" "$([[ "$IS_DRY_RUN" == "true" ]] && printf " (dry-run)")"
printf "Please close running YouClaw instances before cleanup.\n"

REMOVED_COUNT=0

for target_dir in "${TARGET_DIRS[@]}"; do
  if [[ ! -d "$target_dir" ]]; then
    printf "skip: %s\n" "$target_dir"
    continue
  fi

  if [[ "$IS_DRY_RUN" == "true" ]]; then
    printf "would remove: %s\n" "$target_dir"
    continue
  fi

  rm -rf "$target_dir"
  printf "removed: %s\n" "$target_dir"
  REMOVED_COUNT=$((REMOVED_COUNT + 1))
done

if [[ "$IS_DRY_RUN" == "true" ]]; then
  printf "dry-run completed.\n"
else
  printf "cleanup completed, removed %d directories.\n" "$REMOVED_COUNT"
fi
