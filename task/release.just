
# update the version in Cargo.toml to the given version
[group('release')]
update-version version:
  rellog ready {{ version }}

  toml set ./Cargo.toml workspace.package.version {{ version }} > Cargo.toml.tmp
  mv Cargo.toml.tmp Cargo.toml

# create release tag(version pin) for the given version
[group('release')]
release-tag version:
  rellog ready {{ version }}
  just assert-version {{ version }}

  git tag -a {{ version }} -m "Release {{ version }}"

# check if the given version matches the version in Cargo.toml
[group('release')]
assert-version version:
  #!/usr/bin/env sh
  set -eu
  expected_version={{ version }}
  actual_version=$(sh scripts/get-version.sh)
  if [ "$expected_version" != "$actual_version" ]; then
    echo "Version mismatch: expected $expected_version, but got $actual_version"
    exit 1
  fi

# [dev] delete release tag for the given version
[private]
release-delete version:
  git tag -d {{ version }} || true
  git tag -d latest || true
