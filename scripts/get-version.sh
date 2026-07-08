#!/usr/bin/env sh
set -eu

script_path="$(dirname "$(realpath "$0")")"

v="$(toml get $script_path/../Cargo.toml workspace.package.version --raw)"

echo "$v"
