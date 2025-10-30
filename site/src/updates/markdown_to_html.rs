use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, html};
use std::path::Path;

/// Converts markdown content to HTML with relative image path resolution
///
/// This function uses pulldown_cmark to parse markdown and render it as HTML.
/// Relative image paths are resolved to absolute web paths based on the base_path.
///
/// # Arguments
/// * `markdown` - The markdown content to convert
/// * `base_path` - The base path for resolving relative image URLs (e.g., "update-log/1.4.0-word-jump-mode")
pub fn markdown_to_html(markdown: &str, base_path: &Path) -> String {
    let options = Options::all();
    let parser = Parser::new_ext(markdown, options);

    // Map events to transform relative image paths to absolute web paths
    let parser = parser.map(|event| match event {
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let resolved_url = resolve_image_path(&dest_url, base_path);
            Event::Start(Tag::Image {
                link_type,
                dest_url: resolved_url,
                title,
                id,
            })
        }
        _ => event,
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    html_output
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_markdown() {
        let markdown = "# Hello World\n\nThis is a paragraph.";
        let base_path = Path::new("update-log/1.4.0-test");
        let html = markdown_to_html(markdown, base_path);

        assert!(html.contains("<h1>"));
        assert!(html.contains("Hello World"));
        assert!(html.contains("<p>"));
        assert!(html.contains("This is a paragraph."));
    }

    #[test]
    fn test_code_blocks() {
        let markdown = "```rust\nfn main() {}\n```";
        let base_path = Path::new("update-log/1.4.0-test");
        let html = markdown_to_html(markdown, base_path);

        assert!(html.contains("<code"));
        assert!(html.contains("fn main()"));
    }

    #[test]
    fn test_lists() {
        let markdown = "- Item 1\n- Item 2\n- Item 3";
        let base_path = Path::new("update-log/1.4.0-test");
        let html = markdown_to_html(markdown, base_path);

        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>"));
        assert!(html.contains("Item 1"));
    }

    #[test]
    fn test_links() {
        let markdown = "[GitHub](https://github.com)";
        let base_path = Path::new("update-log/1.4.0-test");
        let html = markdown_to_html(markdown, base_path);

        assert!(html.contains("<a "));
        assert!(html.contains("href=\"https://github.com\""));
        assert!(html.contains("GitHub"));
    }

    #[test]
    fn test_relative_image_path() {
        let markdown = "![Alt text](screenshot.png)";
        let base_path = Path::new("update-log/1.4.0-test");
        let html = markdown_to_html(markdown, base_path);

        assert!(html.contains("<img "));
        assert!(html.contains("src=\"/update-log/1.4.0-test/screenshot.png\""));
        assert!(html.contains("alt=\"Alt text\""));
    }

    #[test]
    fn test_absolute_image_path() {
        let markdown = "![Alt text](/assets/image.png)";
        let base_path = Path::new("update-log/1.4.0-test");
        let html = markdown_to_html(markdown, base_path);

        assert!(html.contains("<img "));
        assert!(html.contains("src=\"/assets/image.png\""));
        assert!(html.contains("alt=\"Alt text\""));
    }

    #[test]
    fn test_http_image_path() {
        let markdown = "![Alt text](https://example.com/image.png)";
        let base_path = Path::new("update-log/1.4.0-test");
        let html = markdown_to_html(markdown, base_path);

        assert!(html.contains("<img "));
        assert!(html.contains("src=\"https://example.com/image.png\""));
        assert!(html.contains("alt=\"Alt text\""));
    }
}
