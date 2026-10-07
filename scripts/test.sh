#!/usr/bin/env bash
# Builds v0 copies of both programs into target/litesvm (Anchor's default SBPF v3
# output is not loadable by LiteSVM 0.11), then runs the lifecycle tests.
# Extra arguments are passed to the test binary, e.g. ./scripts/test.sh issue_bond
set -euo pipefail
cd "$(dirname "$0")/.."
FLAGS="--arch v0 --tools-version v1.57 --skip-tools-install --sbf-out-dir target/litesvm"
cargo build-sbf $FLAGS --manifest-path programs/bond-core/Cargo.toml
cargo build-sbf $FLAGS --manifest-path programs/corporate-actions/Cargo.toml
cargo test -p corporate-actions --test lifecycle -- --nocapture "$@"
