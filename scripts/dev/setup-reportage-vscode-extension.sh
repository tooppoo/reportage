#!/usr/bin/env sh
set -eu

cd "$(dirname "$0")/../.."
echo $(pwd)

# `code --install-extension` uses the VS Code remote-cli, which talks to an
# attached VS Code window through VSCODE_IPC_HOOK_CLI. On a container rebuild
# this script runs during postCreateCommand before any window is attached, so
# the socket is unset and the CLI exits non-zero with "code ... is not installed".
# Skip instead of failing so the rebuild succeeds; running this from the
# integrated terminal (where the socket is set) still installs the extension.
if ! command -v code >/dev/null 2>&1; then
  echo "skip: code command not available in this environment" >&2
  exit 0
fi
if [ -z "${VSCODE_IPC_HOOK_CLI:-}" ]; then
  echo "skip: no attached VS Code window; run this from the integrated terminal to install the extension" >&2
  exit 0
fi

cd editors/vscode
pnpm i --frozen-lockfile
pnpm run package
code --install-extension ./reportage-vscode.vsix --force
