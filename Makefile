.PHONY: help build run test clean fmt lint check install doc release

# Default target
help:
	@echo "ARGUS Development Commands"
	@echo "=========================="
	@echo ""
	@echo "  make build    - Build the project in debug mode"
	@echo "  make release  - Build the project in release mode"
	@echo "  make run      - Build and run the project"
	@echo "  make test     - Run all tests"
	@echo "  make fmt      - Format code with rustfmt"
	@echo "  make lint     - Run clippy for linting"
	@echo "  make check    - Run fmt, lint, and test"
	@echo "  make clean    - Clean build artifacts"
	@echo "  make install  - Install the binary locally"
	@echo "  make doc      - Generate and open documentation"
	@echo ""

# Build in debug mode
build:
	@echo "Building ARGUS (debug)..."
	cargo build

# Build in release mode
release:
	@echo "Building ARGUS (release)..."
	cargo build --release

# Build and run
run:
	@echo "Running ARGUS..."
	cargo run

# Run tests
test:
	@echo "Running tests..."
	cargo test

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean

# Format code
fmt:
	@echo "Formatting code..."
	cargo fmt

# Lint with clippy
lint:
	@echo "Running clippy..."
	cargo clippy -- -D warnings

# Check everything
check: fmt lint test
	@echo "All checks passed!"

# Install locally
install:
	@echo "Installing ARGUS..."
	cargo install --path .

# Generate and open documentation
doc:
	@echo "Generating documentation..."
	cargo doc --open

# Create a new user config from example
setup-config:
	@echo "Setting up user configuration..."
	@mkdir -p ~/.config/argus
	@if [ ! -f ~/.config/argus/config.toml ]; then \
		cp config/example.toml ~/.config/argus/config.toml; \
		echo "Configuration created at ~/.config/argus/config.toml"; \
		echo "Edit this file and set your API tokens!"; \
	else \
		echo "Configuration already exists at ~/.config/argus/config.toml"; \
	fi