#!/usr/bin/env bash

set -euo pipefail

APP_ID="com.mcell.youclaw"

IS_DRY_RUN=false

print_usage() {
  printf 'Usage: %s [--dry-run]\n' "${0##*/}"
  printf '\n'
  printf 'Reset current-platform YouClaw local state.\n'
  printf 'This removes app data and cache, including app_v2.sqlite, profiles, memory, and internal AGENTS workspace.\n'
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

case "$(uname -s)" in
  Darwin)
    TARGET_DIRS=(
      "$HOME/Library/Application Support/${APP_ID}"
      "$HOME/Library/Caches/${APP_ID}"
    )
    PLATFORM_LABEL="macOS"
    ;;
  Linux)
    XDG_DATA_HOME_VALUE="${XDG_DATA_HOME:-$HOME/.local/share}"
    XDG_CACHE_HOME_VALUE="${XDG_CACHE_HOME:-$HOME/.cache}"
    TARGET_DIRS=(
      "$XDG_DATA_HOME_VALUE/${APP_ID}"
      "$XDG_CACHE_HOME_VALUE/${APP_ID}"
    )
    PLATFORM_LABEL="Linux"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    WINDOWS_ROAMING_DIR="${APPDATA:-$HOME/AppData/Roaming}"
    WINDOWS_LOCAL_DIR="${LOCALAPPDATA:-$HOME/AppData/Local}"
    TARGET_DIRS=(
      "$WINDOWS_ROAMING_DIR/${APP_ID}"
      "$WINDOWS_LOCAL_DIR/${APP_ID}"
    )
    PLATFORM_LABEL="Windows"
    ;;
  *)
    printf "Unsupported platform: %s\n" "$(uname -s)" >&2
    exit 1
    ;;
esac

printf "YouClaw local state reset for %s%s\n" \
  "$PLATFORM_LABEL" \
  "$([[ "$IS_DRY_RUN" == "true" ]] && printf " (dry-run)")"
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
  printf "reset completed, removed %d directories.\n" "$REMOVED_COUNT"
fi
