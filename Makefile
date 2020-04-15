.PHONY: all client server ui
all: server

server: ui
	cargo build --release -p netholdem-server

ui: client
	cd ui; npm run build

client:
	cd client; wasm-pack build --release
