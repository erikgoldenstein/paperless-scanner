#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

version="${1:-$(jq -r '.version' tauri.conf.json)}"
bundles="${2:-deb,rpm}"

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.-]+)?$ ]]; then
  printf 'invalid package version: %s\n' "$version" >&2
  exit 1
fi

if ! command -v cargo >/dev/null || ! command -v jq >/dev/null; then
  printf 'cargo and jq are required to build release packages\n' >&2
  exit 1
fi

config_override="$(jq -cn --arg version "$version" '{version: $version}')"

cargo tauri build \
  --ci \
  --features gui \
  --bundles "$bundles" \
  --config "$config_override"

shopt -s nullglob
IFS=',' read -r -a bundle_targets <<< "$bundles"
for bundle in "${bundle_targets[@]}"; do
  case "$bundle" in
    deb) packages=(target/release/bundle/deb/*.deb) ;;
    rpm) packages=(target/release/bundle/rpm/*.rpm) ;;
    appimage) packages=(target/release/bundle/appimage/*.AppImage) ;;
    *) printf 'unsupported Linux bundle: %s\n' "$bundle" >&2; exit 1 ;;
  esac

  if (( ${#packages[@]} == 0 )); then
    printf 'Tauri did not produce a %s package\n' "$bundle" >&2
    exit 1
  fi
  printf '%s package: %s\n' "$bundle" "${packages[@]}"
done
