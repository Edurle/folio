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
    // Memory type templates for agent_memory plugin
    templates.insert(
        "memory-decision",
        "---\ntype: decision\nkey: {{key}}\ntags:\n  - {{tag}}\ndate: {{date}}\nstatus: proposed\n---\n\n# {{key}}\n\n## Context\n\n## Decision\n\n## Consequences\n\n",
    );
    templates.insert(
        "memory-error",
        "---\ntype: error\nkey: {{key}}\ntags:\n  - {{tag}}\ndate: {{date}}\nseverity: medium\n---\n\n# {{key}}\n\n## Symptoms\n\n## Root Cause\n\n## Solution\n\n",
    );
    templates.insert(
        "memory-preference",
        "---\ntype: preference\nkey: {{key}}\ntags:\n  - {{tag}}\ndate: {{date}}\ncategory: general\n---\n\n# {{key}}\n\n## Preference\n\n## Examples\n\n",
    );
    templates.insert(
        "memory-fact",
        "---\ntype: fact\nkey: {{key}}\ntags:\n  - {{tag}}\ndate: {{date}}\nconfidence: high\n---\n\n# {{key}}\n\n## Fact\n\n## Source\n\n## Related\n\n",
    );
    templates.insert(
        "memory-procedure",
        "---\ntype: procedure\nkey: {{key}}\ntags:\n  - {{tag}}\ndate: {{date}}\ntrigger: manual\n---\n\n# {{key}}\n\n## Trigger\n\n## Steps\n\n1. \n\n## Prerequisites\n\n",
    );
    templates.insert(
        "memory-task",
        "---\ntype: task\nkey: {{key}}\ntags:\n  - {{tag}}\ndate: {{date}}\nstatus: in-progress\npriority: medium\n---\n\n# {{key}}\n\n## Objective\n\n## Context\n\n## Notes\n\n",
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

    #[test]
    fn test_memory_type_templates() {
        let templates = builtin_templates();
        let mut vars = HashMap::new();
        vars.insert("key".to_string(), "test-decision".to_string());
        vars.insert("tag".to_string(), "rust".to_string());
        vars.insert("date".to_string(), "2026-04-06".to_string());

        let result = render(templates["memory-decision"], &vars);
        assert!(result.contains("type: decision"));
        assert!(result.contains("## Context"));

        let result = render(templates["memory-error"], &vars);
        assert!(result.contains("type: error"));
        assert!(result.contains("## Symptoms"));

        let result = render(templates["memory-preference"], &vars);
        assert!(result.contains("type: preference"));
        assert!(result.contains("## Preference"));

        let result = render(templates["memory-fact"], &vars);
        assert!(result.contains("type: fact"));
        assert!(result.contains("## Source"));

        let result = render(templates["memory-procedure"], &vars);
        assert!(result.contains("type: procedure"));
        assert!(result.contains("## Steps"));

        let result = render(templates["memory-task"], &vars);
        assert!(result.contains("type: task"));
        assert!(result.contains("## Objective"));
    }
}
