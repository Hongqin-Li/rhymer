
lint:
	cargo fmt
serve:
	# cargo install cargo-watch
	RUST_LOG=rhymer=trace cargo watch -x 'run --example simple'
test:
	# --jobs=1 to force sequential testing since we need to clean database before each test.
	RUST_BACKTRACE=1 RUST_LOG=rhymer=trace cargo test --jobs 1 -- --nocapture
