#!/usr/bin/env bash

set -euo pipefail

script_path="$(dirname "$(realpath "$0")")"

echo "$(sh $script_path/package-name.sh).tar.gz"
