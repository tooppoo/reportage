#!/usr/bin/env sh

set -eu

script_path="$(dirname "$(realpath "$0")")"
version="$($script_path/../../get-version.sh)"

echo "reportage_${version}_Linux_aarch64"
