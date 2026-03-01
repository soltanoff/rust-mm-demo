pre-commit: format check test build

tools:
	rustup +nightly component add miri

format:
	cargo fmt --
	cargo clippy -- -D warnings

check:
	cargo check

build:
	cargo build --release

run:
	cargo run --release

test:
	cargo test --release
	RUST_BACKTRACE=full cargo test --features sanitizers --release
	MIRIFLAGS=-Zmiri-backtrace=full cargo +nightly miri test
