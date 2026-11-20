.PHONY: build release test fmt fmt-check clippy lint clean install setup ui ui-test

build:
	cargo build

# Release builds embed the local web UI, so build it first (Node 22.17+, pnpm).
release: ui
	cargo build --release

test:
	cargo test --all-features

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets --all-features

lint: fmt-check clippy

clean:
	cargo clean

install: ui
	cargo install --path .

setup:
	git config core.hooksPath .githooks
	@echo "Git hooks configured."

# ── Local web UI (`recalld ui`) ──────────────────────────────────
ui:
	pnpm -C ui install --frozen-lockfile
	pnpm -C ui build

ui-test: ui
	pnpm -C ui check
	pnpm -C ui test
	cargo test --lib ui::
