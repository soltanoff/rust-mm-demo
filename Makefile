UNSAFE_TOTAL  := $(shell grep -r 'unsafe' src/ --include='*.rs' | wc -l | tr -d ' ')
UNSAFE_BLOCKS := $(shell grep -r 'unsafe {' src/ --include='*.rs' | wc -l | tr -d ' ')
UNSAFE_IMPLS  := $(shell grep -r 'unsafe impl' src/ --include='*.rs' | wc -l | tr -d ' ')
FILES_UNSAFE  := $(shell grep -rl 'unsafe' src/ --include='*.rs' | wc -l | tr -d ' ')
FILES_TOTAL   := $(shell find src/ -name '*.rs' | wc -l | tr -d ' ')

RUN_MIRI = MIRIFLAGS=-Zmiri-backtrace=full cargo +nightly miri test


pre-commit: update-badges format check test build

update-badges:
	@echo "Updating unsafe counters in README.md..."
	@echo "  unsafe usages : $(UNSAFE_TOTAL)"
	@echo "  unsafe blocks : $(UNSAFE_BLOCKS)"
	@echo "  unsafe impl   : $(UNSAFE_IMPLS)"
	@echo "  files w/unsafe: $(FILES_UNSAFE) / $(FILES_TOTAL)"
	@sed -i '' 's#unsafe%20usages-[0-9]*-#unsafe%20usages-$(UNSAFE_TOTAL)-#' README.md
	@sed -i '' 's#unsafe%20blocks-[0-9]*-#unsafe%20blocks-$(UNSAFE_BLOCKS)-#' README.md
	@sed -i '' 's#unsafe%20impl-[0-9]*-#unsafe%20impl-$(UNSAFE_IMPLS)-#' README.md
	@sed -i '' 's#files%20with%20unsafe-[0-9]*%20#files%20with%20unsafe-$(FILES_UNSAFE)%20#' README.md
	@sed -i '' 's#%2F%20[0-9]*-blueviolet#%2F%20$(FILES_TOTAL)-blueviolet#' README.md
	@echo "Done"

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
