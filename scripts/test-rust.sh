#!/usr/bin/env bash
set -euo pipefail

SKIP_CLIPPY=0
if [[ "${1:-}" == "--skip-clippy" ]]; then
  SKIP_CLIPPY=1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo was not found on PATH. Install Rust with rustup first." >&2
  exit 1
fi

cargo fmt --check

if [[ "${SKIP_CLIPPY}" -eq 0 ]]; then
  cargo clippy --workspace --all-targets -- -D warnings
fi

cargo test --workspace
cargo build -p authserver
cargo build -p auth-flow-test
