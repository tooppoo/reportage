#!/usr/bin/env bash

set -euo pipefail

arch="$(uname -m)"
os="$(uname -s)"
version="$(toml get Cargo.toml workspace.package.version --raw)"

echo "reportage_v${version}_${os}_${arch}"
