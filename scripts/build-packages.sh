#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

version="${1:-$(jq -r '.version' tauri.conf.json)}"

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
  --bundles deb,rpm \
  --config "$config_override"

shopt -s nullglob
deb_packages=(target/release/bundle/deb/*.deb)
rpm_packages=(target/release/bundle/rpm/*.rpm)

if (( ${#deb_packages[@]} == 0 || ${#rpm_packages[@]} == 0 )); then
  printf 'Tauri did not produce both Debian and Fedora packages\n' >&2
  exit 1
fi

printf 'Debian package: %s\n' "${deb_packages[@]}"
printf 'Fedora package: %s\n' "${rpm_packages[@]}"
