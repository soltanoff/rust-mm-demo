pre-commit: format check test build

format:
	cargo fmt --
	cargo clippy --

check:
	cargo check

build:
	cargo build --release

run:
	cargo run --release

test:
	cargo test --release
	RUST_BACKTRACE=full cargo test --features sanitizers --release
