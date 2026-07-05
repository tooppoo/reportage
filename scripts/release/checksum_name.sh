#!/usr/bin/env sh

set -eu

script_path="$(dirname "$(realpath "$0")")"

version="$($script_path/../get-version.sh)"
os="$(uname -s)"
arch="$(uname -m)"

echo "$($script_path/template/checksum.sh $version $os $arch)"
