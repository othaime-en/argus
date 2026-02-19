# Changelog

All notable changes to ARGUS will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-02-19

### Added
- Initial release with GitHub Actions support
- Real-time pipeline monitoring with automatic 30-second refresh intervals
- Multi-repository monitoring from a single GitHub organization or user
- Three-panel TUI interface with pipeline list, stage details, and log viewer
- Keyboard-driven navigation (no mouse required)
- Stage-by-stage workflow job breakdown with status indicators
- Full log viewing for any workflow job
- Color-coded status indicators for quick visual scanning
- Error tracking panel to monitor API failures
- Configuration via TOML files with environment variable support
- Graceful error handling with retry logic and exponential backoff
- Rate limit detection and handling for GitHub API
- Connection testing on startup
- Four built-in themes: default, dark, light, and monokai
- Comprehensive unit tests for core functionality

### Known Limitations
- Only GitHub Actions is supported in this release
- No search or filtering capabilities yet
- No notification system yet
- No historical data tracking yet
- Log viewing requires manual load (press 'l')


[0.1.0]: https://github.com/othaime-en/argus/releases/tag/v0.1.0