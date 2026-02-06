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

    # Update version and URLs in Homebrew formula
    sed -i '' 's/version ".*"/version "{{VERSION}}"/' homebrew/sumvox.rb
    sed -i '' 's|releases/download/v[^/]*/sumvox-|releases/download/v{{VERSION}}/sumvox-|g' homebrew/sumvox.rb

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
    echo ""
    echo "CI will automatically update SHA-256 hashes in the formula."

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

# Update Homebrew formula SHA256 from GitHub Release binaries (manual fallback; CI does this automatically)
update-formula VERSION:
    #!/usr/bin/env bash
    set -euo pipefail

    REPO="musingfox/sumvox"
    BASE_URL="https://github.com/${REPO}/releases/download/v{{VERSION}}"

    PLATFORMS=("sumvox-macos-aarch64" "sumvox-macos-x86_64" "sumvox-linux-aarch64" "sumvox-linux-x86_64")

    declare -A SHAS

    for PLATFORM in "${PLATFORMS[@]}"; do
        URL="${BASE_URL}/${PLATFORM}.tar.gz"
        echo "Downloading ${PLATFORM}.tar.gz..."
        SHA=$(curl -sL "$URL" | shasum -a 256 | awk '{print $1}')
        SHAS[$PLATFORM]=$SHA
        echo "  SHA256: $SHA"
    done

    python3 << PYEOF
    import re

    version = "{{VERSION}}"
    shas = {
        "sumvox-macos-aarch64": "${SHAS[sumvox-macos-aarch64]}",
        "sumvox-macos-x86_64": "${SHAS[sumvox-macos-x86_64]}",
        "sumvox-linux-aarch64": "${SHAS[sumvox-linux-aarch64]}",
        "sumvox-linux-x86_64": "${SHAS[sumvox-linux-x86_64]}",
    }

    with open("homebrew/sumvox.rb", "r") as f:
        content = f.read()

    content = re.sub(r'version ".*?"', f'version "{version}"', content)
    content = re.sub(
        r'(releases/download/v)[^/]+(/.+?\.tar\.gz)',
        rf'\g<1>{version}\2',
        content,
    )

    for name, sha in shas.items():
        pattern = rf'(url ".*?{name}\.tar\.gz"\s*\n\s*sha256 )".*?"'
        replacement = rf'\1"{sha}"'
        content = re.sub(pattern, replacement, content)

    with open("homebrew/sumvox.rb", "w") as f:
        f.write(content)

    print(f"Updated homebrew/sumvox.rb to v{version}")
    PYEOF

# Test Homebrew formula locally
test-formula:
    brew install ./homebrew/sumvox.rb
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
