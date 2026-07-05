#!/usr/bin/env sh

set -eu

script_path="$(dirname "$(realpath "$0")")"
version="$($script_path/../../get-version.sh)"

echo "checksum.reportage_${version}_Linux_x86_64.txt"
