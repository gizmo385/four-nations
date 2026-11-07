#!/usr/bin/env bash
# A thin wrapper around `cargo xtask agent-run` (#507): kept so existing commands and docs that
# invoke `scripts/agent-run.sh` keep working. See `cargo xtask agent-run --help` (or
# `xtask/src/agent_run.rs`) for every option; `scripts/agent-run.sh watch [RUN_DIR]` still
# watches (or replays) a run.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec cargo run --quiet --manifest-path "$repo/Cargo.toml" --package xtask -- agent-run "$@"
