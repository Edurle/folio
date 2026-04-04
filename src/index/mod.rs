pub mod builder;
pub mod scanner;

use std::collections::HashSet;
use std::path::Path;

use twox_hash::XxHash64;

use crate::models::{self, Index};

/// Build a full index from scratch (original behavior).
pub fn build_index(root: &str) -> Result<Index, Box<dyn std::error::Error>> {
    let files = scanner::scan(root)?;
    let mut index = Index::new();

    for path in files {
        match builder::build_entry(&path) {
            Ok(entry) => index.insert(entry),
            Err(e) => eprintln!("Warning: failed to index {:?}: {}", path, e),
        }
    }

    index.rebuild_backlinks();
    Ok(index)
}

const CACHE_PATH: &str = ".folio/cache.json";

/// Compute the cache path for global (non-workspace) mode.
/// Uses ~/.cache/folio/<xxhash_hex>/cache.json keyed by the absolute root path.
fn global_cache_path(root: &str) -> Option<std::path::PathBuf> {
    use std::hash::{Hash, Hasher};

    let abs_root = std::fs::canonicalize(root).ok()?;
    let mut hasher = XxHash64::with_seed(0);
    abs_root.hash(&mut hasher);
    let hash_val = hasher.finish();
    let dir_name = format!("{:016x}", hash_val);

    let home = std::env::var("HOME").ok()?;
    Some(std::path::PathBuf::from(home)
        .join(".cache")
        .join("folio")
        .join(dir_name)
        .join("cache.json"))
}

/// Build index with incremental update support.
///
/// In workspace mode (.folio/ exists), loads cached index from .folio/cache.json.
/// In global mode, loads cached index from ~/.cache/folio/<hash>/cache.json.
/// Falls back to full rebuild when no cache exists.
pub fn build_index_incremental(root: &str) -> Result<Index, Box<dyn std::error::Error>> {
    let cache_path = if models::is_workspace(root) {
        Path::new(root).join(CACHE_PATH)
    } else {
        match global_cache_path(root) {
            Some(p) => p,
            None => return build_index(root),
        }
    };

    // Try to load cached index
    let mut index = match Index::load_from_file(&cache_path) {
        Ok(idx) => idx,
        Err(_) => {
            // No valid cache — full rebuild then save
            let idx = build_index(root)?;
            let _ = idx.save_cache(&cache_path);
            return Ok(idx);
        }
    };

    // Scan current files with metadata
    let current_files = scanner::scan_with_meta(root)?;

    // Build a set of current file paths for quick lookup
    let current_paths: HashSet<std::path::PathBuf> = current_files.iter()
        .map(|f| f.path.clone())
        .collect();

    // Detect changes
    let mut affected_paths: Vec<std::path::PathBuf> = Vec::new();
    let mut _added = 0usize;
    let mut _modified = 0usize;
    let mut _deleted = 0usize;

    // Find new and modified files
    for file_meta in &current_files {
        match index.files.get(&file_meta.path) {
            None => {
                // New file
                match builder::build_entry(&file_meta.path) {
                    Ok(entry) => {
                        affected_paths.push(file_meta.path.clone());
                        index.partial_insert(None, entry);
                        _added += 1;
                    }
                    Err(e) => eprintln!("Warning: failed to index {:?}: {}", file_meta.path, e),
                }
            }
            Some(cached_entry) => {
                // Check if modified via mtime + size
                let cached_mtime = cached_entry.modified.map(|dt| {
                    dt.timestamp_millis() as u64
                }).unwrap_or(0);

                if cached_mtime != file_meta.mtime_ms || cached_entry.size != file_meta.size {
                    // Modified — rebuild entry
                    match builder::build_entry(&file_meta.path) {
                        Ok(entry) => {
                            affected_paths.push(file_meta.path.clone());
                            let old = index.files.get(&file_meta.path).cloned();
                            index.partial_insert(old.as_ref(), entry);
                            _modified += 1;
                        }
                        Err(e) => eprintln!("Warning: failed to index {:?}: {}", file_meta.path, e),
                    }
                }
            }
        }
    }

    // Find deleted files (in cache but not on disk)
    let cached_paths: Vec<std::path::PathBuf> = index.files.keys().cloned().collect();
    for cached_path in &cached_paths {
        if !current_paths.contains(cached_path) {
            affected_paths.push(cached_path.clone());
            index.remove_entry(cached_path);
            _deleted += 1;
        }
    }

    // Incrementally rebuild backlinks for affected files
    if !affected_paths.is_empty() {
        index.incremental_rebuild_backlinks(&affected_paths);
    }

    // Save updated cache
    let _ = index.save_cache(&cache_path);

    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_global_cache_path_produces_valid_path() {
        let root = ".";
        let path = global_cache_path(root);
        assert!(path.is_some(), "global_cache_path should return Some for valid root");

        let cache = path.unwrap();
        assert!(cache.to_string_lossy().contains(".cache/folio/"));
        assert!(cache.to_string_lossy().ends_with("cache.json"));
    }

    #[test]
    fn test_global_cache_path_deterministic() {
        let root = ".";
        let p1 = global_cache_path(root);
        let p2 = global_cache_path(root);
        assert_eq!(p1, p2, "same root should produce same cache path");
    }

    #[test]
    fn test_global_cache_path_different_roots() {
        let p1 = global_cache_path(".");
        let p2 = global_cache_path("..");
        assert_ne!(p1, p2, "different roots should produce different cache paths");
    }

    #[test]
    fn test_global_incremental_creates_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_str().unwrap();

        let md_file = tmp.path().join("test.md");
        fs::write(&md_file, "# Hello\n\nworld\n").unwrap();

        let idx1 = build_index_incremental(root).unwrap();
        assert_eq!(idx1.files.len(), 1);

        let abs_root = std::fs::canonicalize(root).unwrap();
        let cache_path = global_cache_path(abs_root.to_str().unwrap()).unwrap();
        assert!(cache_path.exists(), "cache file should be created after build");

        let idx2 = build_index_incremental(root).unwrap();
        assert_eq!(idx2.files.len(), 1);

        if let Some(parent) = cache_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn test_global_incremental_detects_modification() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_str().unwrap();

        let md_file = tmp.path().join("note.md");
        fs::write(&md_file, "# Original\n").unwrap();

        let idx1 = build_index_incremental(root).unwrap();
        assert_eq!(idx1.files.len(), 1);

        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&md_file, "# Modified\n\nextra content\n").unwrap();

        let idx2 = build_index_incremental(root).unwrap();
        assert_eq!(idx2.files.len(), 1);

        let abs_root = std::fs::canonicalize(root).unwrap();
        let cache_path = global_cache_path(abs_root.to_str().unwrap()).unwrap();
        if let Some(parent) = cache_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn test_global_incremental_detects_deletion() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_str().unwrap();

        let f1 = tmp.path().join("a.md");
        let f2 = tmp.path().join("b.md");
        fs::write(&f1, "# A\n").unwrap();
        fs::write(&f2, "# B\n").unwrap();

        let idx1 = build_index_incremental(root).unwrap();
        assert_eq!(idx1.files.len(), 2);

        fs::remove_file(&f1).unwrap();

        let idx2 = build_index_incremental(root).unwrap();
        assert_eq!(idx2.files.len(), 1);

        let abs_root = std::fs::canonicalize(root).unwrap();
        let cache_path = global_cache_path(abs_root.to_str().unwrap()).unwrap();
        if let Some(parent) = cache_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
}
