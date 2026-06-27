#!/usr/bin/sh

cargo install cargo-binstall@1.20.1 --locked
cargo binstall cargo-llvm-cov@0.8.7 \
  just@1.54.0 \
  cargo-nextest@0.9.138 \
  --locked
