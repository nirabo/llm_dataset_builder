# ==========================================
#  LLM Dataset Builder - Development Tools
# ==========================================
#
# This Makefile provides commands for development, testing, and deployment.
#
# Quick Start:
#   make setup   - Set up development environment
#   make help    - Show all available commands
#   make check   - Run all checks before committing
#
# Common Commands:
#   make build   - Build debug version
#   make test    - Run all tests
#   make lint    - Check code style
#   make run     - Run the application
#
# For more information, see README.md
# ==========================================

# Variables
CARGO := cargo
RUSTC := rustc
RUSTFMT := rustfmt
CLIPPY := clippy-driver
TARGET_DIR := target
RELEASE_DIR := $(TARGET_DIR)/release
DEBUG_DIR := $(TARGET_DIR)/debug
BINARY_NAME := llm_dataset_builder
VENV_DIR := .venv
UV := uv

# Default target
.PHONY: all
all: help

# Development setup
.PHONY: setup
setup: setup-pre-commit setup-rust
	@echo "Development environment setup complete"



.PHONY: setup-pre-commit
setup-pre-commit:
	@command -v uv >/dev/null 2>&1 || { echo "Please install uv first: https://github.com/astral-sh/uv"; exit 1; }
	@echo "Creating virtual environment..."
	@uv venv $(VENV_DIR)
	@echo "Installing pre-commit..."
	$(UV) pip install pre-commit
	$(VENV_DIR)/bin/pre-commit install
	@echo "Pre-commit hooks installed"

.PHONY: setup-rust
setup-rust:
	@rustup component add rustfmt clippy
	@echo "Rust components installed"

# Building
.PHONY: build
build:
	$(CARGO) build

.PHONY: release
release:
	$(CARGO) build --release

# Testing
.PHONY: test
test: test-unit test-integration

.PHONY: test-unit
test-unit:
	$(CARGO) test --lib

.PHONY: test-integration
test-integration:
	$(CARGO) test --test '*'

.PHONY: test-coverage
test-coverage:
	@command -v cargo-tarpaulin >/dev/null 2>&1 || { \
		echo "Installing cargo-tarpaulin..."; \
		cargo install cargo-tarpaulin; \
	}
	@cargo tarpaulin --out Html

# Linting and formatting
.PHONY: lint
lint: fmt clippy

.PHONY: fmt
fmt:
	$(CARGO) fmt --all -- --check

.PHONY: fmt-fix
fmt-fix:
	$(CARGO) fmt --all

.PHONY: clippy
clippy:
	$(CARGO) clippy -- -D warnings

# Documentation
.PHONY: doc
doc:
	$(CARGO) doc --no-deps

# Cleaning
.PHONY: clean
clean:
	$(CARGO) clean
	rm -rf $(TARGET_DIR) $(VENV_DIR)

# Pre-commit checks
.PHONY: check
check: fmt clippy test

# Run the application
.PHONY: run
run:
	$(CARGO) run

# Install the application
.PHONY: install
install:
	$(CARGO) install --path .

# Create release artifacts
.PHONY: dist
dist: release
	@mkdir -p dist
	@cp $(RELEASE_DIR)/$(BINARY_NAME) dist/
	@cd dist && \
		tar czf $(BINARY_NAME)-linux.tar.gz $(BINARY_NAME) && \
		zip $(BINARY_NAME)-linux.zip $(BINARY_NAME)
	@echo "Release artifacts created in dist/"

# Help target
.PHONY: help
help:
	@echo "Available targets:"
	@echo "  setup         - Set up development environment (pre-commit hooks and Rust components)"
	@echo "  build         - Build the debug version"
	@echo "  release       - Build the release version"
	@echo "  test          - Run all tests"
	@echo "  test-unit     - Run unit tests"
	@echo "  test-integration - Run integration tests"
	@echo "  test-coverage - Generate test coverage report"
	@echo "  lint          - Run all linters"
	@echo "  fmt           - Check code formatting"
	@echo "  fmt-fix       - Fix code formatting"
	@echo "  clippy        - Run clippy lints"
	@echo "  doc           - Generate documentation"
	@echo "  clean         - Clean build artifacts"
	@echo "  check         - Run all checks (formatting, clippy, tests)"
	@echo "  run           - Run the application"
	@echo "  install       - Install the application"
	@echo "  dist          - Create release artifacts"
	@echo "  help          - Show this help message"

# File targets
$(TARGET_DIR):
	mkdir -p $(TARGET_DIR)

$(RELEASE_DIR): $(TARGET_DIR)
	mkdir -p $(RELEASE_DIR)

$(DEBUG_DIR): $(TARGET_DIR)
	mkdir -p $(DEBUG_DIR)
