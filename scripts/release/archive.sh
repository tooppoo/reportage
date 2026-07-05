#!/usr/bin/env bash

set -euo pipefail

script_path="$(dirname "$(realpath "$0")")"
root="$(realpath "$script_path/../..")"
dist="$root/dist"

function main() {
  cleanup

  build_reportage

  package="$($script_path/package-name.sh)"
  archive="$($script_path/archive-name.sh)"

  mkdir -p "$dist/${package}"
  cp target/release/reportage "$dist/${package}/reportage"

  package_reportage "$package" "$archive"
}

function build_reportage() {
  cargo build --release --locked --quiet --package reportage-cli
}

functionn package_reportage() {
  package=${1%/}
  archive=${2%/}

  tar -C "$dist" -czf "$dist/${archive}" "${package}"
  echo "OUT: $dist/${archive}"

  sha256sum "$dist/$archive" > "$dist/checksums.txt"

  echo "OUT: $dist/$archive"
}

function cleanup() {
  rm -rf "$root/dist" 2>&1 > /dev/null || true
}

main
