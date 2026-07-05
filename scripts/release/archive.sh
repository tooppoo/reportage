#!/usr/bin/env bash

set -euo pipefail

rm -rf dist 2>&1 > /dev/null || true

arch="$(uname -m)"
os="$(uname -s)"
version="$(toml get Cargo.toml workspace.package.version --raw)"

package="reportage_v${version}_${os}_${arch}"
archive="${package}.tar.gz"

mkdir -p "dist/${package}"
cp target/release/reportage "dist/${package}/reportage"

tar -C dist -czf "dist/${archive}" "${package}"
(
  cd dist
  sha256sum "${archive}" > "checksums.txt"
)
