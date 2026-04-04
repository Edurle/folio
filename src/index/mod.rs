pub mod builder;
pub mod scanner;

use crate::models::Index;

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
