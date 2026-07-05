#!/usr/bin/env sh

set -eu

dist=${1%/}

if [ ! -d "$dist" ]; then
  mkdir -p "$dist"
fi
