#!/usr/bin/env sh

set -eu

script_path="$(dirname "$(realpath "$0")")"
version=${1%/}
dist=${2%/}

main() {
  cleanup

  build_reportage

  archive_reportage
}

build_reportage() {
  cargo build --release --locked --quiet --package reportage-cli
}

archive_reportage() {
  archive_dir="$dist/$($script_path/archive_name.sh $version)"
  archive_path="$archive_dir.tar.gz"

  mkdir -p "$archive_dir"
  cp target/release/reportage "$archive_dir/reportage"

  tar -acf "$archive_path" "$archive_dir"

  checksum_path="$dist/checksums_$(basename "$archive_dir").txt"
  sha256sum "$archive_path" > "$checksum_path"

  rm -r "$archive_dir"

  echo "$archive_path"
  echo "$checksum_path"
}

cleanup() {
  rm -rf "$dist" 2>&1 > /dev/null || true
}

main
