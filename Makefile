.PHONY: build release test fmt fmt-check clippy lint clean install setup ui ui-test release-ui

build:
	cargo build

release:
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

install:
	cargo install --path .

setup:
	git config core.hooksPath .githooks
	@echo "Git hooks configured."

# ── Optional local web UI (`recalld ui`) ─────────────────────────
ui:
	pnpm -C ui install --frozen-lockfile
	pnpm -C ui build

ui-test: ui
	pnpm -C ui check
	pnpm -C ui test
	cargo test --features ui --lib ui::

release-ui: ui
	cargo build --release --features ui
