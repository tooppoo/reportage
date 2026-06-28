#!/usr/bin/env sh
set -eu

cd "$(dirname "$0")/../.."
echo $(pwd)

if ! command -v code >/dev/null 2>&1; then
  echo "error: code command is not available in this environment" >&2
  exit 1
fi

cd editors/vscode
pnpm i --frozen-lockfile
pnpm run package
code --install-extension ./reportage-vscode.vsix --force
