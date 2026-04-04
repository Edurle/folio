use std::fs;
use std::path::PathBuf;

/// Recursively scan a directory for .md files, skipping hidden directories.
pub fn scan(root: &str) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut result = Vec::new();
    let root_path = PathBuf::from(root);

    if !root_path.exists() {
        return Ok(result);
    }

    if root_path.is_file() && root_path.extension().is_some_and(|e| e == "md") {
        result.push(root_path);
        return Ok(result);
    }

    if root_path.is_dir() {
        scan_recursive(&root_path, &mut result)?;
    }

    Ok(result)
}

fn scan_recursive(dir: &PathBuf, result: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_str().unwrap_or("");
            if name.starts_with('.') {
                continue;
            }
            scan_recursive(&path, result)?;
        } else if path.extension().is_some_and(|e| e == "md") {
            result.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_scan_finds_md_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.md"), "# A").unwrap();
        fs::write(dir.path().join("b.md"), "# B").unwrap();
        fs::write(dir.path().join("c.txt"), "not markdown").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/d.md"), "# D").unwrap();

        let files = scan(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_scan_skips_hidden_dirs() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".hidden")).unwrap();
        fs::write(dir.path().join(".hidden/secret.md"), "# Secret").unwrap();
        fs::write(dir.path().join("visible.md"), "# Visible").unwrap();

        let files = scan(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("visible.md"));
    }
}
