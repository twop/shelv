# Changelog

All notable changes to Shelv will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Word Jump Navigation**: New quick navigation feature inspired by Vim/Helix editors
  - Press `⌘ J` to activate Word Jump mode
  - Visual letter combinations appear next to words throughout the note
  - Type 2-character sequences to instantly jump to any word
  - Accessible via keyboard shortcut (`⌘ J` by default) and slash palette (`/jump`)

### Changed
- **Command System Rework**: Overhaul of the application's command system ([#187](https://github.com/twop/shelv/pull/187))
  - After egui update to 32 my hacky hotkey system stopped working, hence needed to pay the debt there
  - Added conditions to commands using bool algebra and attribute checking
  - Introduced raw input hook phase to handle keybinding conflicts
  - Made frame hotkeys more ergonomic and unified with Commands system
  - Built debugger UI for inspecting current state and action logs
- Updated system prompt to include Word Jump command documentation
- Added tutorial for Word Jump Navigation
- Improved README with dedicated Word Jump section and usage examples

---

## [1.3.9] - 2025-09-26

### Added
- **Selection Argument Support**: JavaScript functions in `InsertText` commands can now receive selected text as an argument
  - Use `selection` child node in KDL configuration to pass selected text to JS functions
  - Enables custom text transformation commands (e.g., wrap in quotes, make uppercase)
  - Updated system prompt and README with examples and documentation

### Fixed
- Discord invite link in documentation

