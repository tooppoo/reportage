#!/usr/bin/env sh

path=${1%/}
expected_line_count=${2%/}

actual="$(cat "$path" | wc -l)"

if [ "$actual" -ne "$expected_line_count" ]; then
  echo "Expected $expected_line_count lines, but got $actual lines in $path"
  exit 1
fi

echo "Line count assertion passed: $actual lines in $path"
exit 0
