# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.2.0] - 2025-05-29

### Changed
- **BREAKING**: Launch command now requires instance name instead of version
- Updated CLI to use instance-based launching workflow
- Improved error messages for missing instances
- Updated documentation to reflect instance-based workflow

### Removed
- **BREAKING**: Removed default instance logic from launcher
- Removed automatic instance creation during launch

### Fixed
- Improved instance management workflow consistency
- Updated justfile commands to work with instances

## [0.1.6] - 2025-05-28

### Fixed
- Update GitHub Actions to use latest versions (actions/upload-artifact@v4, actions/cache@v4)
- Fix release workflow errors that caused build failures
- Simplify release body format and remove duplicate content
- Improve release verification logic to handle partial build failures gracefully

## [0.1.5] - 2025-05-28

### Changed
- Remove Cargo.lock from version control for better flexibility
- Update .gitignore to exclude Cargo.lock files

## [0.1.4] - 2025-05-28

### Added
- Comprehensive GitHub Actions release workflow for multi-platform builds
- Automatic release generation with Windows, macOS, and Linux binaries
- Release notes generation from CHANGELOG.md
- Just commands for development workflow

### Changed
- Updated release process to be fully automated
- Improved development workflow with better tooling

## [0.1.3] - 2025-05-28

### Added
- **Multi-Platform Releases**: Automated GitHub releases for Windows, macOS (Intel & Apple Silicon), and Linux
- **Release Automation**: Comprehensive release workflow with changelog extraction and asset generation
- **Cross-Platform Binaries**: Built and packaged binaries for all major operating systems

### Changed
- **Release Process**: Streamlined release creation with automatic changelog integration
- **Asset Naming**: Standardized release asset naming convention for better clarity

### Fixed
- **Release Workflow**: Complete rewrite of release automation for better reliability
- **Cross-Compilation**: Improved build process for multiple target architectures

## [0.1.2] - 2025-05-28

### Fixed
- **GitHub Releases**: Updated release workflow to use modern `softprops/action-gh-release` action
- **Release Creation**: Fixed issue where tags were created instead of proper GitHub releases
- **Asset Upload**: Improved binary asset upload process for releases

### Removed
- **Changelog Generator**: Permanently removed conflicting changelog automation workflow

## [0.1.1] - 2025-05-28

### Changed
- **Release Workflow**: Enhanced GitHub release creation with proper changelog content extraction
- **Repository Cleanup**: Removed automated changelog generation in favor of manual management

### Fixed
- **GitHub Actions**: Resolved race conditions between auto-format and changelog workflows

## [0.1.0] - 2025-05-28

### Added
- **Authentication**: Microsoft OAuth2 authentication flow with Azure integration
- **Instance Management**: Create and manage multiple Minecraft game instances
- **Java Detection**: Automatic detection of Java installations across platforms
- **Version Listing**: Fetch and display all available Minecraft releases and snapshots
- **Command Line Interface**: Full CLI with subcommands for launcher, auth, java, and instance management
- **Cross-Platform**: Support for Windows, macOS (Intel & Apple Silicon), and Linux
- **Configuration**: Instance-specific configurations and settings management

### Technical Features
- Clean architecture with modular design
- Async/await patterns for optimal performance  
- Comprehensive error handling and logging
- OAuth2 token storage and refresh mechanisms
- Platform-specific file system handling

