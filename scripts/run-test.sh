#!/bin/bash
set -euxo pipefail

export RUST_LOG=${RUST_LOG-debug}

cargo build --bin nvim-mcp
cargo test "$@"
