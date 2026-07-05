#!/usr/bin/env sh

set -eu

script_path="$(dirname "$(realpath "$0")")"

src=${1%/}
dist=${2%/}

archive_name="$src"/"$($script_path/archive_name.sh)"

tar -xf "$archive_name".tar.gz

"$script_path/../utils/ensure_dir.sh" "$dist"
mv "$archive_name"/reportage "$dist"/reportage
rm -r "$archive_name"
