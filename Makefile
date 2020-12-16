
lint:
	cargo fmt
serve:
	# cargo install cargo-watch
	RUST_LOG=rhymer=trace cargo watch -x 'run --example simple'
