use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Link types found in markdown content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkType {
    WikiLink,
    MarkdownLink,
    TagReference,
}

/// A single link extracted from a markdown file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub target: PathBuf,
    pub line_number: usize,
    pub link_type: LinkType,
}

/// A section identified by a heading in the markdown file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub level: u8,
    pub heading: String,
    pub content_start: usize,
    pub content_end: usize,
}

/// Parsed representation of a single markdown file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub title: Option<String>,
    pub frontmatter: HashMap<String, serde_yaml::Value>,
    pub sections: Vec<Section>,
    pub outgoing_links: Vec<Link>,
    pub backlinks: Vec<PathBuf>,
    pub tags: Vec<String>,
    pub word_count: usize,
    pub created: Option<DateTime<Utc>>,
    pub modified: Option<DateTime<Utc>>,
    pub size: u64,
    pub checksum: u64,
}

/// The full in-memory index of all scanned markdown files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    pub files: HashMap<PathBuf, FileEntry>,
    pub tags: HashMap<String, Vec<PathBuf>>,
}

impl Index {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            tags: HashMap::new(),
        }
    }

    pub fn insert(&mut self, entry: FileEntry) {
        // Update tag index
        for tag in &entry.tags {
            self.tags
                .entry(tag.clone())
                .or_default()
                .push(entry.path.clone());
        }
        self.files.insert(entry.path.clone(), entry);
    }

    pub fn get(&self, path: &PathBuf) -> Option<&FileEntry> {
        self.files.get(path)
    }

    /// Rebuild backlinks from all outgoing links.
    pub fn rebuild_backlinks(&mut self) {
        // Clear existing backlinks
        for entry in self.files.values_mut() {
            entry.backlinks.clear();
        }

        // Collect all outgoing link targets
        let link_pairs: Vec<(PathBuf, PathBuf)> = self
            .files
            .values()
            .flat_map(|entry| {
                entry
                    .outgoing_links
                    .iter()
                    .map(|link| (entry.path.clone(), link.target.clone()))
            })
            .collect();

        // Apply backlinks
        for (source, target) in link_pairs {
            if let Some(target_entry) = self.files.get_mut(&target) {
                if !target_entry.backlinks.contains(&source) {
                    target_entry.backlinks.push(source);
                }
            }
        }
    }
}
