
#!/usr/bin/env bash

set -eu

usage() {
  cat <<'EOF'
Usage:
  find-bare-markdown-paths.sh [-n] [directory]

Options:
  -n  Print matching line details as file:line:path.
      Without -n, print each matching Markdown file once.

Detects bare repository-style file paths in Markdown prose, such as:

  docs/example.md
  ./docs/example.md
  ../docs/example.md
  src/main.rs

The script ignores:

  - fenced code blocks
  - inline code spans
  - Markdown inline link destinations, such as [label](docs/example.md)

This is a heuristic detector, not a full Markdown parser.
EOF
}

show_lines=0

while getopts 'nh' opt; do
  case "$opt" in
    n)
      show_lines=1
      ;;
    h)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

shift "$((OPTIND - 1))"

case "$#" in
  0)
    root=.
    ;;
  1)
    root=$1
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

if [ ! -d "$root" ]; then
  printf 'error: not a directory: %s\n' "$root" >&2
  exit 1
fi

repo_root=$(git -C "$root" rev-parse --show-toplevel 2>/dev/null) || {
  printf 'error: not inside a git repository: %s\n' "$root" >&2
  exit 1
}

root_abs=$(cd "$root" && pwd -P)
repo_abs=$(cd "$repo_root" && pwd -P)

if [ "$root_abs" = "$repo_abs" ]; then
  root_pathspec=.
else
  case "$root_abs/" in
    "$repo_abs/"*)
      root_pathspec=${root_abs#"$repo_abs"/}
      ;;
    *)
      printf 'error: directory is outside the git repository: %s\n' "$root" >&2
      exit 1
      ;;
  esac
fi

git -C "$repo_root" ls-files -- "$root_pathspec" |
while IFS= read -r file; do
  case "$file" in
    *.md|*.markdown)
      awk -v show_lines="$show_lines" -v display_file="$file" '
BEGIN {
  path_re = "^(\\./|\\.\\./|[[:alnum:]_.-]+/)[[:alnum:]_.@%+-]+(/[[:alnum:]_.@%+-]+)*\\.(md|markdown|adoc|rst|txt|json|toml|ya?ml|kdl|rs|go|sh|ts|tsx|js|jsx|css|scss|html|svg|png|jpg|jpeg|gif|webp|pdf)$"
}

FNR == 1 {
  in_fence = 0
  reported = 0
}

function strip_markdown_inline_links(s, before, after) {
  # Remove inline Markdown link/image destinations:
  #   [README](../README.md)
  #   ![alt](docs/image.png)
  #
  # This intentionally removes only the destination part.
  # The label text remains as prose, but linked file paths are not scanned.
  while (match(s, /!?\[[^][]*\]\([^)]*\)/)) {
    before = substr(s, 1, RSTART - 1)
    after = substr(s, RSTART + RLENGTH)
    s = before " " after
  }

  return s
}

function strip_reference_link_definition(s) {
  # Remove reference-style link definitions:
  #   [README]: ../README.md
  #   [README]: ../README.md "README"
  if (s ~ /^[[:space:]]*\[[^]]+\]:[[:space:]]+/) {
    return ""
  }

  return s
}

function strip_inline_code(s, before, after) {
  while (match(s, /`[^`]*`/)) {
    before = substr(s, 1, RSTART - 1)
    after = substr(s, RSTART + RLENGTH)
    s = before " " after
  }

  return s
}

function strip_inline_link_destinations(s, before, after) {
  while (match(s, /\]\([^)]*\)/)) {
    before = substr(s, 1, RSTART - 1)
    after = substr(s, RSTART + RLENGTH)
    s = before " " after
  }

  return s
}

function trim_candidate(s) {
  gsub(/^[<({\["]+/, "", s)
  gsub(/[>)}\]".,;:!?]+$/, "", s)
  return s
}

function report_match(path) {
  if (show_lines == "1") {
    printf "%s:%d:%s\n", display_file, FNR, path
    return
  }

  if (!reported) {
    print display_file
    reported = 1
  }
}

function scan_line(line, fields, n, i, candidate) {
  line = strip_inline_code(line)
  line = strip_markdown_inline_links(line)
  line = strip_reference_link_definition(line)

  n = split(line, fields, /[[:space:]]+/)

  for (i = 1; i <= n; i++) {
    candidate = trim_candidate(fields[i])

    if (candidate ~ path_re) {
      report_match(candidate)

      if (show_lines != "1") {
        return
      }
    }
  }
}

/^[[:space:]]*```/ || /^[[:space:]]*~~~/ {
  in_fence = !in_fence
  next
}

in_fence {
  next
}

{
  scan_line($0)
}
' "$repo_root/$file"
      ;;
  esac
done
