use std::path::{Path, PathBuf};

use hyped::*;
use tailwind_fuse::*;

use crate::footer::footer_section;
use crate::ui_components::{
    NavElement, ThemeColor, WaveDirection, content, page_header, space, theme, wave,
};
use crate::updates::markdown_to_html;
use crate::{BackgroundColor, HoverState, LinkStyle, SpacingSize, TextColor, TextStyle};

/// A file path wrapper for type safety
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FsFile(PathBuf);

impl FsFile {
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// A directory path wrapper for type safety
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FsDir(PathBuf);

impl FsDir {
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// Entry type returned by list_dir
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsEntry {
    File(FsFile),
    Dir(FsDir),
}

impl FsEntry {
    pub fn as_dir(&self) -> Option<&FsDir> {
        match self {
            FsEntry::Dir(d) => Some(d),
            _ => None,
        }
    }
}

/// Trait for abstracting file system operations to allow testing with in-memory implementations
pub trait FileSystem {
    /// List all entries in a directory
    fn list_dir(&self, path: &FsDir) -> Result<Vec<FsEntry>, std::io::Error>;

    /// Read file contents as string
    fn read_file(&self, file: &FsFile) -> Result<String, std::io::Error>;

    /// Safely construct an FsDir by checking if the path exists and is a directory
    fn as_dir(&self, path: impl Into<PathBuf>) -> Option<FsDir>;
}

/// Real file system implementation that uses std::fs
pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn list_dir(&self, dir: &FsDir) -> Result<Vec<FsEntry>, std::io::Error> {
        let entries = std::fs::read_dir(dir.path())?
            .filter_map(|entry| entry.ok())
            .map(|e| {
                let path = e.path();
                if path.is_dir() {
                    FsEntry::Dir(FsDir(path))
                } else {
                    FsEntry::File(FsFile(path))
                }
            })
            .collect();
        Ok(entries)
    }

    fn read_file(&self, file: &FsFile) -> Result<String, std::io::Error> {
        std::fs::read_to_string(file.path())
    }

    fn as_dir(&self, path: impl Into<PathBuf>) -> Option<FsDir> {
        let path_buf = path.into();
        if path_buf.is_dir() {
            Some(FsDir(path_buf))
        } else {
            None
        }
    }
}

/// Represents a single update entry
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateEntry {
    pub version: Version,
    pub optional_name: Option<String>,
    pub markdown_content: String,
    pub folder_path: FsDir,
}

/// Version type for proper sorting
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// Parse version from string like "1.3.9" or "1_3_9"
    pub fn parse(s: &str) -> Result<Self, String> {
        let normalized = s.replace('_', ".");
        let parts: Vec<&str> = normalized.split('.').collect();

        if parts.len() != 3 {
            return Err(format!("Invalid version format: {}", s));
        }

        let major = parts[0]
            .parse()
            .map_err(|_| format!("Invalid major version: {}", parts[0]))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| format!("Invalid minor version: {}", parts[1]))?;
        let patch = parts[2]
            .parse()
            .map_err(|_| format!("Invalid patch version: {}", parts[2]))?;

        Ok(Version {
            major,
            minor,
            patch,
        })
    }

    /// Convert version to route format (1_3_9)
    pub fn to_route_format(&self) -> String {
        format!("{}_{}_{}", self.major, self.minor, self.patch)
    }

    /// Convert version to file format (1.3.9)
    pub fn to_file_format(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Parse folder name to extract version and optional name
/// Format: "1.3.9-optional-name" or just "1.3.9"
fn parse_folder_name(folder_name: &str) -> Result<(Version, Option<String>), String> {
    let parts: Vec<&str> = folder_name.splitn(2, '-').collect();

    let version = Version::parse(parts[0])?;
    let optional_name = if parts.len() > 1 {
        Some(parts[1].to_string())
    } else {
        None
    };

    Ok((version, optional_name))
}

/// Load all update entries from the updates directory
pub fn load_updates<FS: FileSystem>(
    fs: &FS,
    updates_dir: &FsDir,
) -> Result<Vec<UpdateEntry>, String> {
    let entries = fs
        .list_dir(updates_dir)
        .map_err(|e| format!("Failed to read updates directory: {}", e))?;

    let mut updates = Vec::new();

    for entry in entries {
        // Skip if not a directory
        let entry_dir = match entry.as_dir() {
            Some(d) => d,
            None => continue,
        };

        // Get folder name
        let folder_name = match entry_dir.path().file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };

        // Parse folder name
        let (version, optional_name) = match parse_folder_name(folder_name) {
            Ok(parsed) => parsed,
            Err(_) => continue, // Skip folders that don't match the pattern
        };

        // Look for markdown file matching the version
        let markdown_filename = format!("{}.md", version.to_file_format());
        let markdown_path = entry_dir.path().join(&markdown_filename);
        let markdown_file = FsFile(markdown_path);

        // Read markdown content
        let markdown_content = match fs.read_file(&markdown_file) {
            Ok(content) => content,
            Err(_) => {
                eprintln!(
                    "Warning: No markdown file found at {:?}",
                    markdown_file.path()
                );
                continue;
            }
        };

        updates.push(UpdateEntry {
            version,
            optional_name,
            markdown_content,
            folder_path: entry_dir.clone(),
        });
    }

    // Sort by version (newest first)
    updates.sort_by(|a, b| b.version.cmp(&a.version));

    Ok(updates)
}

/// Render the update page with sidebar and markdown content
pub fn update_page(updates: &[UpdateEntry], selected: &UpdateEntry) -> Element {
    let html_content = markdown_to_html(&selected.markdown_content, selected.folder_path.path());
    let updates_list_items: Vec<Element> = updates
        .iter()
        .map(|u| {
            let is_active = u.version == selected.version;
            let link_class = if is_active {
                tw_join!(
                    TextStyle::SmallGeneralText,
                    TextColor::Primary,
                    "font-semibold"
                )
            } else {
                tw_join!(
                    TextStyle::SmallGeneralText,
                    LinkStyle {
                        color: TextColor::Subtle,
                        hover: HoverState::ColorChange
                    }
                    .to_class()
                )
            };

            let display_text = format!(
                "{}{}",
                u.version.to_file_format(),
                u.optional_name
                    .as_ref()
                    .map(|n| format!(" - {}", n))
                    .unwrap_or_default()
            );

            li(a(display_text)
                .href(&format!("/updates/{}", u.version.to_route_format()))
                .class(&link_class))
            .class("mb-2")
        })
        .collect();

    let nav_items = vec![];
    // let nav_items = vec![NavElement::new(
    //     format!("{}", selected.version.to_route_format()),
    //     format!("/updates/{}", selected.version.to_route_format()),
    // )];

    div((
        theme(
            ThemeColor::Dark,
            content((page_header(&nav_items), space(SpacingSize::Small))),
        ),
        wave(WaveDirection::Up, ThemeColor::Dark, SpacingSize::Medium),
        theme(
            ThemeColor::Light,
            content((
                space(SpacingSize::Small),
                // Two-column layout
                div((
                    // Main content area
                    div((
                        h1(format!(
                            "{}: {}",
                            selected.version.to_file_format(),
                            "Update name"
                        ))
                        .class(&tw_join!(
                            TextStyle::MainHeader,
                            TextColor::MainHeader,
                            "mb-6"
                        )),
                        div(danger(&html_content)).class("markdown-content"),
                    ))
                    .class("flex-1 md:pl-8"),
                    // Sidebar with updates list
                    div((
                        // h3("Updates").class(&tw_join!(
                        //     TextStyle::SubHeader,
                        //     TextColor::SubHeader,
                        //     "mb-4"
                        // )),
                        ul(updates_list_items).class("space-y-1"),
                    ))
                    .class("w-full md:-mr-4 md:w-64 mb-8 md:mb-0 md:pl-4"),
                ))
                .class("flex flex-col md:flex-row"),
                space(SpacingSize::Large),
            )),
        ),
        // Footer section
        footer_section(),
    ))
    .class(&tw_join!(
        "flex flex-col",
        BackgroundColor::Default.as_class()
    ))
}

#[cfg(test)]
mod virtual_fs {
    use std::collections::HashMap;

    use super::*;

    /// Virtual file for DSL
    #[derive(Debug, Clone)]
    pub struct VFile {
        pub name: String,
        pub content: String,
    }

    /// Virtual directory for DSL
    #[derive(Debug, Clone)]
    pub struct VDir {
        pub name: String,
        pub items: Vec<VItem>,
    }

    /// Virtual item (file or directory)
    #[derive(Debug, Clone)]
    pub enum VItem {
        File(VFile),
        Dir(VDir),
    }

    /// Create a virtual file
    pub fn file(name: impl Into<String>, content: impl Into<String>) -> VItem {
        VItem::File(VFile {
            name: name.into(),
            content: content.into(),
        })
    }

    /// Create a virtual directory
    pub fn dir(name: impl Into<String>, items: impl IntoIterator<Item = VItem>) -> VItem {
        VItem::Dir(VDir {
            name: name.into(),
            items: items.into_iter().collect(),
        })
    }

    /// Create a root virtual directory (unnamed)
    pub fn root(items: impl IntoIterator<Item = VItem>) -> VDir {
        VDir {
            name: String::new(),
            items: items.into_iter().collect(),
        }
    }

    /// In-memory file system implementation for testing
    #[allow(dead_code)]
    #[derive(Debug, Clone)]
    pub struct InMemoryFileSystem {
        files: HashMap<PathBuf, String>,
    }

    impl InMemoryFileSystem {
        /// Create a file system from a virtual directory structure
        pub fn from_structure(structure: VDir) -> Self {
            let mut fs = Self {
                files: Default::default(),
            };
            fs.add_structure(PathBuf::new(), structure);
            fs
        }

        fn add_structure(&mut self, base_path: PathBuf, dir: VDir) {
            for item in dir.items {
                match item {
                    VItem::File(vfile) => {
                        let file_path = base_path.join(&vfile.name);
                        {
                            let this = &mut *self;
                            let content = vfile.content;
                            this.files.insert(file_path.into(), content);
                        };
                    }
                    VItem::Dir(vdir) => {
                        let dir_path = base_path.join(&vdir.name);
                        self.add_structure(dir_path, vdir);
                    }
                }
            }
        }

        /// Check if a path is a directory in the in-memory file system
        fn is_dir(&self, path: &Path) -> bool {
            // A path is a directory if it has child files
            self.files.keys().any(|file_path| {
                if let Some(parent) = file_path.parent() {
                    parent == path || parent.starts_with(path)
                } else {
                    false
                }
            })
        }
    }

    impl FileSystem for InMemoryFileSystem {
        fn list_dir(&self, dir: &FsDir) -> Result<Vec<FsEntry>, std::io::Error> {
            let path = dir.path();

            // Find all paths that are direct children of the given path
            let files: Vec<PathBuf> = self
                .files
                .keys()
                .filter(|file_path| {
                    if let Some(parent) = file_path.parent() {
                        parent == path
                    } else {
                        false
                    }
                })
                .cloned()
                .collect();

            // Also find directories (paths that have children)
            let mut dirs: Vec<PathBuf> = self
                .files
                .keys()
                .filter_map(|file_path| {
                    // Get all ancestors
                    let mut ancestors = vec![];
                    let mut current = file_path.as_path();
                    while let Some(parent) = current.parent() {
                        if parent == Path::new("") {
                            break;
                        }
                        ancestors.push(parent.to_path_buf());
                        current = parent;
                    }

                    // Find direct children of path
                    ancestors.into_iter().find(|ancestor| {
                        if let Some(parent) = ancestor.parent() {
                            parent == path
                        } else {
                            false
                        }
                    })
                })
                .collect();

            // Remove duplicates
            dirs.sort();
            dirs.dedup();

            if files.is_empty()
                && dirs.is_empty()
                && !self.files.keys().any(|p| p.starts_with(path))
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Directory not found: {:?}", path),
                ));
            }

            let mut entries: Vec<FsEntry> = Vec::new();
            entries.extend(dirs.into_iter().map(|p| FsEntry::Dir(FsDir(p))));
            entries.extend(files.into_iter().map(|p| FsEntry::File(FsFile(p))));
            entries.sort_by_key(|e| match e {
                FsEntry::File(f) => f.path().to_path_buf(),
                FsEntry::Dir(d) => d.path().to_path_buf(),
            });

            Ok(entries)
        }

        fn read_file(&self, file: &FsFile) -> Result<String, std::io::Error> {
            self.files.get(file.path()).cloned().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("File not found: {:?}", file.path()),
                )
            })
        }

        fn as_dir(&self, path: impl Into<PathBuf>) -> Option<FsDir> {
            let path_buf = path.into();
            if self.is_dir(&path_buf) {
                Some(FsDir(path_buf))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use virtual_fs::*;

    #[test]
    fn test_version_parse() {
        let v = Version::parse("1.3.9").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 3);
        assert_eq!(v.patch, 9);

        let v2 = Version::parse("1_3_9").unwrap();
        assert_eq!(v2, v);
    }

    #[test]
    fn test_version_formats() {
        let v = Version {
            major: 1,
            minor: 3,
            patch: 9,
        };
        assert_eq!(v.to_route_format(), "1_3_9");
        assert_eq!(v.to_file_format(), "1.3.9");
    }

    #[test]
    fn test_version_sorting() {
        let mut versions = vec![
            Version {
                major: 1,
                minor: 3,
                patch: 9,
            },
            Version {
                major: 1,
                minor: 4,
                patch: 0,
            },
            Version {
                major: 1,
                minor: 3,
                patch: 10,
            },
        ];
        versions.sort();

        assert_eq!(
            versions[0],
            Version {
                major: 1,
                minor: 3,
                patch: 9
            }
        );
        assert_eq!(
            versions[1],
            Version {
                major: 1,
                minor: 3,
                patch: 10
            }
        );
        assert_eq!(
            versions[2],
            Version {
                major: 1,
                minor: 4,
                patch: 0
            }
        );
    }

    #[test]
    fn test_parse_folder_name() {
        let (version, name) = parse_folder_name("1.3.9-bug-fixes").unwrap();
        assert_eq!(version.to_file_format(), "1.3.9");
        assert_eq!(name, Some("bug-fixes".to_string()));

        let (version2, name2) = parse_folder_name("1.4.0").unwrap();
        assert_eq!(version2.to_file_format(), "1.4.0");
        assert_eq!(name2, None);
    }

    #[test]
    fn test_as_dir() {
        let structure = root([dir(
            "updates",
            [dir("1.3.9-test", [file("1.3.9.md", "# Test")])],
        )]);

        let fs = InMemoryFileSystem::from_structure(structure);

        // Valid directory
        let updates_dir = fs.as_dir("updates");
        assert!(updates_dir.is_some());
        assert_eq!(updates_dir.unwrap().path(), Path::new("updates"));

        // Invalid path (file, not directory)
        let file_as_dir = fs.as_dir("updates/1.3.9-test/1.3.9.md");
        assert!(file_as_dir.is_none());

        // Non-existent path
        let nonexistent = fs.as_dir("nonexistent");
        assert!(nonexistent.is_none());
    }

    #[test]
    fn test_vfs_dsl() {
        let structure = root([dir(
            "updates",
            [dir(
                "1.3.9-test",
                [file("1.3.9.md", "# Test Update\n\nThis is a test.")],
            )],
        )]);

        let fs = InMemoryFileSystem::from_structure(structure);

        let file = FsFile(PathBuf::from("updates/1.3.9-test/1.3.9.md"));
        let content = fs.read_file(&file).unwrap();
        assert_eq!(content, "# Test Update\n\nThis is a test.");

        let dir = fs.as_dir("updates").unwrap();
        let entries = fs.list_dir(&dir).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(matches!(entries[0], FsEntry::Dir(_)));
        assert_eq!(
            entries[0].as_dir().unwrap().path(),
            Path::new("updates/1.3.9-test")
        );
    }

    #[test]
    fn test_load_updates_with_dsl() {
        let structure = root([dir(
            "updates",
            [
                dir(
                    "1.3.9-bug-fixes",
                    [file("1.3.9.md", "# Version 1.3.9\n\nBug fixes")],
                ),
                dir(
                    "1.4.0-new-features",
                    [file("1.4.0.md", "# Version 1.4.0\n\nNew features")],
                ),
            ],
        )]);

        let fs = InMemoryFileSystem::from_structure(structure);
        let updates_dir = fs.as_dir("updates").unwrap();
        let updates = load_updates(&fs, &updates_dir).unwrap();

        assert_eq!(updates.len(), 2);

        // Should be sorted newest first
        assert_eq!(updates[0].version.to_file_format(), "1.4.0");
        assert_eq!(updates[0].optional_name, Some("new-features".to_string()));
        assert!(updates[0].markdown_content.contains("New features"));

        assert_eq!(updates[1].version.to_file_format(), "1.3.9");
        assert_eq!(updates[1].optional_name, Some("bug-fixes".to_string()));
        assert!(updates[1].markdown_content.contains("Bug fixes"));
    }

    #[test]
    fn test_load_updates_skips_invalid_folders() {
        let structure = root([dir(
            "updates",
            [
                dir("1.3.9-valid", [file("1.3.9.md", "# Valid")]),
                dir("invalid-folder", [file("test.md", "# Invalid")]),
            ],
        )]);

        let fs = InMemoryFileSystem::from_structure(structure);
        let updates_dir = fs.as_dir("updates").unwrap();
        let updates = load_updates(&fs, &updates_dir).unwrap();

        // Should only load the valid entry
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].version.to_file_format(), "1.3.9");
    }

    #[test]
    fn test_load_updates_with_multiple_versions() {
        let structure = root([dir(
            "updates",
            [
                dir("1.3.9-bug-fixes", [file("1.3.9.md", "# 1.3.9")]),
                dir("1.4.0", [file("1.4.0.md", "# 1.4.0")]),
                dir("1.3.10-hotfix", [file("1.3.10.md", "# 1.3.10")]),
            ],
        )]);

        let fs = InMemoryFileSystem::from_structure(structure);
        let updates_dir = fs.as_dir("updates").unwrap();
        let updates = load_updates(&fs, &updates_dir).unwrap();

        assert_eq!(updates.len(), 3);

        // Should be sorted newest first: 1.4.0, 1.3.10, 1.3.9
        assert_eq!(updates[0].version.to_file_format(), "1.4.0");
        assert_eq!(updates[1].version.to_file_format(), "1.3.10");
        assert_eq!(updates[2].version.to_file_format(), "1.3.9");
    }
}
