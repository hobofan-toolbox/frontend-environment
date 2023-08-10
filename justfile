default:
  just --list

build_features:
  cargo build
  cargo build --no-default-features