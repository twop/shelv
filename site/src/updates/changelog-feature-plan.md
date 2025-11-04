# Changelog Website Feature

## Status: ✅ COMPLETED

All phases completed successfully. The updates/changelog feature is fully functional with styling matching the main site.

## Requirements

### File System Structure
- [x] File system based entries for updates
- [x] File pattern: `1.3.9-{optional name}` folder format
- [x] Each folder contains:
  - [x] Markdown file with matching version name (e.g., `1.3.9.md` inside `1.3.9-{optional-name}/`)
  - [x] Optional media resources (screenshots, videos) in the same folder
- [x] Updates log entries location: `/site/updates` folder
- [x] Markdown files include frontmatter with creation date
- [x] Example structure:
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
- [x] New route for specific update: `updates/{version}` (e.g., `updates/1_3_9`)
  - Note: Only version number passed to match route, underscores replace dots
- [x] New route for updates list: `/updates` (renders latest by default)

### UI Components
- [x] Updates list on the left side
- [x] Markdown content converted to HTML on the right side
- [x] Extract reusable components from main page (overall shell)
- [x] Server-side rendering only (no client interactivity needed)
- [x] Style markdown-to-HTML output appropriately

### Technical Details
- [x] Use existing `pulldown-cmark` library for:
  - Markdown parsing
  - HTML rendering
- [x] At site startup (`site/main.rs`):
  - [x] Scan for all update entries
  - [x] Structure entries in list sorted by version
  - [x] Keep raw markdown in memory
- [x] Resolve relative image paths in markdown (relative to markdown file)

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

### Phase 3: Bare Bones Rendering ✅
- [x] Implement redirect from `/updates` to latest update (e.g., `/updates/1_4_0`)
- [x] Create updates page layout with two-column design:
  - [x] Left sidebar: list of all updates (links)
  - [x] Right/main area: current update content
- [x] Render updates list in left sidebar:
  - [x] Each update as a clickable link to `/updates/{version}`
  - [x] Display version number and optional name
  - [x] Highlight the currently active/selected update (using "[ACTIVE]" marker)
- [x] Display raw markdown content in main area (no HTML parsing yet)
- [x] Completely unstyled for now (bare bones HTML with inline styles)
- [x] Verify routing, redirects, and data loading works

**Implementation Notes:**
- `/updates` route now redirects to latest update instead of rendering a separate list page
- Two-column layout uses flexbox with fixed-width sidebar (300px)
- Active update is highlighted with "[ACTIVE]" text marker
- Raw markdown displayed in `<pre>` tags for now

### Phase 4: Test Content ✅
- [x] Create test update entry for version 1.4.0:
  - [x] Create folder: `1.4.0-word-jump-mode`
  - [x] Add markdown file with frontmatter (based on unreleased CHANGELOG.md)
  - [x] Add test image asset (screenshot.png)
  - [x] Verify file structure works
- [x] Create additional test entry for version 1.3.9:
  - [x] Create folder: `1.3.9-selection-support`
  - [x] Add markdown file with frontmatter (based on 1.3.9 CHANGELOG.md)
  - [x] Add test image asset

**Implementation Notes:**
- Created two test entries with real content from CHANGELOG.md
- Both entries include proper frontmatter (date, title)
- Images copied from site/assets/media/ folder
- Structure follows the required pattern: `version-name/version.md`

### Phase 5: Markdown to HTML Conversion ✅
- [x] Implement markdown to HTML conversion using `pulldown-cmark`
- [x] Handle image resource loading:
  - [x] Resolve relative paths to markdown file location
  - [x] Ensure images load correctly in rendered HTML
- [x] Keep unstyled for now

**Implementation Notes:**
- Created `markdown_to_html.rs` with custom event processor
- Intercepts image tags and transforms relative paths to absolute web paths
- Added static file serving for `/update-log` directory
- Image path resolution: `screenshot.png` → `/update-log/1.4.0-word-jump-mode/screenshot.png`
- All tests passing (21 tests including 3 new image path resolution tests)

### Phase 6: Styling ✅
- [x] Style HTML output using current theme
- [x] Apply consistent styling with main page
- [x] Style markdown elements (headings, lists, code blocks, etc.)

**Implementation Notes:**
- Added comprehensive CSS styling in `app.css` using `@layer components`
- Created scoped `.markdown-content` class for all markdown elements
- Matched typography from home page: smaller text (`text-sm`), darker color (`nord4-darker`)
- Headers remain bright (`nord6`) for proper visual hierarchy
- Styled all elements: headings, paragraphs, lists, code blocks, links, images, blockquotes, tables
- Used Nord color palette and Tailwind CSS variables for consistency
- Responsive line heights: `leading-6` on mobile, `leading-7` on desktop

### Phase 7: Code Refactoring ✅
- [x] Move `update_page()` render function from `main.rs` to `updates.rs`
- [x] Refactor route handler to pass data slices instead of HTML elements
- [x] Extract reusable `page_header` component to `ui_components.rs`
- [x] Create `NavElement` struct for flexible navigation
- [x] Update both `home.rs` and `updates.rs` to use shared components
- [x] Remove duplicate header/logo/icon functions

**Implementation Notes:**
- Better separation of concerns: route handlers fetch data, render functions build UI
- Single source of truth for page header across all pages
- Each page can specify its own navigation items via `NavElement` slice
- Removed ~150 lines of duplicate code

## Notes
- Pure server-side rendering
- HTML is rendered using the `hyped` library (https://github.com/swlkr/hyped)
- Use existing `pulldown-cmark` dependency for markdown parsing
- Maintain consistency with main page styling
- Image paths are relative to markdown files
