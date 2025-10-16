# Contributing to Google Calendar MCP Server

Thank you for your interest in contributing! This document provides guidelines and instructions for contributing to this project.

## Getting Started

### Prerequisites
- Rust nightly toolchain (see `rust-toolchain.toml`)
- Google Cloud project with Calendar API enabled
- Basic understanding of OAuth 2.0 and MCP (Model Context Protocol)

### Development Setup

1. **Fork and clone the repository**
   ```bash
   git clone https://github.com/YOUR_USERNAME/mcp-google-calendar.git
   cd mcp-google-calendar
   ```

2. **Set up configuration**
   ```bash
   cp .env.example .env
   # Edit .env with your Google OAuth credentials
   ```

3. **Build and test**
   ```bash
   make run    # Start development server
   make test   # Run tests
   make fmt    # Format code
   make clippy # Run linter
   ```

## How to Contribute

### Reporting Issues

Before creating an issue, please:
- Search existing issues to avoid duplicates
- Collect relevant information (OS, Rust version, error messages, logs)
- Provide clear steps to reproduce bugs

When creating an issue, include:
- Clear description of the problem or feature request
- Expected vs actual behavior
- Environment details (OS, Rust version, deployment platform)
- Relevant logs with `RUST_LOG=debug` enabled

### Submitting Pull Requests

1. **Create a feature branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**
   - Follow the existing code style (enforced by `rustfmt`)
   - Add tests for new functionality
   - Update documentation as needed
   - Keep commits focused and atomic

3. **Test your changes**
   ```bash
   make test
   make clippy
   make fmt
   ```

4. **Commit with clear messages**
   ```bash
   git commit -m "feat: add event recurrence support"
   ```

   Follow [Conventional Commits](https://www.conventionalcommits.org/):
   - `feat:` New features
   - `fix:` Bug fixes
   - `docs:` Documentation changes
   - `refactor:` Code refactoring
   - `test:` Test additions or changes
   - `chore:` Build process or tooling changes

5. **Push and create PR**
   ```bash
   git push origin feature/your-feature-name
   ```
   Then open a pull request on GitHub with:
   - Clear description of changes
   - Link to related issues
   - Screenshots/logs if relevant

## Code Guidelines

### Rust Style
- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo +nightly fmt` to format code
- Address all `cargo +nightly clippy` warnings
- Prefer explicit error types over `anyhow` in library code
- Document public APIs with doc comments

### Architecture Principles
- **Separation of concerns**: Keep OAuth, MCP tools, and Google API client decoupled
- **Pluggable storage**: Use the `TokenStorage` trait for new storage backends
- **Error handling**: Wrap Google API errors consistently in MCP error types
- **Security**: Never log or expose OAuth tokens, client secrets, or user data

### Testing
- Write unit tests for business logic
- Integration tests should mock Google API when possible
- Test OAuth flows require manual verification (see `docs/testing.md`)
- Aim for >70% code coverage on critical paths

### Documentation
- Update README.md for user-facing changes
- Update `docs/` for architecture or deployment changes
- Add inline comments for complex logic
- Update CLAUDE.md for changes affecting AI coding agents

## Project Structure

```
src/
├── config/       # Configuration loading and validation
├── oauth/        # OAuth 2.0 + PKCE implementation
├── mcp/          # MCP tool definitions and server
├── google_calendar/ # Google Calendar API client
├── proxy/        # OAuth 2.1 DCR proxy (optional)
├── handlers/     # HTTP endpoint handlers
└── main.rs       # Application entry point
```

## Security

### Reporting Security Issues
**Do not open public issues for security vulnerabilities.**

Instead, please use [GitHub Security Advisories](https://github.com/kamekamek/mcp-google-calendar/security/advisories/new) to report security vulnerabilities privately.

### Security Guidelines
- Never commit credentials (`.env`, `Secrets.toml`, `*.pem`)
- Validate all user inputs (especially `user_id`, event data)
- Use HTTPS in production deployments
- Follow OAuth 2.0 security best practices
- Review the security checklist in `docs/security.md`

## Community

### Code of Conduct
This project adheres to the Contributor Covenant [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

### Getting Help
- Read the [documentation](docs/)
- Check existing issues and discussions
- Ask questions in GitHub Discussions
- Join our community chat (if applicable)

## Development Resources

- [MCP Specification](https://modelcontextprotocol.io/)
- [Google Calendar API Reference](https://developers.google.com/calendar/api/v3/reference)
- [OAuth 2.0 RFC](https://datatracker.ietf.org/doc/html/rfc6749)
- [Rust Async Book](https://rust-lang.github.io/async-book/)

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
