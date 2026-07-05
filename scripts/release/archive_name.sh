#!/usr/bin/env sh

set -eu

version=${1%/}

script_path="$(dirname "$(realpath "$0")")"

arch="$(uname -m)"
os="$(uname -s)"

echo "reportage_${version}_${os}_${arch}"
