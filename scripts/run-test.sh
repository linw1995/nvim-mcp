#!/bin/bash
set -euxo pipefail

cargo build --bin nvim-mcp
cargo test "$@"
