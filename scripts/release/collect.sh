#!/usr/bin/env sh

set -eu

script_path="$(dirname "$(realpath "$0")")"

src=${1%/}
dist=${2%/}

main() {
  cp_archive "$($script_path/expected/x86_64_archive.sh).tar.gz"
  cp_archive "$($script_path/expected/aarch64_archive.sh).tar.gz"

  cat_checksums > "$dist"/checksums.txt
}

cp_archive() {
  cp "$src"/"$1" "$dist"/"$1"
}
cat_checksums() {
  cat "$src"/"$($script_path/expected/x86_64_checksum.sh)" \
      "$src"/"$($script_path/expected/aarch64_checksum.sh)"
}

main
