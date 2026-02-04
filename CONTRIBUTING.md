# Contributing to SumVox

Thank you for your interest in contributing to SumVox! This guide will help you get started.

## Code of Conduct

Be respectful, constructive, and collaborative. We're all here to build better tools.

## Development Setup

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- macOS 10.15+ or Linux (for development)
- API keys for testing (Gemini recommended)

### Getting Started

1. **Fork and clone** the repository:
   ```bash
   git fork https://github.com/musingfox/sumvox
   cd sumvox
   ```

2. **Build the project**:
   ```bash
   cargo build
   ```

3. **Run tests**:
   ```bash
   cargo test
   ```

4. **Set up development config**:
   ```bash
   cargo run -- init
   cargo run -- credentials set google
   ```

### Project Structure

```
sumvox/
├── src/
│   ├── main.rs           # Entry point and hook orchestration
│   ├── cli.rs            # CLI argument parsing
│   ├── config.rs         # Configuration management
│   ├── transcript.rs     # Claude Code transcript parsing
│   ├── credentials.rs    # Credential management
│   ├── error.rs          # Error types
│   ├── llm/              # LLM providers
│   │   ├── mod.rs
│   │   ├── google.rs     # Gemini integration
│   │   ├── anthropic.rs  # Claude integration
│   │   ├── openai.rs     # GPT integration
│   │   └── ollama.rs     # Local Ollama integration
│   ├── tts/              # TTS engines
│   │   ├── mod.rs
│   │   ├── google.rs     # Google TTS
│   │   └── macos.rs      # macOS say command
│   └── provider_factory.rs  # Provider creation
├── config/
│   └── recommended.json  # Recommended configuration
├── tests/                # Integration tests
└── Cargo.toml           # Dependencies and metadata
```

## Testing

### Running Tests

```bash
# All tests
cargo test

# Specific module
cargo test llm::
cargo test tts::

# With output
cargo test -- --nocapture

# Specific test
cargo test test_gemini_api
```

### Writing Tests

- Unit tests: Add `#[cfg(test)]` modules in source files
- Integration tests: Add files to `tests/` directory
- Mock external APIs using `mockito` crate
- Test both success and error paths

Example:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_fallback() {
        // Test implementation
    }
}
```

## Code Style

### Formatting

- Use `rustfmt` for consistent formatting:
  ```bash
  cargo fmt
  ```

### Linting

- Run `clippy` before committing:
  ```bash
  cargo clippy -- -D warnings
  ```

### Conventions

- Use descriptive variable names
- Add comments for complex logic only
- Write self-documenting code
- Follow Rust naming conventions:
  - `snake_case` for functions and variables
  - `PascalCase` for types and structs
  - `SCREAMING_SNAKE_CASE` for constants

## Commit Messages

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, no logic change)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

### Examples

```bash
feat(llm): add Claude Sonnet 4 support

Add support for the latest Claude Sonnet 4 model with improved
performance and quality.

Closes #123

fix(tts): handle empty summary gracefully

Previously crashed when LLM returned empty string. Now falls back
to configured fallback_message.

docs(readme): update installation instructions

Add Homebrew installation method and clarify prerequisites.
```

## Pull Request Process

### Before Submitting

1. **Create a feature branch**:
   ```bash
   git checkout -b feat/your-feature-name
   ```

2. **Make your changes**:
   - Write tests for new features
   - Update documentation
   - Follow code style guidelines

3. **Verify everything works**:
   ```bash
   cargo test
   cargo clippy -- -D warnings
   cargo fmt --check
   ```

4. **Commit with conventional commits**:
   ```bash
   git add .
   git commit -m "feat(scope): your message"
   ```

### Submitting PR

1. **Push to your fork**:
   ```bash
   git push origin feat/your-feature-name
   ```

2. **Create PR on GitHub**:
   - Use a clear, descriptive title
   - Reference related issues
   - Describe what changed and why
   - Include testing notes

3. **PR Template** (will auto-fill):
   - Description of changes
   - Type of change (feature, fix, docs, etc.)
   - Testing performed
   - Checklist

### Review Process

- Maintainers will review your PR
- Address feedback and push updates
- Once approved, maintainers will merge

## Adding New Features

### Adding a New LLM Provider

1. Create `src/llm/your_provider.rs`:
   ```rust
   use async_trait::async_trait;
   use crate::error::Result;
   use crate::llm::LlmProvider;

   pub struct YourProvider {
       api_key: String,
       model: String,
   }

   #[async_trait]
   impl LlmProvider for YourProvider {
       async fn generate(&self, prompt: &str) -> Result<String> {
           // Implementation
       }
   }
   ```

2. Register in `src/llm/mod.rs`
3. Add config support in `src/config.rs`
4. Write tests
5. Update documentation

### Adding a New TTS Engine

Similar process - implement the `TtsEngine` trait.

## Debugging

### Enable Debug Logging

```bash
RUST_LOG=debug cargo run
```

### Common Issues

1. **Test failures**: Check API keys are set
2. **Build errors**: Try `cargo clean && cargo build`
3. **Permission issues**: Check file permissions for config

## Documentation

### Updating Documentation

- `README.md`: User-facing documentation
- `CLAUDE.md`: Project configuration for Claude
- `CONTRIBUTING.md`: This file
- Code comments: For complex logic only

### Building Documentation

```bash
cargo doc --open
```

## Release Process

*For maintainers only*

### Versioning

We follow [Semantic Versioning](https://semver.org/):

- `MAJOR.MINOR.PATCH` (e.g., 1.0.0)
- **MAJOR**: Breaking changes (not backward compatible)
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes (backward compatible)

### Release Checklist

- [ ] All tests pass (`cargo test`)
- [ ] Update version in `Cargo.toml`
- [ ] Update version-related info in README.md
- [ ] Update `CHANGELOG.md` with new version
- [ ] Commit all changes
- [ ] Create git tag (`git tag -a vX.Y.Z -m "Release vX.Y.Z"`)
- [ ] Push tag (`git push origin vX.Y.Z`)
- [ ] Wait for GitHub Actions to complete build
- [ ] Verify GitHub Release page
- [ ] Update Homebrew formula (if using tap)
- [ ] Test installation process

### Step-by-Step Release Guide

#### 1. Prepare Release

```bash
# Ensure all tests pass
cargo test

# Ensure code is committed
git status

# Update version in Cargo.toml
# Update CHANGELOG.md
```

#### 2. Create Version Tag

```bash
# Create and push tag (triggers GitHub Actions)
git tag -a v1.0.0 -m "Release v1.0.0"
git push origin v1.0.0
```

#### 3. Automated Build

GitHub Actions will automatically:
- Build binaries for macOS (x86_64, ARM64) and Linux (x86_64, ARM64)
- Create GitHub Release
- Upload archives and SHA256 checksums

#### 4. Update Homebrew Formula

##### Option A: Personal Tap (Recommended for early stages)

```bash
# 1. Create Homebrew tap repository
# Repository name must be: homebrew-<tap-name>
# Example: homebrew-sumvox

# 2. Copy formula
cp homebrew/sumvox.rb /path/to/homebrew-sumvox/Formula/

# 3. Update SHA256
# Get SHA256 from GitHub Release page
SHA256=$(curl -sL https://github.com/musingfox/sumvox/archive/refs/tags/v1.0.0.tar.gz | shasum -a 256 | awk '{print $1}')

# 4. Update url and sha256 in formula
sed -i '' "s/PLACEHOLDER_SHA256/$SHA256/" Formula/sumvox.rb

# 5. Commit and push
git add Formula/sumvox.rb
git commit -m "Release sumvox v1.0.0"
git push
```

Installation for users:
```bash
brew tap musingfox/sumvox
brew install sumvox
```

##### Option B: Submit to Homebrew Core (After project matures)

Homebrew Core requirements:
- Project has some notability and user base
- 50+ stars or 75+ forks in 30 days
- Continuous maintenance and updates
- Follows all Homebrew guidelines

Submission process:
```bash
# 1. Fork homebrew-core
# 2. Add formula to Formula/ directory
# 3. Test formula
brew install --build-from-source ./Formula/sumvox.rb
brew test sumvox
brew audit --strict sumvox

# 4. Submit PR to Homebrew/homebrew-core
```

#### 5. Publish to crates.io (Optional)

```bash
# 1. Login to crates.io
cargo login

# 2. Publish
cargo publish --dry-run  # Test first
cargo publish           # Official publish
```

Installation for users:
```bash
cargo install sumvox
```

### Rolling Back a Release

If you need to retract a release:

```bash
# Delete tag
git tag -d v1.0.0
git push origin :refs/tags/v1.0.0

# Delete GitHub Release (manual operation on GitHub)

# If published to crates.io (cannot delete, only yank)
cargo yank --vers 1.0.0
```

### Troubleshooting

#### Q: GitHub Actions build failed?

Check:
- Is the dependency version in Cargo.toml correct?
- Are cross-platform compilation tools installed?
- Check Actions logs for specific errors

#### Q: Homebrew formula test failed?

```bash
# Local testing
brew install --build-from-source ./homebrew/sumvox.rb
brew test sumvox
brew audit --strict sumvox
```

#### Q: How to update SHA256?

```bash
# Calculate SHA256 of tar.gz
curl -sL https://github.com/USER/REPO/archive/refs/tags/vX.Y.Z.tar.gz | shasum -a 256
```

### Resources

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Cargo Publishing Guide](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [Semantic Versioning](https://semver.org/)

## Getting Help

- **Issues**: Search existing issues or create new one
- **Discussions**: For questions and ideas
- **Pull Requests**: For code contributions

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

## Thank You!

Your contributions make SumVox better for everyone. We appreciate your time and effort! 🎉
