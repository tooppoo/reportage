#!/usr/bin/env sh

set -euo pipefail

script_path="$(dirname "$(realpath "$0")")"

arch="$(uname -m)"
os="$(uname -s)"
version="$($script_path/get-version.sh)"

echo "reportage_${version}_${os}_${arch}"
