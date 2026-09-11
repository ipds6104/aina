.PHONY: dev test check build docker-build docker-up docker-down clean

dev:
	cargo run

test:
	cargo test

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
