use std::path::PathBuf;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::models::{Link, LinkType, Section};

/// Result of parsing a markdown document body.
pub struct ParsedMarkdown {
    pub title: Option<String>,
    pub sections: Vec<Section>,
    pub links: Vec<Link>,
    pub tags: Vec<String>,
    pub word_count: usize,
}

/// Parse markdown body content to extract structure, links, and tags.
pub fn parse(content: &str) -> ParsedMarkdown {
    let mut title: Option<String> = None;
    let mut sections: Vec<Section> = Vec::new();
    let mut links: Vec<Link> = Vec::new();
    let mut tags: Vec<String> = Vec::new();

    let mut current_section: Option<Section> = None;
    let mut byte_offset = 0usize;
    let mut in_heading = false;
    let mut heading_level: u8 = 0;
    let mut heading_text = String::new();
    let mut word_count = 0usize;
    let mut line_number = 1usize;

    let opts = Options::empty();
    let parser = Parser::new_ext(content, opts);

    for event in parser {
        match event {
            Event::Start(Tag::Heading {
                level,
                id: _,
                classes: _,
                attrs: _,
            }) => {
                // Save previous section
                if let Some(mut sec) = current_section.take() {
                    sec.content_end = byte_offset;
                    sections.push(sec);
                }
                in_heading = true;
                heading_level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                let heading = heading_text.trim().to_string();

                if title.is_none() {
                    title = Some(heading.clone());
                }

                current_section = Some(Section {
                    level: heading_level,
                    heading,
                    content_start: byte_offset,
                    content_end: 0, // will be set when next section starts or at end
                });
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title: _,
                id: _,
            }) => {
                let dest = dest_url.to_string();
                if !dest.is_empty() && !dest.starts_with('#') && !dest.contains("://") {
                    let lt = match link_type {
                        pulldown_cmark::LinkType::Inline => LinkType::MarkdownLink,
                        _ => LinkType::MarkdownLink,
                    };
                    links.push(Link {
                        target: PathBuf::from(dest),
                        line_number,
                        link_type: lt,
                    });
                }
            }
            Event::Code(code) | Event::InlineHtml(code) => {
                byte_offset += code.len();
            }
            Event::Text(text) => {
                if in_heading {
                    heading_text.push_str(&text);
                }
                word_count += text.split_whitespace().count();

                // Extract inline tags like #tag
                for word in text.split_whitespace() {
                    if word.starts_with('#') && word.len() > 1 {
                        let tag = word.trim_start_matches('#').trim_end_matches(|c: char| {
                            c == '.' || c == ',' || c == ';' || c == ':' || c == '!' || c == '?'
                        });
                        if !tag.is_empty() && !tag.starts_with('/') {
                            // Skip heading-like things that aren't tags
                            if !tags.contains(&tag.to_string()) {
                                tags.push(tag.to_string());
                            }
                        }
                    }
                }

                byte_offset += text.len();
            }
            Event::SoftBreak | Event::HardBreak => {
                line_number += 1;
                byte_offset += 1;
            }
            Event::Html(html) => {
                byte_offset += html.len();
            }
            _ => {}
        }
    }

    // Save last section
    if let Some(mut sec) = current_section.take() {
        sec.content_end = byte_offset;
        sections.push(sec);
    }

    // Extract wikilinks [[target]] from raw content since pulldown-cmark doesn't parse them natively
    extract_wikilinks(content, &mut links, &mut tags);

    ParsedMarkdown {
        title,
        sections,
        links,
        tags,
        word_count,
    }
}

/// Extract [[wikilinks]] and [[path|alias]] from raw content.
fn extract_wikilinks(content: &str, links: &mut Vec<Link>, _tags: &mut Vec<String>) {
    let mut line_number = 1;

    for line in content.lines() {
        let bytes = line.as_bytes();
        let mut i = 0;
        while i + 3 < bytes.len() {
            if &bytes[i..i + 2] == b"[[" {
                // Find closing ]]
                if let Some(end) = line[i + 2..].find("]]") {
                    let inner = &line[i + 2..i + 2 + end];
                    let target = if let Some(pipe_pos) = inner.find('|') {
                        &inner[..pipe_pos]
                    } else {
                        inner
                    };
                    let target = target.trim();

                    if !target.is_empty() {
                        links.push(Link {
                            target: PathBuf::from(target),
                            line_number,
                            link_type: LinkType::WikiLink,
                        });
                    }
                    i = i + 2 + end + 2;
                    continue;
                }
            }
            i += 1;
        }
        line_number += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let content = "# Title\n\nSome body text here.\n\n## Section 1\n\nContent of section 1.\n";
        let result = parse(content);
        assert_eq!(result.title, Some("Title".to_string()));
        assert_eq!(result.sections.len(), 2);
        assert!(result.word_count > 0);
    }

    #[test]
    fn test_parse_wikilinks() {
        let content = "# Note\n\nSee [[other-note]] and [[path/to/file|display text]].\n";
        let result = parse(content);
        assert_eq!(result.links.len(), 2);
        assert_eq!(result.links[0].target, PathBuf::from("other-note"));
        assert_eq!(result.links[1].target, PathBuf::from("path/to/file"));
        assert_eq!(result.links[0].link_type, LinkType::WikiLink);
    }

    #[test]
    fn test_parse_markdown_links() {
        let content = "# Note\n\nSee [link](other.md) for details.\n";
        let result = parse(content);
        assert_eq!(result.links.len(), 1);
        assert_eq!(result.links[0].target, PathBuf::from("other.md"));
        assert_eq!(result.links[0].link_type, LinkType::MarkdownLink);
    }

    #[test]
    fn test_parse_inline_tags() {
        let content = "# Note\n\nThis is about #rust and #cli tools.\n";
        let result = parse(content);
        assert!(result.tags.contains(&"rust".to_string()));
        assert!(result.tags.contains(&"cli".to_string()));
    }

    #[test]
    fn test_parse_no_title() {
        let content = "Just some text without a heading.\n";
        let result = parse(content);
        assert!(result.title.is_none());
    }
}
