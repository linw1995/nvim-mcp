export RUSTFLAGS="-Cinstrument-coverage"
export CARGO_TARGET_DIR="./target/coverage"
export LLVM_PROFILE_FILE="${CARGO_TARGET_DIR}/data/nvim-mcp-%p-%m.profraw"

cargo build --bin nvim-mcp
cargo test "$@"

echo "Generating code coverage report..."

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
