# Global Mode Incremental Index Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable incremental index updates in global mode (no `.folio/` directory) by caching the index under `~/.cache/folio/<hash>/cache.json`.

**Architecture:** Modify `build_index_incremental()` in `src/index/mod.rs` to resolve a cache path for both workspace and global modes. Global mode caches in `~/.cache/folio/<xxhash>/cache.json` keyed by the absolute root path. Uses existing `twox-hash` dependency — no new crates needed.

**Tech Stack:** Rust, twox-hash (XxHash64), std::fs, serde_json

---

### Task 1: Add `global_cache_path` helper function

**Files:**
- Modify: `src/index/mod.rs`

- [ ] **Step 1: Write the `global_cache_path` function**

Add this function in `src/index/mod.rs` after the `CACHE_PATH` constant (line 25):

```rust
/// Compute the cache path for global (non-workspace) mode.
/// Uses ~/.cache/folio/<xxhash_hex>/cache.json keyed by the absolute root path.
fn global_cache_path(root: &str) -> Option<PathBuf> {
    let abs_root = std::fs::canonicalize(root).ok()?;
    let mut hasher = std::hash::DefaultHasher::new();
    use std::hash::Hash;
    use std::hash::Hasher;
    abs_root.hash(&mut hasher);
    let hash_val = hasher.finish();
    let dir_name = format!("{:016x}", hash_val);

    let base = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    Some(base.join("folio").join(dir_name).join("cache.json"))
}
```

Wait — the project doesn't have `dirs` crate and shouldn't add one. Use `std::env::var("HOME")` or `dirs::cache_dir()` equivalent. Actually, let's keep it simple with no new deps:

```rust
/// Compute the cache path for global (non-workspace) mode.
/// Uses ~/.cache/folio/<xxhash_hex>/cache.json keyed by the absolute root path.
fn global_cache_path(root: &str) -> Option<std::path::PathBuf> {
    use std::hash::{Hash, Hasher};
    use twox_hash::XxHash64;

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
```

- [ ] **Step 2: Add `use twox_hash::XxHash64` import at top of file**

The file currently has no imports beyond the existing ones. Add `use twox_hash::XxHash64;` to the imports section at the top of `src/index/mod.rs`:

```rust
use std::collections::HashSet;
use std::path::Path;

use crate::models::{self, Index};
use twox_hash::XxHash64;
```

(Remove the now-unused `use std::path::Path;` only import — `Path` is already available via `std::path::PathBuf` usage in function bodies.)

- [ ] **Step 3: Build and verify compilation**

Run: `cargo build`
Expected: Compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add src/index/mod.rs
git commit -m "feat(index): add global_cache_path helper for non-workspace cache"
```

---

### Task 2: Refactor `build_index_incremental` to support both modes

**Files:**
- Modify: `src/index/mod.rs:32-119`

- [ ] **Step 1: Replace `build_index_incremental` with unified cache path logic**

Replace the entire `build_index_incremental` function (lines 32-119) with:

```rust
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
```

Key changes from the original:
1. Removed the `if !models::is_workspace(root) { return build_index(root); }` early return.
2. Cache path is now resolved for both modes: workspace uses `.folio/cache.json`, global uses `~/.cache/folio/<hash>/cache.json`.
3. If `global_cache_path` returns `None` (e.g. no `$HOME`), falls back to `build_index(root)` without caching.

- [ ] **Step 2: Build and verify compilation**

Run: `cargo build`
Expected: Compiles with no errors.

- [ ] **Step 3: Run existing tests**

Run: `cargo test`
Expected: All existing tests pass (same as before — no test changes yet).

- [ ] **Step 4: Commit**

```bash
git add src/index/mod.rs
git commit -m "feat(index): enable incremental updates in global mode via ~/.cache/folio"
```

---

### Task 3: Add tests for global cache path and incremental behavior

**Files:**
- Modify: `src/index/mod.rs` (add test module at bottom)

- [ ] **Step 1: Write tests**

Add at the bottom of `src/index/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_global_cache_path_produces_valid_path() {
        // Use current directory as root — must resolve to a valid path
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

        // Create a markdown file
        let md_file = tmp.path().join("test.md");
        fs::write(&md_file, "# Hello\n\nworld\n").unwrap();

        // Build index (first call — no cache, full build)
        let idx1 = build_index_incremental(root).unwrap();
        assert_eq!(idx1.files.len(), 1);

        // Compute where the cache should be
        let abs_root = std::fs::canonicalize(root).unwrap();
        let cache_path = global_cache_path(abs_root.to_str().unwrap()).unwrap();

        // Cache should exist
        assert!(cache_path.exists(), "cache file should be created after build");

        // Second call should load from cache (incremental)
        let idx2 = build_index_incremental(root).unwrap();
        assert_eq!(idx2.files.len(), 1);

        // Clean up the cache dir
        if let Some(parent) = cache_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn test_global_incremental_detects_modification() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_str().unwrap();

        // Create initial file
        let md_file = tmp.path().join("note.md");
        fs::write(&md_file, "# Original\n").unwrap();

        // Build initial index
        let idx1 = build_index_incremental(root).unwrap();
        assert_eq!(idx1.files.len(), 1);

        // Modify file (ensure mtime changes by waiting briefly)
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&md_file, "# Modified\n\nextra content\n").unwrap();

        // Rebuild — should detect modification
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

        // Create two files
        let f1 = tmp.path().join("a.md");
        let f2 = tmp.path().join("b.md");
        fs::write(&f1, "# A\n").unwrap();
        fs::write(&f2, "# B\n").unwrap();

        // Build initial index
        let idx1 = build_index_incremental(root).unwrap();
        assert_eq!(idx1.files.len(), 2);

        // Delete one file
        fs::remove_file(&f1).unwrap();

        // Rebuild — should detect deletion
        let idx2 = build_index_incremental(root).unwrap();
        assert_eq!(idx2.files.len(), 1);

        let abs_root = std::fs::canonicalize(root).unwrap();
        let cache_path = global_cache_path(abs_root.to_str().unwrap()).unwrap();
        if let Some(parent) = cache_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test index::tests`
Expected: All 6 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/index/mod.rs
git commit -m "test(index): add tests for global mode incremental updates"
```

---

### Task 4: Update the spec to reflect XxHash64 instead of SHA-256

**Files:**
- Modify: `docs/superpowers/specs/2026-04-05-global-incremental-index-design.md`

- [ ] **Step 1: Update spec to say XxHash64 instead of SHA-256**

Change `~/.cache/folio/<sha256_prefix>/cache.json` to `~/.cache/folio/<xxhash64_hex>/cache.json` and update the description accordingly:

```markdown
| Global | `~/.cache/folio/<xxhash64_hex>/cache.json` |

- XxHash64 (with seed 0) of the canonicalized absolute root path, formatted as 16 hex characters for the directory name. Uses the existing `twox-hash` dependency.
- Example: `/home/user/docs` → `~/.cache/folio/a1b2c3d4e5f67890/cache.json`
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/specs/2026-04-05-global-incremental-index-design.md
git commit -m "docs: update spec to reflect XxHash64 instead of SHA-256"
```
