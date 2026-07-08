#!/usr/bin/env sh

set -eu

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
  archive_name="$($script_path/archive_name.sh)"
  archive_dir="$dist/$archive_name"
  archive_path="$archive_dir.tar.gz"

  mkdir -p "$archive_dir"
  cp target/release/reportage "$archive_dir/reportage"

  # archive from within "$archive_dir" so the archive contains "reportage"
  # at its root (matching install.sh's binary.pathInArchive), instead of
  # leaking the staging directory name into the packaged paths (same
  # reasoning as the checksum step below).
  tar -acf "$archive_path" -C "$archive_dir" reportage

  checksum_path="$($script_path/checksum_name.sh)"
  (
    # if directory is not changed, "$dist/*.tar.gz" will be written to checksum file.
    # checksum file and archives is in same directory finally,
    # so "$dist" should not be included in any file paths.
    #
    # e.g.
    # {checksum} dist/reportage_0.0.1_Linux_x86_64.tar.gz
    #
    # expected:
    # {checksum} reportage_0.0.1_Linux_x86_64.tar.gz
    cd "$dist"
    sha256sum "$archive_name.tar.gz" > "$checksum_path"
  )

  rm -r "$archive_dir"

  echo "$archive_path"
  echo "dist/$checksum_path"
}

cleanup() {
  rm -rf "$dist" 2>&1 > /dev/null || true
}

main
