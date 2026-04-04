use std::collections::HashMap;

/// Result of splitting a markdown file into frontmatter and body.
pub struct FrontmatterResult {
    pub frontmatter: HashMap<String, serde_yaml::Value>,
    pub body_start: usize,
}

/// Extract YAML frontmatter from markdown content.
/// Frontmatter is delimited by `---` at the start of the file.
///
/// Returns a FrontmatterResult with the parsed frontmatter fields
/// and the byte offset where the body content begins.
pub fn parse(content: &str) -> FrontmatterResult {
    let trimmed = content.trim_start();

    if !trimmed.starts_with("---") {
        return FrontmatterResult {
            frontmatter: HashMap::new(),
            body_start: 0,
        };
    }

    // The closing --- must be on its own line.
    // Look for "\n---" after the opening ---.
    let after_opening = &trimmed[3..];

    // The YAML content starts after optional newline following opening ---
    let yaml_start = after_opening
        .find(|c: char| c != '\n' && c != '\r')
        .unwrap_or(0);

    // Find closing --- (must be at start of a line)
    let search_from = 0;
    let mut close_pos = None;
    let mut pos = search_from;
    for line in after_opening[search_from..].split('\n') {
        if line.trim() == "---" && pos > 0 {
            // Found closing ---
            close_pos = Some(pos);
            break;
        }
        pos += line.len() + 1; // +1 for the \n
    }

    let Some(close_offset) = close_pos else {
        return FrontmatterResult {
            frontmatter: HashMap::new(),
            body_start: 0,
        };
    };

    let yaml_str = &after_opening[yaml_start..close_offset].trim_end();
    let frontmatter: HashMap<String, serde_yaml::Value> =
        serde_yaml::from_str(yaml_str).unwrap_or_default();

    // Calculate body_start in original content
    // after_opening starts at: content.len() - trimmed.len() + 3
    let prefix_len = content.len() - trimmed.len() + 3;
    let body_start_in_after = close_offset + after_opening[close_offset..]
        .find(|c: char| c != '-' && c != '\n' && c != '\r')
        .unwrap_or(after_opening[close_offset..].len());

    let body_start = prefix_len + body_start_in_after;

    FrontmatterResult {
        frontmatter,
        body_start,
    }
}

/// Serialize frontmatter back to YAML string with `---` delimiters.
pub fn serialize(frontmatter: &HashMap<String, serde_yaml::Value>) -> String {
    if frontmatter.is_empty() {
        return String::new();
    }
    let yaml = serde_yaml::to_string(frontmatter).unwrap_or_default();
    format!("---\n{}---\n", yaml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_with_frontmatter() {
        let content = "---\ntitle: Hello\ntags:\n  - rust\n  - cli\n---\n\nBody content\n";
        let result = parse(content);
        assert_eq!(
            result.frontmatter.get("title").unwrap().as_str(),
            Some("Hello")
        );
        assert!(result.body_start > 0);
        assert_eq!(&content[result.body_start..], "Body content\n");
    }

    #[test]
    fn test_parse_without_frontmatter() {
        let content = "# Hello\n\nBody content\n";
        let result = parse(content);
        assert!(result.frontmatter.is_empty());
        assert_eq!(result.body_start, 0);
    }

    #[test]
    fn test_parse_empty_frontmatter() {
        let content = "---\n---\n\nBody content\n";
        let result = parse(content);
        assert!(result.frontmatter.is_empty());
        assert!(result.body_start > 0);
        assert_eq!(&content[result.body_start..], "Body content\n");
    }

    #[test]
    fn test_serialize() {
        let mut fm = HashMap::new();
        fm.insert(
            "title".to_string(),
            serde_yaml::Value::String("Test".to_string()),
        );
        let serialized = serialize(&fm);
        assert!(serialized.starts_with("---\n"));
        assert!(serialized.contains("title: Test"));
        assert!(serialized.ends_with("---\n"));
    }
}
