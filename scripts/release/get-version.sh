#!/usr/bin/env sh

set -euo pipefail

v="$(toml get Cargo.toml workspace.package.version --raw)"

echo "v$v"
