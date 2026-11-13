.PHONY: dev test test-fast test-doc check build docker-build docker-up docker-down clean

dev:
	cargo run

# Full suite: parallel per-test processes plus doc tests (nextest skips doctests).
test:
	cargo nextest run
	cargo test --doc

# Fast inner loop: library tests only, no doctests.
test-fast:
	cargo nextest run --lib

test-doc:
	cargo test --doc

check:
	cargo check

build:
	cargo build --release

docker-build:
	docker build -t aina:latest .

docker-up:
	docker compose up -d

docker-down:
	docker compose down

clean:
	cargo clean
