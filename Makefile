RUN_MIRI = MIRIFLAGS=-Zmiri-backtrace=full cargo +nightly miri test


pre-commit: format check test build

tools:
	#rustup toolchain install nightly
	#rustup component add miri --toolchain nightly
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

base-test:
	cargo test --release

loom:
	RUST_BACKTRACE=full cargo test --features sanitizers --release

miri-spinlock:
	$(RUN_MIRI) spinlock

miri-spscringbuffer:  # v1 and v2
	$(RUN_MIRI) spscringbuffer

miri-lazy:
	$(RUN_MIRI) lazy

miri:
	$(RUN_MIRI)

test: base-test loom miri
