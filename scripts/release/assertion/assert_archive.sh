#!/usr/bin/env sh

set -eu

script_path="$(dirname "$(realpath "$0")")"
version="$($script_path/../../get-version.sh)"

dist_path=${1%/}

main() {
  assert_archive_x86_64
  assert_archive_aarch64
}

assert_archive_x86_64() {
  assert_archive "$dist_path/reportage_${version}_Linux_x86_64"
}
assert_archive_aarch64() {
  assert_archive "$dist_path/reportage_${version}_Linux_aarch64"
}
assert_archive() {
  archive_name=${1%/}

  archive_path="$archive_name.tar.gz"

  "$script_path/assert_exists.sh" "$archive_path"
  "$script_path/assert_is_file.sh" "$archive_path"
  "$script_path/assert_is_tarball.sh" "$archive_path"

  unpack "$archive_path"

  binary_path="$archive_name/reportage"

  "$script_path/assert_exists.sh" "$binary_path"
  "$script_path/assert_is_file.sh" "$binary_path"
  "$script_path/assert_is_executable.sh" "$binary_path"

  unpack_cleanup
}
unpack() {
  archive_path=${1%/}

  tar -xvf "$archive_path" \
    # reverse to ensure that files are removed before directories
    | sort -r \
    > "$dist_path/unpacked.tmp"
}
unpack_cleanup() {
  cat "$dist_path/unpacked.tmp" \
    | grep "$archive_name" \
    # use -r to remove unpacked file & dir transparently
    | xargs rm -r
  rm "$dist_path/unpacked.tmp"
}

main
