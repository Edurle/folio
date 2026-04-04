use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

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

    /// Remove an entry from the index, cleaning up tag references.
    pub fn remove_entry(&mut self, path: &PathBuf) {
        if let Some(entry) = self.files.remove(path) {
            for tag in &entry.tags {
                if let Some(paths) = self.tags.get_mut(tag) {
                    paths.retain(|p| p != path);
                    if paths.is_empty() {
                        self.tags.remove(tag);
                    }
                }
            }
        }
    }

    /// Insert or update an entry, cleaning up old tags if replacing.
    pub fn partial_insert(&mut self, old_entry: Option<&FileEntry>, new_entry: FileEntry) {
        // Remove old entry's tags from the tag index
        if let Some(old) = old_entry {
            for tag in &old.tags {
                if let Some(paths) = self.tags.get_mut(tag) {
                    paths.retain(|p| p != &old.path);
                    if paths.is_empty() {
                        self.tags.remove(tag);
                    }
                }
            }
        }
        self.insert(new_entry);
    }

    /// Incrementally rebuild backlinks only for affected paths.
    ///
    /// affected_paths: files that were added, modified, or deleted.
    /// We need to:
    /// 1. Clear backlinks for affected files and files that link to/from them
    /// 2. Recompute backlinks from all outgoing links (only touching affected entries)
    pub fn incremental_rebuild_backlinks(&mut self, affected_paths: &[PathBuf]) {
        let affected_set: HashSet<PathBuf> = affected_paths.iter().cloned().collect();

        // Clear backlinks for affected files themselves
        for path in affected_paths {
            if let Some(entry) = self.files.get_mut(path) {
                entry.backlinks.clear();
            }
        }

        // Clear backlinks pointing to affected files from all other files
        // (because a non-affected file may have linked to a now-changed file)
        for entry in self.files.values_mut() {
            let has_link_to_affected = entry.outgoing_links.iter()
                .any(|link| affected_set.contains(&link.target));
            if has_link_to_affected {
                // Remove backlinks from this entry pointing to affected files
                entry.backlinks.retain(|bl| !affected_set.contains(bl));
            }
        }

        // Recompute: for each file's outgoing links, add backlinks to targets
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

        for (source, target) in link_pairs {
            // Only update if source or target is affected
            if affected_set.contains(&source) || affected_set.contains(&target) {
                if let Some(target_entry) = self.files.get_mut(&target) {
                    if !target_entry.backlinks.contains(&source) {
                        target_entry.backlinks.push(source);
                    }
                }
            }
        }
    }

    /// Save cache to the given path.
    pub fn save_cache(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Load an index from a JSON cache file.
    pub fn load_from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let index: Index = serde_json::from_str(&content)?;
        Ok(index)
    }
}

/// Check if the given directory has a .folio workspace.
pub fn is_workspace(root: &str) -> bool {
    Path::new(root).join(".folio").exists()
}
