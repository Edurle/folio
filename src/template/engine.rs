use std::collections::HashMap;

/// Simple template engine that replaces {{variable}} placeholders.
pub fn render(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{{{}}}}}", key), value);
    }
    result
}

/// Built-in templates.
pub fn builtin_templates() -> HashMap<&'static str, &'static str> {
    let mut templates = HashMap::new();
    templates.insert(
        "basic-note",
        "---\ntitle: {{title}}\ntags:\n  - {{tag}}\ndate: {{date}}\n---\n\n# {{title}}\n\n",
    );
    templates.insert(
        "adr",
        "---\nnumber: {{number}}\ntitle: {{title}}\ndate: {{date}}\nstatus: {{status}}\n---\n\n# {{number}}. {{title}}\n\n## Context\n\n## Decision\n\n## Consequences\n\n",
    );
    templates.insert(
        "changelog-entry",
        "---\ntype: {{type}}\ndate: {{date}}\n---\n\n## {{description}}\n\n",
    );
    templates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render() {
        let template = "---\ntitle: {{title}}\n---\n\n# {{title}}\n\nContent here.\n";
        let mut vars = HashMap::new();
        vars.insert("title".to_string(), "My Note".to_string());

        let result = render(template, &vars);
        assert!(result.contains("title: My Note"));
        assert!(result.contains("# My Note"));
        assert!(!result.contains("{{"));
    }

    #[test]
    fn test_builtin_templates() {
        let templates = builtin_templates();
        assert!(templates.contains_key("basic-note"));
        assert!(templates.contains_key("adr"));
    }
}
