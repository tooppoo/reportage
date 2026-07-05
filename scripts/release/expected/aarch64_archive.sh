#!/usr/bin/env sh

set -eu

script_path="$(dirname "$(realpath "$0")")"
version="$($script_path/../../get-version.sh)"

echo "$($script_path/../template/archive.sh $version Linux aarch64)"
