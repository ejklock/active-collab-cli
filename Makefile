# All targets wrap the Docker toolchain — there is no local Rust toolchain required.

.DEFAULT_GOAL := binary

PREFIX ?= $(HOME)/.local
BINDIR := $(PREFIX)/bin

UNAME_M := $(shell uname -m)
ifeq ($(UNAME_M),arm64)
NATIVE_TARGET := aarch64-apple-darwin
else
NATIVE_TARGET := x86_64-apple-darwin
endif

.PHONY: binary build image run test comment-policy fmt clippy lint check clean install install-native uninstall help

binary:
	docker compose run --rm dev cargo build --release

build: binary

image:
	docker compose --profile release build build

run:
	docker compose run --rm dev cargo run

fix: 
	docker compose run --rm dev cargo fix --bin "ac" -p ac

test:
	docker compose run --rm dev cargo test

comment-policy:
	docker compose run --rm dev cargo test --test comment_policy

fmt:
	docker compose run --rm dev cargo fmt --check

clippy:
	docker compose run --rm dev cargo clippy --all-targets -- -D warnings

lint: fmt clippy

check: fmt clippy test comment-policy

clean:
	docker compose run --rm dev cargo clean

install: binary
	mkdir -p $(BINDIR)
	install -m 0755 target/release/ac $(BINDIR)/ac
	@echo "Installed: $(BINDIR)/ac"
	@echo "Ensure $(BINDIR) is on your PATH. For a system-wide install: make install PREFIX=/usr/local (with sudo)"

# Docker binary is Linux-only (ELF); install-native bypasses Docker and needs a host cargo.
install-native:
	cargo build --release --target $(NATIVE_TARGET)
	mkdir -p $(BINDIR)
	install -m 0755 target/$(NATIVE_TARGET)/release/ac $(BINDIR)/ac
	@echo "Installed (native): $(BINDIR)/ac"
	@echo "Ensure $(BINDIR) is on your PATH. For a system-wide install: make install-native PREFIX=/usr/local (with sudo)"

uninstall:
	rm -f $(BINDIR)/ac
	@echo "Removed: $(BINDIR)/ac"

help:
	@echo "Usage: make <target>"
	@echo ""
	@echo "  binary          Build release binary to ./target/release/ac (default)"
	@echo "  build           Alias for binary"
	@echo "  image           Build release Docker image (runtime, profile=release)"
	@echo "  run             Run the dev binary via cargo run"
	@echo "  test            Run the full test suite"
	@echo "  comment-policy  Run the comment-policy gate (cargo test --test comment_policy)"
	@echo "  fmt             Check formatting (cargo fmt --check)"
	@echo "  clippy          Run clippy with -D warnings"
	@echo "  lint            Run fmt + clippy"
	@echo "  check           Run fmt + clippy + test + comment-policy (full local gate)"
	@echo "  clean           Remove build artifacts"
	@echo "  install         Install ac to \$$(BINDIR) (default: ~/.local/bin); override with PREFIX=/usr/local"
	@echo "  install-native  Native macOS build; requires a host Rust toolchain (bypasses Docker)"
	@echo "  uninstall       Remove ac from \$$(BINDIR)"
	@echo "  help            Show this message"
