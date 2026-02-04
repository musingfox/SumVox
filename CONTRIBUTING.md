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

(For maintainers only)

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Create git tag: `git tag -a v1.x.x -m "Release v1.x.x"`
4. Push tag: `git push origin v1.x.x`
5. GitHub Actions will build and create release

## Getting Help

- **Issues**: Search existing issues or create new one
- **Discussions**: For questions and ideas
- **Pull Requests**: For code contributions

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

## Thank You!

Your contributions make SumVox better for everyone. We appreciate your time and effort! 🎉
