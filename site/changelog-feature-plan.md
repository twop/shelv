# Changelog Website Feature

## Requirements

### File System Structure
- [ ] File system based entries for updates
- [ ] File pattern: `1.3.9-{optional name}` folder format
- [ ] Each folder contains:
  - [ ] Markdown file with matching version name (e.g., `1.3.9.md` inside `1.3.9-{optional-name}/`)
  - [ ] Optional media resources (screenshots, videos) in the same folder
- [ ] Updates log entries location: `/site/updates` folder
- [ ] Markdown files include frontmatter with creation date
- [ ] Example structure:
  ```
  /site/updates/
    1.3.9-bug-fixes/
      1.3.9.md
      screenshot.png
    1.4.0-new-features/
      1.4.0.md
      demo.webm
  ```

### Routing
- [ ] New route for specific update: `updates/{version}` (e.g., `updates/1_3_9`)
  - Note: Only version number passed to match route, underscores replace dots
- [ ] New route for updates list: `/updates` (renders latest by default)

### UI Components
- [ ] Updates list on the left side
- [ ] Markdown content converted to HTML on the right side
- [ ] Extract reusable components from main page (overall shell)
- [ ] Server-side rendering only (no client interactivity needed)
- [ ] Style markdown-to-HTML output appropriately

### Technical Details
- [ ] Use existing `pulldown-cmark` library for:
  - Markdown parsing
  - HTML rendering
- [ ] At site startup (`site/main.rs`):
  - [ ] Scan for all update entries
  - [ ] Structure entries in list sorted by version
  - [ ] Keep raw markdown in memory
- [ ] Resolve relative image paths in markdown (relative to markdown file)

## Execution Plan

### Phase 1: Basic Routing and File Structure ✅
- [x] Add route for specific update view: `/updates/{version}`
- [x] Add route for updates list view: `/updates`
- [x] Create new file for updates logic and UI rendering

### Phase 2: Content Loading ✅
- [x] Create `FileSystem` trait for abstracting file operations
  - [x] `list_dir(&FsDir) -> Result<Vec<FsEntry>>` - list directory contents
  - [x] `read_file(&FsFile) -> Result<String>` - read file contents
  - [x] `as_dir(PathBuf) -> Option<FsDir>` - safely construct FsDir
- [x] Implement type-safe wrappers:
  - [x] `FsFile` - wrapper for file paths
  - [x] `FsDir` - wrapper for directory paths
  - [x] `FsEntry` - enum for File/Dir with helper methods
- [x] Implement `RealFileSystem` for production (uses `std::fs`)
- [x] Implement `InMemoryFileSystem` for testing
- [x] Create DSL for test file structures:
  - [x] `root(items)` - create root virtual directory
  - [x] `dir(name, items)` - create virtual directory
  - [x] `file(name, content)` - create virtual file
- [x] Implement `load_updates()` function:
  - [x] Find all update entries in `/site/updates` folder
  - [x] Parse version numbers from folder names (format: `1.3.9-optional-name`)
  - [x] Sort entries by version (newest first)
  - [x] Store raw markdown in memory
- [x] Write comprehensive tests (14 tests, all passing)
- [x] Wire up startup function in `site/main.rs` to load updates at server start
- [x] Store loaded updates in application state
- [x] Update route handlers to use loaded data

**Implementation Notes:**
- Virtual file system allows testing without disk I/O
- Type-safe wrappers prevent mixing files and directories
- DSL makes test setup clean and readable
- `Version` struct implements `Ord` for proper semantic versioning sort

### Phase 3: Bare Bones Rendering
- [ ] Implement completely unstyled rendering for both routes
- [ ] For content: dump raw markdown (no HTML parsing yet)
- [ ] Verify routing and data loading works

### Phase 4: Test Content
- [ ] Create test update entry for version 1.4.0:
  - [ ] Create folder: `1.4.0-{optional-name}`
  - [ ] Add markdown file with frontmatter
  - [ ] Add test image asset
  - [ ] Verify file structure works

### Phase 5: Markdown to HTML Conversion
- [ ] Implement markdown to HTML conversion using `pulldown-cmark`
- [ ] Handle image resource loading:
  - [ ] Resolve relative paths to markdown file location
  - [ ] Ensure images load correctly in rendered HTML
- [ ] Keep unstyled for now

### Phase 6: Styling
- [ ] Style HTML output using current theme
- [ ] Apply consistent styling with main page
- [ ] Style markdown elements (headings, lists, code blocks, etc.)

## Notes
- Pure server-side rendering
- HTML is rendered using the `hyped` library (https://github.com/swlkr/hyped)
- Use existing `pulldown-cmark` dependency for markdown parsing
- Maintain consistency with main page styling
- Image paths are relative to markdown files
