#!/usr/bin/env sh

set -euo

script_path="$(dirname "$(realpath "$0")")"

echo "$($script_path/archive_name.sh).tar.gz"
