# Justfile for SumVox development and release tasks
# Install just: cargo install just

# Default recipe - show help
default:
    @just --list

# Run all tests
test:
    cargo test

# Run tests with output
test-verbose:
    cargo test -- --nocapture

# Run specific module tests
test-module MODULE:
    cargo test {{MODULE}}::

# Build debug version
build:
    cargo build

# Build release version
build-release:
    cargo build --release

# Run with debug logging
run *ARGS:
    RUST_LOG=debug cargo run -- {{ARGS}}

# Format code
fmt:
    cargo fmt

# Check formatting without making changes
fmt-check:
    cargo fmt -- --check

# Run clippy linter
lint:
    cargo clippy -- -D warnings

# Run all checks (fmt, lint, test)
check: fmt-check lint test

# Clean build artifacts
clean:
    cargo clean
    rm -f *.tar.gz SHA256SUMS*.txt

# Install locally
install:
    cargo install --path .

# Create a release (requires version argument)
release VERSION:
    #!/usr/bin/env bash
    set -euo pipefail

    echo "Preparing release {{VERSION}}"

    # Check if working directory is clean
    if ! git diff-index --quiet HEAD --; then
        echo "Error: Working directory has uncommitted changes"
        exit 1
    fi

    # Update version in Cargo.toml
    sed -i '' 's/^version = ".*"/version = "{{VERSION}}"/' Cargo.toml

    # Update version in Homebrew formula
    sed -i '' 's/version ".*"/version "{{VERSION}}"/' homebrew/sumvox.rb
    sed -i '' 's|url "https://github.com/.*/archive/refs/tags/v.*\.tar\.gz"|url "https://github.com/musingfox/sumvox/archive/refs/tags/v{{VERSION}}.tar.gz"|' homebrew/sumvox.rb

    # Run tests
    cargo test

    # Commit version bump
    git add Cargo.toml homebrew/sumvox.rb
    git commit -m "chore: bump version to {{VERSION}}"

    # Create and push tag
    git tag -a "v{{VERSION}}" -m "Release v{{VERSION}}"

    echo "Ready to push! Run:"
    echo "  git push origin main"
    echo "  git push origin v{{VERSION}}"

# Build release tarball for current platform
package VERSION:
    #!/usr/bin/env bash
    set -euo pipefail

    # Detect platform
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')
    ARCH=$(uname -m)

    if [ "$ARCH" = "arm64" ]; then
        ARCH="aarch64"
    fi

    NAME="sumvox-${OS}-${ARCH}"

    # Build release
    cargo build --release

    # Create tarball
    cd target/release
    tar czf "../../${NAME}.tar.gz" sumvox
    cd ../..

    # Calculate SHA256
    shasum -a 256 "${NAME}.tar.gz" | tee SHA256SUMS.txt

    echo "Created ${NAME}.tar.gz"

# Update Homebrew formula SHA256
update-formula VERSION:
    #!/usr/bin/env bash
    set -euo pipefail

    URL="https://github.com/musingfox/sumvox/archive/refs/tags/v{{VERSION}}.tar.gz"

    echo "Calculating SHA256 for $URL..."
    SHA256=$(curl -sL "$URL" | shasum -a 256 | awk '{print $1}')

    echo "SHA256: $SHA256"

    # Update formula
    sed -i '' "s/sha256 \".*\"/sha256 \"$SHA256\"/" homebrew/sumvox.rb
    sed -i '' "s/url \".*\"/url \"$URL\"/" homebrew/sumvox.rb
    sed -i '' "s/version \".*\"/version \"{{VERSION}}\"/" homebrew/sumvox.rb

    echo "Updated homebrew/sumvox.rb"

# Test Homebrew formula locally
test-formula:
    brew install --build-from-source ./homebrew/sumvox.rb
    brew test sumvox
    brew audit --strict sumvox

# Initialize config (for testing)
init:
    ./target/release/sumvox init

# Set credentials (for testing)
credentials PROVIDER:
    ./target/release/sumvox credentials set {{PROVIDER}}

# Show config
show-config:
    cat ~/.config/sumvox/config.json | jq .
