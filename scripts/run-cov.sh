#!/bin/bash
set -euxo pipefail

export RUST_LOG=${RUST_LOG-debug}

export CARGO_INCREMENTAL=0
export RUSTFLAGS="-Cinstrument-coverage -Ccodegen-units=1 -Copt-level=0 -Clink-dead-code"
# Nightly
# export RUSTFLAGS="-Cinstrument-coverage -Ccodegen-units=1 -Copt-level=0 -Clink-dead-code -Zpanic_abort_tests -Cpanic=abort"
# export RUSTDOCFLAGS="-Cpanic=abort"
export CARGO_TARGET_DIR="./target/coverage"
export LLVM_PROFILE_FILE="${CARGO_TARGET_DIR}/data/nvim-mcp-%p-%m.profraw"

rm -rf ${CARGO_TARGET_DIR}/data/
mkdir -p ${CARGO_TARGET_DIR}/data/

cargo clean --package nvim-mcp # Make sure to clean previous builds (Force build.rs to be reruned)
cargo build --bin nvim-mcp
cargo test "$@"

echo "Generating code coverage report..."

rm -rf ${CARGO_TARGET_DIR}/result/
mkdir -p ${CARGO_TARGET_DIR}/result/

grcov ${CARGO_TARGET_DIR}/data \
	--llvm \
	--branch \
	--source-dir . \
	--ignore-not-existing \
	--ignore '../*' --ignore "/*" \
	--binary-path ${CARGO_TARGET_DIR}/debug/ \
	--output-types html,cobertura,markdown \
	--output-path ${CARGO_TARGET_DIR}/result/
tail -n 1 ${CARGO_TARGET_DIR}/result/markdown.md
