#!/usr/bin/env sh

path=${1%/}

if [ ! -f "$path" ]; then
  echo "Error: $path is not a file"
  exit 1
fi

echo "Success: $path is a file"
exit 0
