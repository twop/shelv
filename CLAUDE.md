# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Build and Run
- `cargo run` - Run the application
- `cargo check` - Check code for errors (preferred workflow for development)

### Testing
- Tests should be run using standard Rust tooling: `cargo test`

## Architecture Overview

Shelv is a Rust-based note-taking application built with egui for the GUI framework. The application is designed as a hackable playground for ephemeral thoughts with live code execution capabilities.

### Core Components

#### Application Structure
- `main.rs` - Entry point with eframe setup, global hotkeys, tray icon management, and file watching
- `app_state.rs` - Central state management including notes, UI state, and command handling
- `app_ui.rs` - UI rendering logic
- `app_actions.rs` - Action processing and state mutations
- `app_io.rs` - I/O operations and external system interactions

#### Text Processing & Commands
- `text_structure.rs` - Text parsing and structure analysis for markdown-like content
- `commands/` - Keyboard command handlers for various text editing operations:
  - List item navigation (tab/shift-tab, enter)
  - Markdown heading toggling
  - Code block toggling
  - LLM integration commands
  - Slash palette for command discovery

#### Scripting System
- `scripting/` - JavaScript runtime integration using Boa engine
  - `note_eval.rs` - JavaScript code block evaluation within notes
  - `settings_eval.rs` - Settings configuration via JavaScript
  - `note_eval_context.rs` - Context for script execution

#### Core Features
- **Live Code Blocks**: JavaScript code can be executed directly within notes
- **LLM Integration**: Built-in AI assistance for text editing and generation
- **Global Hotkeys**: System-wide keyboard shortcuts for quick access
- **File Watching**: Automatic reloading of notes when files change externally
- **Tray Integration**: System tray icon for quick show/hide

### Key Dependencies
- `egui` + `eframe` - GUI framework
- `boa_engine` - JavaScript runtime for code block execution
- `pulldown-cmark` - Markdown parsing
- `syntect` - Syntax highlighting
- `global-hotkey` - System-wide keyboard shortcuts
- `tray-icon` - System tray functionality
- `genai` - LLM integration
- `hotwatch` - File system monitoring

### Data Flow
1. User input is captured through egui
2. Commands are processed through the command system
3. Text changes update the text structure
4. State changes trigger UI re-rendering
5. Persistent state is saved to disk automatically

The application uses a command-action pattern where user inputs are converted to commands, which generate actions that mutate the application state.