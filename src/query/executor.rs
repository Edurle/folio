use crate::models::{FileEntry, Index};
use crate::query::parser::{self, Expr, Op, Value};

/// Execute a query expression against the index.
pub fn execute<'a>(index: &'a Index, expression: &str) -> Result<Vec<&'a FileEntry>, String> {
    let expr = parser::parse(expression)?;
    let mut results: Vec<&FileEntry> = index.files.values().collect();

    results.retain(|entry| evaluate(&expr, entry));
    Ok(results)
}

/// Evaluate an expression against a single FileEntry.
fn evaluate(expr: &Expr, entry: &FileEntry) -> bool {
    match expr {
        Expr::All => true,
        Expr::And(left, right) => evaluate(left, entry) && evaluate(right, entry),
        Expr::Or(left, right) => evaluate(left, entry) || evaluate(right, entry),
        Expr::Comparison {
            field,
            is_frontmatter,
            op,
            value,
        } => {
            let field_value = if *is_frontmatter {
                entry.frontmatter.get(field).map(|v| yaml_to_string(v))
            } else {
                get_field_value(field, entry)
            };

            match field_value {
                Some(fv) => compare(&fv, op, value, entry),
                None => false,
            }
        }
        Expr::FuncCall { .. } => {
            // Function calls not fully implemented yet
            true
        }
    }
}

fn get_field_value(field: &str, entry: &FileEntry) -> Option<String> {
    match field {
        "title" => entry.title.clone(),
        "path" => Some(entry.path.to_str().unwrap_or("").to_string()),
        "content" => {
            // Content matching needs the raw file content, use path as marker
            Some(String::new())
        }
        "tags" => Some(entry.tags.join(",")),
        "word_count" => Some(entry.word_count.to_string()),
        "size" => Some(entry.size.to_string()),
        "created" => entry.created.map(|dt| dt.to_rfc3339()),
        "modified" => entry.modified.map(|dt| dt.to_rfc3339()),
        _ => entry.frontmatter.get(field).map(|v| yaml_to_string(v)),
    }
}

fn yaml_to_string(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Sequence(seq) => {
            seq.iter().map(|v| yaml_to_string(v)).collect::<Vec<_>>().join(",")
        }
        serde_yaml::Value::Null => String::new(),
        _ => format!("{:?}", value),
    }
}

fn compare(field_value: &str, op: &Op, value: &Value, _entry: &FileEntry) -> bool {
    match (op, value) {
        (Op::Eq, Value::String(s)) => field_value == s,
        (Op::Eq, Value::Number(n)) => {
            field_value.parse::<f64>().map(|v| v == *n).unwrap_or(false)
        }
        (Op::Eq, Value::Boolean(b)) => field_value == b.to_string(),
        (Op::Neq, Value::String(s)) => field_value != s,
        (Op::Neq, Value::Number(n)) => {
            field_value.parse::<f64>().map(|v| v != *n).unwrap_or(false)
        }
        (Op::Gt, Value::String(s)) => field_value > s.as_str(),
        (Op::Gt, Value::Number(n)) => {
            field_value.parse::<f64>().map(|v| v > *n).unwrap_or(false)
        }
        (Op::Lt, Value::String(s)) => field_value < s.as_str(),
        (Op::Lt, Value::Number(n)) => {
            field_value.parse::<f64>().map(|v| v < *n).unwrap_or(false)
        }
        (Op::Gte, Value::String(s)) => field_value >= s.as_str(),
        (Op::Gte, Value::Number(n)) => {
            field_value.parse::<f64>().map(|v| v >= *n).unwrap_or(false)
        }
        (Op::Lte, Value::String(s)) => field_value <= s.as_str(),
        (Op::Lte, Value::Number(n)) => {
            field_value.parse::<f64>().map(|v| v <= *n).unwrap_or(false)
        }
        (Op::Contains, Value::String(s)) => {
            // For tags (comma-separated) or general string containment
            let parts: Vec<&str> = field_value.split(',').collect();
            parts.iter().any(|p| p.trim() == s) || field_value.contains(s.as_str())
        }
        (Op::Matches, Value::String(_pattern)) => {
            // Simple substring match for now; can upgrade to regex later
            field_value.contains(match value {
                Value::String(s) => s.as_str(),
                _ => "",
            })
        }
        (Op::In, Value::List(items)) => {
            items.iter().any(|item| match item {
                Value::String(s) => {
                    let parts: Vec<&str> = field_value.split(',').collect();
                    parts.iter().any(|p| p.trim() == s) || field_value == s.as_str()
                }
                Value::Number(n) => field_value.parse::<f64>().map(|v| v == *n).unwrap_or(false),
                _ => false,
            })
        }
        (Op::StartsWith, Value::String(s)) => field_value.starts_with(s.as_str()),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_test_entry(path: &str, title: &str, tags: Vec<&str>, status: &str) -> FileEntry {
        let mut frontmatter = HashMap::new();
        frontmatter.insert("status".to_string(), serde_yaml::Value::String(status.to_string()));
        frontmatter.insert("tags".to_string(), serde_yaml::Value::Sequence(
            tags.iter().map(|t| serde_yaml::Value::String(t.to_string())).collect()
        ));

        FileEntry {
            path: PathBuf::from(path),
            title: Some(title.to_string()),
            frontmatter,
            sections: Vec::new(),
            outgoing_links: Vec::new(),
            backlinks: Vec::new(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            word_count: 100,
            created: None,
            modified: None,
            size: 1024,
            checksum: 0,
        }
    }

    #[test]
    fn test_execute_eq() {
        let mut index = Index::new();
        index.insert(make_test_entry("a.md", "A", vec!["rust"], "draft"));
        index.insert(make_test_entry("b.md", "B", vec!["python"], "published"));

        let results = execute(&index, "status = 'draft'").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, Some("A".to_string()));
    }

    #[test]
    fn test_execute_contains() {
        let mut index = Index::new();
        index.insert(make_test_entry("a.md", "A", vec!["rust", "cli"], "draft"));
        index.insert(make_test_entry("b.md", "B", vec!["python"], "published"));

        let results = execute(&index, "tags contains 'rust'").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_execute_and() {
        let mut index = Index::new();
        index.insert(make_test_entry("a.md", "A", vec!["rust"], "draft"));
        index.insert(make_test_entry("b.md", "B", vec!["rust"], "published"));
        index.insert(make_test_entry("c.md", "C", vec!["python"], "draft"));

        let results = execute(&index, "status = 'draft' AND tags contains 'rust'").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, Some("A".to_string()));
    }

    #[test]
    fn test_execute_in() {
        let mut index = Index::new();
        index.insert(make_test_entry("a.md", "A", vec!["rust"], "draft"));
        index.insert(make_test_entry("b.md", "B", vec!["python"], "review"));

        let results = execute(&index, "status in ['draft', 'review']").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_execute_starts_with() {
        let mut index = Index::new();
        index.insert(make_test_entry("./memory/note.md", "Memory Note", vec!["mem"], "draft"));
        index.insert(make_test_entry("./docs/guide.md", "Docs Guide", vec!["doc"], "published"));

        let results = execute(&index, "path starts_with './memory/'").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, Some("Memory Note".to_string()));
    }

    #[test]
    fn test_execute_frontmatter() {
        let mut index = Index::new();
        index.insert(make_test_entry("a.md", "A", vec!["rust"], "draft"));

        let results = execute(&index, "frontmatter.status = 'draft'").unwrap();
        assert_eq!(results.len(), 1);
    }
}
