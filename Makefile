.PHONY: all pkg server ui
all: server

pkg: server
	@echo Building deb package...
	@strip target/release/netholdem-server
	@cargo deb -q -p netholdem-server 2>/dev/null

server: ui
	@echo Building server...
	@cargo build -q --release -p netholdem-server

ui:
	@echo Building WASM...
	@cd ui; \
		wasm-pack -q build --target web --no-typescript . 2>/dev/null
	@echo Building bundle...
	@rm -rf ui/build
	@mkdir -p ui/build
	@rollup ./ui/static/main.js --format iife --silent |\
		uglifyjs -c >./ui/build/bundle.js
	@cp ui/pkg/*.wasm ui/build/
	@cp ui/static/*.html ui/build/
	@cp ui/static/*.json ui/build/
