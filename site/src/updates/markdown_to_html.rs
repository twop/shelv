use pulldown_cmark::{CowStr, Event, MetadataBlockKind, Options, Parser, Tag, TagEnd, html};
use serde::Deserialize;
use std::path::Path;

/// Metadata extracted from YAML frontmatter
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Metadata {
    pub date: String,
    pub title: String,
}

/// Converts markdown content to HTML with relative image path resolution and frontmatter parsing
///
/// This function uses pulldown_cmark to parse markdown and render it as HTML.
/// Relative image paths are resolved to absolute web paths based on the base_path.
///
/// # Arguments
/// * `markdown` - The markdown content to convert
/// * `base_path` - The base path for resolving relative image URLs (e.g., "update-log/1.4.0-word-jump-mode")
///
/// # Returns
/// A tuple of (HTML string, Option<Metadata>). Metadata is None if parsing fails or frontmatter is missing.
pub fn markdown_to_html(markdown: &str, base_path: &Path) -> (String, Option<Metadata>) {
    let options = Options::ENABLE_YAML_STYLE_METADATA_BLOCKS | Options::all();
    let parser = Parser::new_ext(markdown, options);

    let mut metadata: Option<Metadata> = None;
    let mut events = Vec::new();

    let mut inside_yaml_meta = false;
    let mut video_dest_url = None;
    // Collect events and extract metadata
    for event in parser {
        match event {
            Event::Start(Tag::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                inside_yaml_meta = true;
            }
            Event::End(TagEnd::MetadataBlock(MetadataBlockKind::YamlStyle)) => {
                inside_yaml_meta = false;
            }
            Event::Text(ref text) if metadata.is_none() && inside_yaml_meta => {
                if let Ok(parsed) = serde_yaml_ng::from_str::<Metadata>(text.as_ref()) {
                    metadata = Some(parsed);
                }
                // Don't include metadata text in events
            }

            Event::Text(_) if video_dest_url.is_some() => {
                // we can't meaningfully do anything about the name of the video, due to <video> element's lack of `alt` attribute
            }

            Event::Start(Tag::Image {
                link_type: _,
                dest_url,
                title: _,
                id: _,
            }) if is_video_file(&dest_url) => {
                video_dest_url = Some(dest_url.to_string());
            }

            Event::End(TagEnd::Image) if video_dest_url.is_some() => {
                if let Some(dest_url) = video_dest_url.take() {
                    // note that it will always be successful, but still don't want to  unwrap here
                    let resolved_url = resolve_image_path(&dest_url, base_path);

                    let video_html = format!(
                        "<video controls autoplay loop muted>\n  <source src=\"{}\" type=\"{}\">\n  Your browser does not support the video tag.\n</video>",
                        resolved_url,
                        get_video_mime_type(&dest_url)
                    );

                    events.push(Event::Html(video_html.into()));
                }
            }

            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                events.push(Event::Start(Tag::Image {
                    link_type,
                    dest_url: resolve_image_path(&dest_url, base_path),
                    title,
                    id,
                }));
            }
            _ => {
                events.push(event);
            }
        }
    }

    let mut html_output = String::new();
    html::push_html(&mut html_output, events.into_iter());

    (html_output, metadata)
}

/// Resolves a potentially relative image path to an absolute web path
///
/// # Arguments
/// * `url` - The image URL from the markdown (may be relative or absolute)
/// * `base_path` - The base path for the markdown file's directory
///
/// # Returns
/// An absolute web path if the URL was relative, otherwise the original URL
fn resolve_image_path(url: &str, base_path: &Path) -> CowStr<'static> {
    // Check if URL is already absolute (http://, https://, or starts with /)
    if url.starts_with("http://") || url.starts_with("https://") || url.starts_with('/') {
        // Already absolute, return as-is
        url.to_string().into()
    } else {
        // Relative path - resolve to web-accessible path
        // Convert base_path to string and combine with relative URL
        let web_path = format!("/{}/{}", base_path.display(), url);
        web_path.into()
    }
}

/// Checks if the given URL points to a video file based on its extension
fn is_video_file(url: &str) -> bool {
    let video_extensions = [".mp4", ".webm", ".ogg", ".mov", ".avi", ".mkv"];
    let url_lower = url.to_lowercase();
    video_extensions.iter().any(|ext| url_lower.ends_with(ext))
}

/// Returns the appropriate MIME type for a video file based on its extension
fn get_video_mime_type(url: &str) -> &'static str {
    let url_lower = url.to_lowercase();
    match url_lower {
        _ if url_lower.ends_with(".mp4") => "video/mp4",
        _ if url_lower.ends_with(".webm") => "video/webm",
        _ if url_lower.ends_with(".ogg") => "video/ogg",
        _ if url_lower.ends_with(".mov") => "video/quicktime",
        _ if url_lower.ends_with(".avi") => "video/x-msvideo",
        _ if url_lower.ends_with(".mkv") => "video/x-matroska",
        _ => "video/mp4", // Default fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_markdown() {
        let markdown = "# Hello World\n\nThis is a paragraph.";
        let base_path = Path::new("update-log/1.4.0-test");
        let (html, metadata) = markdown_to_html(markdown, base_path);

        assert!(html.contains("<h1>"));
        assert!(html.contains("Hello World"));
        assert!(html.contains("<p>"));
        assert!(html.contains("This is a paragraph."));
        assert!(metadata.is_none());
    }

    #[test]
    fn test_code_blocks() {
        let markdown = "```rust\nfn main() {}\n```";
        let base_path = Path::new("update-log/1.4.0-test");
        let (html, _) = markdown_to_html(markdown, base_path);

        assert!(html.contains("<code"));
        assert!(html.contains("fn main()"));
    }

    #[test]
    fn test_lists() {
        let markdown = "- Item 1\n- Item 2\n- Item 3";
        let base_path = Path::new("update-log/1.4.0-test");
        let (html, _) = markdown_to_html(markdown, base_path);

        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>"));
        assert!(html.contains("Item 1"));
    }

    #[test]
    fn test_links() {
        let markdown = "[GitHub](https://github.com)";
        let base_path = Path::new("update-log/1.4.0-test");
        let (html, _) = markdown_to_html(markdown, base_path);

        assert!(html.contains("<a "));
        assert!(html.contains("href=\"https://github.com\""));
        assert!(html.contains("GitHub"));
    }

    #[test]
    fn test_relative_image_path() {
        let markdown = "![Alt text](screenshot.png)";
        let base_path = Path::new("update-log/1.4.0-test");
        let (html, _) = markdown_to_html(markdown, base_path);

        assert!(html.contains("<img "));
        assert!(html.contains("src=\"/update-log/1.4.0-test/screenshot.png\""));
        assert!(html.contains("alt=\"Alt text\""));
    }

    #[test]
    fn test_absolute_image_path() {
        let markdown = "![Alt text](/assets/image.png)";
        let base_path = Path::new("update-log/1.4.0-test");
        let (html, _) = markdown_to_html(markdown, base_path);

        assert!(html.contains("<img "));
        assert!(html.contains("src=\"/assets/image.png\""));
        assert!(html.contains("alt=\"Alt text\""));
    }

    #[test]
    fn test_http_image_path() {
        let markdown = "![Alt text](https://example.com/image.png)";
        let base_path = Path::new("update-log/1.4.0-test");
        let (html, _) = markdown_to_html(markdown, base_path);

        assert!(html.contains("<img "));
        assert!(html.contains("src=\"https://example.com/image.png\""));
        assert!(html.contains("alt=\"Alt text\""));
    }

    #[test]
    fn test_frontmatter_parsing() {
        let markdown = r#"---
date: 2025-01-15
title: "Word Jump Mode"
---

# Version 1.4.0

This is the content."#;
        let base_path = Path::new("update-log/1.4.0-test");
        let (html, metadata) = markdown_to_html(markdown, base_path);

        assert!(html.contains("<h1>"));
        assert!(html.contains("Version 1.4.0"));
        assert!(html.contains("This is the content."));
        assert!(!html.contains("date:"));
        assert!(!html.contains("title:"));

        let meta = metadata.expect("Metadata should be parsed");
        assert_eq!(meta.date, "2025-01-15");
        assert_eq!(meta.title, "Word Jump Mode");
    }

    #[test]
    fn test_missing_frontmatter() {
        let markdown = "# No Frontmatter\n\nJust regular content.";
        let base_path = Path::new("update-log/1.4.0-test");
        let (html, metadata) = markdown_to_html(markdown, base_path);

        assert!(html.contains("<h1>"));
        assert!(html.contains("No Frontmatter"));
        assert!(metadata.is_none());
    }

    #[test]
    fn test_invalid_frontmatter() {
        let markdown = r#"---
invalid yaml here: [broken
---

# Content"#;
        let base_path = Path::new("update-log/1.4.0-test");
        let (html, metadata) = markdown_to_html(markdown, base_path);

        assert!(html.contains("<h1>"));
        assert!(html.contains("Content"));
        assert!(metadata.is_none());
    }
}
