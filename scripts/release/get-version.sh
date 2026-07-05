#!/usr/bin/env sh

set -eu

v="$(toml get Cargo.toml workspace.package.version --raw)"

echo "v$v"
