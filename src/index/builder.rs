use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use twox_hash::XxHash64;
use std::hash::Hasher;

use crate::models::{FileEntry, Link, Section};
use crate::parser;

/// Build a FileEntry from a markdown file on disk.
pub fn build_entry(path: &PathBuf) -> Result<FileEntry, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let metadata = fs::metadata(path)?;

    let checksum = {
        let mut hasher = XxHash64::with_seed(0);
        hasher.write(content.as_bytes());
        hasher.finish()
    };

    let fm_result = parser::frontmatter::parse(&content);
    let body = &content[fm_result.body_start..];

    let md_result = parser::markdown::parse(body);

    // Merge tags from frontmatter and inline
    let mut all_tags = md_result.tags;
    if let Some(serde_yaml::Value::Sequence(tags)) = fm_result.frontmatter.get("tags") {
        for tag in tags {
            if let Some(s) = tag.as_str() {
                if !all_tags.contains(&s.to_string()) {
                    all_tags.push(s.to_string());
                }
            }
        }
    }

    let modified = metadata.modified().ok().map(|t| {
        let dt: DateTime<Utc> = t.into();
        dt
    });

    let created = metadata.created().ok().map(|t| {
        let dt: DateTime<Utc> = t.into();
        dt
    });

    // Extract title from frontmatter if not in body
    let title = md_result.title.or_else(|| {
        fm_result
            .frontmatter
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    });

    Ok(FileEntry {
        path: path.clone(),
        title,
        frontmatter: fm_result.frontmatter,
        sections: md_result.sections,
        outgoing_links: md_result.links,
        backlinks: Vec::new(), // populated by Index::rebuild_backlinks
        tags: all_tags,
        word_count: md_result.word_count,
        created,
        modified,
        size: metadata.len(),
        checksum,
    })
}

/// Resolve link targets relative to the source file's directory.
/// Converts relative paths to absolute paths within the root.
pub fn resolve_link_target(source: &PathBuf, target: &PathBuf, root: &PathBuf) -> Option<PathBuf> {
    // If target has .md extension, resolve relative to source
    if target.extension().is_some_and(|e| e == "md") || !target.to_str().unwrap_or("").contains('.') {
        let source_dir = source.parent()?;
        let resolved = source_dir.join(target);

        // Try with .md extension if no extension
        if resolved.extension().is_none() {
            let with_ext = resolved.with_extension("md");
            if with_ext.exists() {
                return Some(with_ext);
            }
        }

        if resolved.exists() {
            return Some(resolved);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_build_entry_basic() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(
            &file_path,
            "---\ntitle: Test\ntags:\n  - unit\n---\n\n# Test\n\nHello world.\n",
        )
        .unwrap();

        let entry = build_entry(&file_path).unwrap();
        assert_eq!(entry.title, Some("Test".to_string()));
        assert!(entry.tags.contains(&"unit".to_string()));
        assert!(entry.word_count > 0);
        assert!(entry.checksum != 0);
    }

    #[test]
    fn test_build_entry_with_links() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("linked.md");
        fs::write(
            &file_path,
            "# Note\n\nSee [[other]] and [link](file.md).\n",
        )
        .unwrap();

        let entry = build_entry(&file_path).unwrap();
        assert_eq!(entry.outgoing_links.len(), 2);
    }
}
