# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `--version` / `-v` flag to display version information
- Godoc comments for config types and functions
- Unit tests for config module

### Changed
- Improved error handling in install with descriptive warning messages

### Fixed
- Removed deprecated `rand.Seed()` call for Go 1.20+ compatibility

## [0.1.45] - 2025-01-14

### Added
- Review, debug, test, refactor, explain, security, migrate commands
- Reference files for conductor skill (roles.md, delegation.md, formats.md)

### Changed
- Avoid hard timeouts and surface run context in MCP tools
- Simplify commands as skill wrappers with docs mode
- Aggressive skill description with STOP prefix and explicit triggers
- Change output format from JSON to markdown
- Enforce skill loading over built-in agents in description

## [0.1.44] - 2025-01-13

### Changed
- Maximize description for auto-activation
- Make SKILL.md proactive with explicit MCP delegation rules

### Fixed
- Remove auth unchecked badge
- Use user scope for claude mcp add

## [0.1.43] - 2025-01-12

### Added
- Improve settings role management

### Changed
- Balance SKILL.md - emphasize core rules, add missing details
- Minimize SKILL.md (59 to 43 lines)
- Streamline SKILL.md further (95 to 59 lines)

### Fixed
- Remove CLI column from roles table (config concern)

## [0.1.42] - 2025-01-11

### Changed
- Rename reference/ to references/ (agentskills.io standard)
- Add reference files for conductor skill
- Streamline conductor skill prompt
- Mandate oracle delegation for deep thinking tasks

### Fixed
- Use claude mcp add CLI command instead of direct file write

## [0.1.41] - 2025-01-10

### Changed
- Simplify install defaults

[Unreleased]: https://github.com/Skyline-23/conductor-kit/compare/v0.1.45...HEAD
[0.1.45]: https://github.com/Skyline-23/conductor-kit/compare/v0.1.44...v0.1.45
[0.1.44]: https://github.com/Skyline-23/conductor-kit/compare/v0.1.43...v0.1.44
[0.1.43]: https://github.com/Skyline-23/conductor-kit/compare/v0.1.42...v0.1.43
[0.1.42]: https://github.com/Skyline-23/conductor-kit/compare/v0.1.41...v0.1.42
[0.1.41]: https://github.com/Skyline-23/conductor-kit/compare/v0.1.40...v0.1.41
