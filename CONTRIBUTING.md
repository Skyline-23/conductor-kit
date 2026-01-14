# Contributing to Conductor Kit

Thank you for your interest in contributing to Conductor Kit! This document provides guidelines and instructions for contributing.

## Development Setup

### Prerequisites

- Go 1.23 or later
- A host CLI: Codex CLI, Claude Code, or OpenCode

### Building from Source

```bash
git clone https://github.com/Skyline-23/conductor-kit
cd conductor-kit
go build -o conductor ./cmd/conductor
```

### Running Tests

```bash
go test ./cmd/conductor -v
```

## How to Contribute

### Reporting Issues

- Use the [GitHub Issues](https://github.com/Skyline-23/conductor-kit/issues) page
- Include your OS, Go version, and CLI versions
- Provide minimal reproduction steps

### Submitting Changes

1. Fork the repository
2. Create a feature branch: `git checkout -b feat/my-feature`
3. Make your changes
4. Run tests: `go test ./cmd/conductor`
5. Commit with conventional commit messages (see below)
6. Push and open a Pull Request

## Commit Message Format

This project uses [Conventional Commits](https://www.conventionalcommits.org/). Each commit message should follow this format:

```
type: description
```

### Types

| Type       | Description                                      |
|------------|--------------------------------------------------|
| `feat`     | New feature                                      |
| `fix`      | Bug fix                                          |
| `docs`     | Documentation changes                            |
| `refactor` | Code refactoring (no functional change)          |
| `test`     | Adding or updating tests                         |
| `chore`    | Maintenance tasks (build, deps, etc.)            |

### Examples

```
feat: add --version flag with goreleaser ldflags support
fix: remove deprecated rand.Seed() call for Go 1.20+
docs: add godoc comments to config types and functions
test: add config module unit tests
refactor: improve error handling in install with warning messages
```

## Code Style

- Follow standard Go formatting (`go fmt`)
- Add godoc comments for exported types and functions
- Keep functions small and focused
- Handle errors explicitly (no silent `_ = err`)

## Project Structure

```
conductor-kit/
  cmd/conductor/     # Go CLI source code
  commands/          # Markdown commands for Codex/Claude/OpenCode
  config/            # Default configuration files
  skills/conductor/  # Main conductor skill
```

## Areas for Contribution

- Additional unit tests for core modules
- Linux installation improvements
- New MCP tool integrations
- Documentation improvements
- Bug fixes and error handling

## Questions?

Open a [Discussion](https://github.com/Skyline-23/conductor-kit/discussions) or reach out via Issues.
