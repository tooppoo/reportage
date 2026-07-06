#!/usr/bin/env sh

set -eu

version=${1%/}
os=${2%/}
arch=${3%/}

echo "checksum.reportage_${version}_${os}_${arch}.txt"
