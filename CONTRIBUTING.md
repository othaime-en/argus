# Contributing to ARGUS

Thank you for considering contributing to ARGUS! This document provides guidelines and information to help you contribute effectively.

## Getting Started

ARGUS is an open-source terminal user interface for monitoring CI/CD pipelines. The project is in active development, and we welcome contributions of all kinds—from bug reports to feature implementations.

### Development Environment Setup

Before you begin, ensure you have:

1. **Rust toolchain** - Install from [rustup.rs](https://rustup.rs/)
2. **Git** - For version control
3. **A GitHub account** - For testing GitHub Actions integration

Clone the repository and build the project:

```bash
git clone https://github.com/othaime-en/argus.git
cd argus
cargo build
```

Run the test suite to verify everything works:

```bash
cargo test
```

## Development Workflow

### Making Changes

We follow a standard Git workflow:

1. **Fork the repository** on GitHub
2. **Create a feature branch** from `main`:
   ```bash
   git checkout -b feature/my-new-feature
   ```
3. **Make your changes** with clear, focused commits
4. **Add tests** for new functionality
5. **Run the test suite** to ensure nothing broke
6. **Push your branch** and open a Pull Request

### Commit Messages

We use conventional commit format for clarity:

- `feat: add GitLab CI integration` - New features
- `fix: handle rate limiting correctly` - Bug fixes
- `docs: update installation guide` - Documentation changes
- `refactor: simplify polling logic` - Code improvements
- `test: add unit tests for config parsing` - Test additions
- `chore: update dependencies` - Maintenance tasks

Each commit message should explain **what** changed and **why**, not just **how**. The code diff shows how something changed, but your commit message should provide context that isn't obvious from reading the code.

### Code Style

ARGUS follows standard Rust conventions:

- Run `cargo fmt` before committing to ensure consistent formatting
- Run `cargo clippy` to catch common mistakes and non-idiomatic code
- Add doc comments (`///`) for all public APIs
- Keep functions focused and under 50 lines when possible
- Use descriptive variable names—clarity over brevity

Our `rustfmt.toml` configuration enforces:
- 100 character line width
- Four-space indentation
- Automatic import reordering

### Testing

When adding new features, include tests that verify:

1. **Happy path** - The feature works as intended
2. **Error cases** - The feature handles failures gracefully
3. **Edge cases** - Boundary conditions are handled correctly

For API clients, use mocked responses rather than making real network calls. See the existing tests in `src/api/github.rs` for examples.

## Areas for Contribution

### High-Priority Items

These are features from the implementation plan that would have immediate impact:

- **GitLab CI integration** - Implement the `CIPlatform` trait for GitLab
- **Jenkins integration** - Add support for Jenkins pipelines
- **Search functionality** - Add filtering and search to the pipeline list
- **Notification system** - Email, Slack, or webhook notifications for pipeline events

### Good First Issues

If you're new to the project, consider starting with:

- **Documentation improvements** - Add examples, clarify instructions, fix typos
- **Error message improvements** - Make errors more helpful and actionable
- **Theme additions** - Create new color schemes
- **Configuration validation** - Better error messages for invalid configs
- **Unit test coverage** - Add tests for untested code paths

### Architecture Guidelines

ARGUS follows a clean separation of concerns:

- **`api/`** - Platform-specific API clients that implement the `CIPlatform` trait
- **`models/`** - Data structures representing pipelines, stages, and logs
- **`services/`** - Background services like the polling mechanism
- **`state/`** - Application state management
- **`ui/`** - Terminal user interface rendering
- **`utils/`** - Shared utilities for errors, time formatting, etc.

When adding new features:
- Keep platform-specific code in the `api/` module
- Use the existing error types rather than panicking
- Update the configuration schema if you add new settings
- Follow the patterns established in existing code

## Pull Request Process

When you open a pull request:

1. **Describe your changes** - Explain what the PR does and why
2. **Link related issues** - Reference any issues this PR addresses
3. **Show before/after** - For UI changes, include screenshots if possible
4. **Confirm tests pass** - Both your new tests and existing ones
5. **Wait for review** - A maintainer will review your code

We aim to review PRs within a few days. If you haven't heard back within a week, feel free to ping the reviewers.

### What We Look For

During code review, we check:

- **Correctness** - Does the code do what it claims?
- **Testing** - Are there tests that prove it works?
- **Error handling** - What happens when things go wrong?
- **Performance** - Will this scale to 100+ pipelines?
- **Documentation** - Can others understand and use this?
- **Consistency** - Does it fit with the rest of the codebase?

We may ask for changes before merging. This is normal and helps maintain code quality. Don't be discouraged—view it as collaboration toward making your contribution even better.

## Reporting Bugs

If you find a bug, please open an issue on GitHub with:

- **What you expected** to happen
- **What actually happened** instead
- **Steps to reproduce** the issue
- **Your environment** (OS, Rust version, ARGUS version)
- **Relevant logs** if available

The more detail you provide, the easier it is for us to fix the problem.

## Feature Requests

We welcome feature ideas! Before opening an issue:

1. Check if someone else has already suggested it
2. Consider whether it fits ARGUS's core mission
3. Think about how it would work in a terminal interface

When proposing a feature, describe:
- **The problem** you're trying to solve
- **Your proposed solution** (but be open to alternatives)
- **Alternatives you've considered** and why they're not ideal

## License

By contributing to ARGUS, you agree that your contributions will be licensed under the MIT License, the same license covering the project.

## Questions?

If you have questions about contributing, feel free to:
- Open a discussion on GitHub
- Ask in an issue or pull request
- Reach out to the maintainers

Thank you for helping make ARGUS better!