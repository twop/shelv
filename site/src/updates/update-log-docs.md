# Update Log Documentation

## Overview

The Shelv website has an updates/changelog feature that displays release notes in a clean, two-column layout. Each update is stored as a markdown file with optional media assets.

## How to Add a New Update

### 1. Create a Folder

Create a new folder in `update-log/` following this naming pattern:

```
{version}-{optional-name}/
```

**Examples:**
- `1.4.0-word-jump-mode/`
- `1.3.9-selection-support/`
- `2.0.0/` (name is optional)

**Important:** The version number must be in semantic versioning format: `MAJOR.MINOR.PATCH`

### 2. Create the Markdown File

Inside the folder, create a markdown file named exactly `{version}.md` (matching the version in the folder name).

**Example:** For folder `1.4.0-word-jump-mode/`, create file `1.4.0.md`

### 3. Add Frontmatter

**Every markdown file must start with a frontmatter block** containing the date and title:

```markdown
---
date: 2025-01-15
title: "Word Jump Mode"
---

# Version 1.4.0 - Word Jump Mode

Your content starts here...
```

**Frontmatter fields:**
- `date` (required): Release date in `YYYY-MM-DD` format
- `title` (required): Short title for the update (can match the optional folder name)

### 4. Write Your Content

After the frontmatter, write your update content in standard markdown format. You can use:

- **Headings** (`#`, `##`, `###`, etc.)
- **Paragraphs** (regular text)
- **Lists** (ordered and unordered)
- **Code blocks** (with syntax highlighting)
- **Links** (`[text](url)`)
- **Images** (`![alt text](image.png)`)
- **Bold** and *italic* text
- **Blockquotes** (`>`)
- **Tables**
- **Horizontal rules** (`---`)

**Complete Example:**

```markdown
---
date: 2025-01-15
title: "Word Jump Mode"
---

# Version 1.4.0 - Word Jump Mode

Released on January 15, 2025

## New Features

### Word Jump Mode

Jump to any word on screen with just a couple of keystrokes! Similar to Vimium for browsers.

![Word Jump Demo](screenshot.png)

**How it works:**
1. Press `Cmd+J` to activate word jump mode
2. Type the 2-letter code shown next to the word
3. Your cursor instantly jumps to that location

## Bug Fixes

- Fixed crash when opening large files
- Improved markdown rendering performance
- Fixed image loading issues

## Technical Details

Updated dependencies:
- `egui` to version 0.29
- `pulldown-cmark` to 0.12
```

### 5. Add Media Assets (Optional)

You can include images, screenshots, or videos in the same folder:

```
1.4.0-word-jump-mode/
├── 1.4.0.md
├── screenshot.png
├── demo.webm
└── feature-preview.jpg
```

**Important:** Reference media files using relative paths in your markdown:

```markdown
![Feature Screenshot](screenshot.png)
![Another Image](demo-image.jpg)
```

The system will automatically resolve these paths when rendering.

## File Structure Example

```
update-log/
├── 1.4.0-word-jump-mode/
│   ├── 1.4.0.md          (with frontmatter)
│   └── screenshot.png
├── 1.3.9-selection-support/
│   ├── 1.3.9.md          (with frontmatter)
│   └── demo.webm
└── 1.3.8-bug-fixes/
    └── 1.3.8.md          (with frontmatter)
```

## Frontmatter Template

Copy and paste this template at the start of every update markdown file:

```markdown
---
date: YYYY-MM-DD
title: "Your Update Title"
---
```

**Example with real data:**

```markdown
---
date: 2025-01-15
title: "Word Jump Mode & Bug Fixes"
---
```

## Viewing Updates

### On the Website

- **Latest update:** Visit `/updates` (automatically shows the newest update)
- **Specific update:** Visit `/updates/1_4_0` (note: dots replaced with underscores)
- **Browse all updates:** Use the sidebar on any update page

### Update List

The sidebar shows all updates sorted by version (newest first), with:
- Version number
- Optional name (if provided)
- Active highlight for current update

## How It Works Behind the Scenes

When the server starts:
1. Scans the `update-log/` directory
2. Parses folder names to extract versions and names
3. Loads markdown content (including frontmatter) into memory
4. Sorts updates by semantic version (newest first)
5. Serves updates via HTTP routes

When rendering:
1. Strips frontmatter from markdown before rendering
2. Converts markdown to HTML using `pulldown-cmark`
3. Resolves relative image paths to absolute web paths
4. Applies Nord-themed styling via CSS
5. Displays in a two-column layout (sidebar + content)

## Styling Notes

All markdown content is automatically styled to match the main site:

- **Headings:** Bright (`nord6`), larger sizes
- **Body text:** Subtle (`nord4-darker`), smaller size (`text-sm`)
- **Code blocks:** Dark background with syntax highlighting
- **Links:** Blue (`nord8`), underlined, hover effects
- **Images:** Rounded corners with subtle borders
- **Lists:** Proper indentation and spacing

No manual styling is needed in your markdown files.

## Tips

1. **Version numbers:** Must follow semantic versioning (e.g., `1.2.3`)
2. **File names:** Must match exactly: folder `1.4.0-name/` → file `1.4.0.md`
3. **Frontmatter:** Required at the top of every markdown file
4. **Date format:** Use `YYYY-MM-DD` (e.g., `2025-01-15`)
5. **Images:** Use relative paths, the system handles the rest
6. **Markdown:** Standard CommonMark with GitHub-flavored extensions
7. **Order:** Updates are automatically sorted by version number

## Common Mistakes to Avoid

❌ **Don't:** Name folder `v1.4.0-name` (no `v` prefix)  
✅ **Do:** Name folder `1.4.0-name`

❌ **Don't:** Create file `update.md` or `changelog.md`  
✅ **Do:** Create file matching version: `1.4.0.md`

❌ **Don't:** Forget the frontmatter block  
✅ **Do:** Always start with `---` frontmatter `---`

❌ **Don't:** Use wrong date format: `01/15/2025` or `15-Jan-2025`  
✅ **Do:** Use ISO format: `2025-01-15`

❌ **Don't:** Use absolute paths for images: `/assets/image.png`  
✅ **Do:** Use relative paths: `image.png` or `./screenshots/image.png`

❌ **Don't:** Manually add HTML styling  
✅ **Do:** Use standard markdown, styling is automatic

## Quick Checklist

- [ ] Created folder with format `{version}-{optional-name}/`
- [ ] Created markdown file named `{version}.md`
- [ ] Added frontmatter block with `date` and `title`
- [ ] Date is in `YYYY-MM-DD` format
- [ ] Wrote content using standard markdown
- [ ] Added images/media to the same folder (if needed)
- [ ] Used relative paths for all media references
- [ ] Verified version number follows semantic versioning

That's it! The update will automatically appear on the website when the server restarts.
