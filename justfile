default:
  just --list

build_features:
  cargo build --no-default-features
  cargo build