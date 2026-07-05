#!/usr/bin/env sh

set -euo pipefail

script_path="$(dirname "$(realpath "$0")")"
dist=${1%/}

main() {
  cleanup

  build_reportage

  archive_reportage
}

build_reportage() {
  cargo build --release --locked --quiet --package reportage-cli
}

archive_reportage() {
  archive_dir="$dist/$($script_path/archive_name.sh)"
  archive_path="$dist/$($script_path/archive_path.sh)"

  mkdir -p "$archive_dir"
  cp target/release/reportage "$archive_dir/reportage"

  tar -acf "$archive_path" "$archive_dir"

  checksum_path="$dist/checksums_$(basename "$archive_dir").txt"
  sha256sum "$archive_path" > "$checksum_path"

  echo "$archive_path"
  echo "$checksum_path"
}

cleanup() {
  rm -rf "$dist" 2>&1 > /dev/null || true
}

main
