#!/usr/bin/env sh

path=${1%/}

if [ ! -e "$path" ]; then
  echo "Error: $path does not exist"
  exit 1
fi

echo "Success: $path exists"
exit 0
