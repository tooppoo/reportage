#!/usr/bin/env sh

path=${1%/}

if [ ! -x "$path" ]; then
  echo "Error: $path is not executable"
  exit 1
fi

echo "Success: $path is executable"

exit 0
