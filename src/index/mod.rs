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
