
# generate each public spec schema.json from its schema.internal.json
[group('schema')]
schema-artifacts-gen:
  cargo run --locked -p xtask -- schema-artifacts gen

# check every generated public spec schema.json is in sync with its schema.internal.json
[group('schema')]
[group('check')]
schema-artifacts-check:
  cargo run --locked -p xtask -- schema-artifacts check
