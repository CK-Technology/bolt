# Contributing to Bolt

Thank you for your interest in contributing to Bolt.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/bolt`
3. Create a branch: `git checkout -b feature/your-feature`
4. Make your changes
5. Run tests: `cargo test`
6. Submit a pull request

## Development Setup

### Requirements
- Rust 1.91+
- Linux (kernel 5.4+)
- For GPU testing: NVIDIA, AMD, or Intel GPU with appropriate drivers

### Build
```bash
cargo build --release
```

### Test
```bash
cargo test
cargo test --all-features
```

### Lint
```bash
cargo clippy
cargo fmt --check
```

## Code Guidelines

- Follow Rust idioms and conventions
- Use `cargo fmt` before committing
- Add tests for new functionality
- Update documentation for user-facing changes
- Keep commits focused and atomic

## Pull Request Process

1. Ensure all tests pass
2. Update relevant documentation
3. Add a clear description of changes
4. Reference any related issues

## Reporting Issues

- Use GitHub Issues for bug reports and feature requests
- Include reproduction steps for bugs
- Include system information (OS, GPU, driver versions)

## Code of Conduct

Be respectful and constructive in all interactions.

## License

By contributing, you agree that your contributions will be licensed under the same license as the project.
