#!/usr/bin/env sh

path=${1%/}

if ! tar -tf "$path" > /dev/null 2>&1; then
  echo "ERROR: path is not a valid tarball: $path" >&2
  exit 1
fi

echo "Success: $path is a valid tarball"
exit 0
