mod cli;
mod commands;
mod index;
mod models;
mod parser;
mod plugins;
mod query;
mod template;

use std::fs;
use std::path::PathBuf;

use clap::Parser;

use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Commands::New { path, template, content } => {
            cmd_new(&path, template.as_deref(), content.as_deref())?;
        }
        Commands::Cat { path } => {
            cmd_cat(&path, cli.pretty)?;
        }
        Commands::Edit { action } => {
            cmd_edit(action)?;
        }
        Commands::Rm { path } => {
            cmd_rm(&path)?;
        }
        Commands::Mv { src, dst } => {
            cmd_mv(&src, &dst)?;
        }
        Commands::Ls { path, tag, filter } => {
            cmd_ls(path.as_deref(), tag.as_deref(), filter.as_deref(), cli.pretty)?;
        }
        Commands::Query { expression } => {
            cmd_query(".", &expression, cli.pretty)?;
        }
        Commands::Search { text } => {
            cmd_search(".", &text, cli.pretty)?;
        }
        Commands::Tags => {
            cmd_tags(".", cli.pretty)?;
        }
        Commands::Graph { path, full, orphans, path_between } => {
            cmd_graph(path.as_deref(), full, orphans, path_between.as_deref(), cli.pretty)?;
        }
        Commands::Template { action } => {
            cmd_template(action)?;
        }
        Commands::Batch { action } => {
            cmd_batch(action)?;
        }
        Commands::Init => {
            cmd_init()?;
        }
        Commands::Status => {
            cmd_status()?;
        }
        Commands::Index => {
            cmd_index()?;
        }
        Commands::Plugin { action } => {
            cmd_plugin(action)?;
        }
    }
    Ok(())
}

fn cmd_new(path: &str, _template: Option<&str>, content: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = PathBuf::from(path);

    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let body = content.unwrap_or("");
    fs::write(&file_path, body)?;

    let result = serde_json::json!({
        "status": "ok",
        "path": path,
        "created": true
    });
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn cmd_cat(path: &str, pretty: bool) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = PathBuf::from(path);
    let content = fs::read_to_string(&file_path)?;

    let fm_result = parser::frontmatter::parse(&content);
    let body = &content[fm_result.body_start..];
    let md_result = parser::markdown::parse(body);

    let mut all_tags = md_result.tags;
    if let Some(serde_yaml::Value::Sequence(tags)) = fm_result.frontmatter.get("tags") {
        for tag in tags {
            if let Some(s) = tag.as_str() {
                if !all_tags.contains(&s.to_string()) {
                    all_tags.push(s.to_string());
                }
            }
        }
    }

    let metadata = fs::metadata(&file_path)?;
    let modified = metadata.modified().ok().map(|t| {
        let dt: chrono::DateTime<chrono::Utc> = t.into();
        dt.to_rfc3339()
    });

    let result = serde_json::json!({
        "path": path,
        "title": md_result.title,
        "frontmatter": fm_result.frontmatter,
        "sections": md_result.sections.iter().map(|s| serde_json::json!({
            "level": s.level,
            "heading": s.heading,
        })).collect::<Vec<_>>(),
        "links": md_result.links.iter().map(|l| serde_json::json!({
            "target": l.target.to_str().unwrap_or(""),
            "line_number": l.line_number,
            "type": format!("{:?}", l.link_type),
        })).collect::<Vec<_>>(),
        "tags": all_tags,
        "word_count": md_result.word_count,
        "size": metadata.len(),
        "modified": modified,
    });

    if pretty {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("{}", serde_json::to_string(&result)?);
    }
    Ok(())
}

fn cmd_edit(action: cli::EditAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        cli::EditAction::Frontmatter { path, key, value } => {
            edit_frontmatter(&path, &key, &value)?;
        }
        cli::EditAction::Section { path, heading, content } => {
            edit_section(&path, &heading, content.as_deref())?;
        }
        cli::EditAction::Append { path, content } => {
            edit_append(&path, content.as_deref())?;
        }
    }
    Ok(())
}

fn edit_frontmatter(path: &str, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let fm_result = parser::frontmatter::parse(&content);
    let body = &content[fm_result.body_start..];

    let mut frontmatter = fm_result.frontmatter;
    let yaml_value: serde_yaml::Value = serde_yaml::from_str(value)
        .unwrap_or(serde_yaml::Value::String(value.to_string()));
    frontmatter.insert(key.to_string(), yaml_value);

    let new_content = format!("{}{}", parser::frontmatter::serialize(&frontmatter), body);
    fs::write(path, new_content)?;

    let result = serde_json::json!({
        "status": "ok",
        "path": path,
        "key": key,
        "value": value
    });
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

fn edit_section(path: &str, heading: &str, content: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let file_content = fs::read_to_string(path)?;
    let fm_result = parser::frontmatter::parse(&file_content);
    let body = &file_content[fm_result.body_start..];

    let new_section_content = match content {
        Some(c) => c.to_string(),
        None => std::io::read_to_string(std::io::stdin())?,
    };

    let md_result = parser::markdown::parse(body);
    let section_idx = md_result.sections.iter().position(|s| s.heading == heading);

    match section_idx {
        Some(idx) => {
            let section = &md_result.sections[idx];
            let section_level = section.level;
            let prefix = "#".repeat(section_level as usize);

            let before = &body[..section.content_start];
            let after = if idx + 1 < md_result.sections.len() {
                &body[md_result.sections[idx + 1].content_start..]
            } else {
                ""
            };

            let new_body = format!(
                "{}{} {}\n\n{}\n\n{}",
                before,
                prefix,
                heading,
                new_section_content.trim(),
                after
            );

            let new_content = format!(
                "{}{}",
                parser::frontmatter::serialize(&fm_result.frontmatter),
                new_body
            );
            fs::write(path, new_content)?;

            let result = serde_json::json!({
                "status": "ok",
                "path": path,
                "section": heading,
                "updated": true
            });
            println!("{}", serde_json::to_string(&result)?);
        }
        None => {
            let result = serde_json::json!({
                "status": "error",
                "message": format!("Section '{}' not found", heading)
            });
            println!("{}", serde_json::to_string(&result)?);
        }
    }
    Ok(())
}

fn edit_append(path: &str, content: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let file_content = fs::read_to_string(path)?;
    let append_content = match content {
        Some(c) => c.to_string(),
        None => std::io::read_to_string(std::io::stdin())?,
    };

    let new_content = format!("{}\n{}\n", file_content.trim_end(), append_content);
    fs::write(path, new_content)?;

    let result = serde_json::json!({
        "status": "ok",
        "path": path,
        "appended": true
    });
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

fn cmd_rm(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    fs::remove_file(path)?;

    let result = serde_json::json!({
        "status": "ok",
        "path": path,
        "deleted": true
    });
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

fn cmd_mv(src: &str, dst: &str) -> Result<(), Box<dyn std::error::Error>> {
    let src_path = PathBuf::from(src);
    let dst_path = PathBuf::from(dst);

    if let Some(parent) = dst_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::rename(&src_path, &dst_path)?;

    let result = serde_json::json!({
        "status": "ok",
        "from": src,
        "to": dst,
        "links_updated": false
    });
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

fn cmd_ls(path: Option<&str>, _tag: Option<&str>, _filter: Option<&str>, pretty: bool) -> Result<(), Box<dyn std::error::Error>> {
    let dir = path.unwrap_or(".");
    let idx = index::build_index(dir)?;

    let mut entries: Vec<_> = idx.files.values().map(|entry| {
        let backlinks: Vec<_> = entry.backlinks.iter()
            .map(|p| p.to_str().unwrap_or(""))
            .collect();
        serde_json::json!({
            "path": entry.path.to_str().unwrap_or(""),
            "title": entry.title,
            "tags": entry.tags,
            "backlinks": backlinks,
            "word_count": entry.word_count,
        })
    }).collect();

    // Sort by path
    entries.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));

    let result = serde_json::json!({
        "results": entries,
        "count": entries.len(),
    });

    if pretty {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("{}", serde_json::to_string(&result)?);
    }
    Ok(())
}

fn cmd_query(root: &str, expression: &str, pretty: bool) -> Result<(), Box<dyn std::error::Error>> {
    let idx = index::build_index(root)?;

    let results = query::executor::execute(&idx, expression).map_err(|e| {
        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
    })?;

    let json_results: Vec<_> = results.iter().map(|entry| {
        let backlinks: Vec<_> = entry.backlinks.iter()
            .map(|p: &PathBuf| p.to_str().unwrap_or(""))
            .collect();
        let links: Vec<_> = entry.outgoing_links.iter()
            .map(|l| serde_json::json!({
                "target": l.target.to_str().unwrap_or(""),
                "type": format!("{:?}", l.link_type),
            }))
            .collect();
        serde_json::json!({
            "path": entry.path.to_str().unwrap_or(""),
            "title": entry.title,
            "frontmatter": entry.frontmatter,
            "links": links,
            "backlinks": backlinks,
            "tags": entry.tags,
            "word_count": entry.word_count,
            "size": entry.size,
            "modified": entry.modified.map(|dt: chrono::DateTime<chrono::Utc>| dt.to_rfc3339()),
        })
    }).collect();

    let result = serde_json::json!({
        "results": json_results,
        "count": json_results.len(),
        "query": expression,
    });

    if pretty {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("{}", serde_json::to_string(&result)?);
    }
    Ok(())
}

fn cmd_search(root: &str, text: &str, pretty: bool) -> Result<(), Box<dyn std::error::Error>> {
    let idx = index::build_index(root)?;
    let mut results = Vec::new();

    for entry in idx.files.values() {
        if let Ok(content) = fs::read_to_string(&entry.path) {
            if content.to_lowercase().contains(&text.to_lowercase()) {
                let backlinks: Vec<_> = entry.backlinks.iter()
                    .map(|p| p.to_str().unwrap_or(""))
                    .collect();
                results.push(serde_json::json!({
                    "path": entry.path.to_str().unwrap_or(""),
                    "title": entry.title,
                    "tags": entry.tags,
                    "backlinks": backlinks,
                }));
            }
        }
    }

    let result = serde_json::json!({
        "results": results,
        "count": results.len(),
        "search": text,
    });

    if pretty {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("{}", serde_json::to_string(&result)?);
    }
    Ok(())
}

fn cmd_graph(
    path: Option<&str>,
    full: bool,
    orphans: bool,
    path_between: Option<&[String]>,
    pretty: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let idx = index::build_index(".")?;

    let result = if orphans {
        let orphan_files = commands::graph::orphans(&idx);
        serde_json::json!({
            "orphans": orphan_files,
            "count": orphan_files.len(),
        })
    } else if full {
        let graph = commands::graph::full_graph(&idx);
        serde_json::json!({
            "nodes": graph.nodes.iter().map(|n| serde_json::json!({
                "id": n.id,
                "title": n.title,
                "tags": n.tags,
            })).collect::<Vec<_>>(),
            "edges": graph.edges.iter().map(|e| serde_json::json!({
                "source": e.source,
                "target": e.target,
                "type": e.link_type,
            })).collect::<Vec<_>>(),
        })
    } else if let (Some(pb), Some(from)) = (path_between, path_between.and_then(|pb| pb.first())) {
        let to = path_between.unwrap()[1].as_str();
        match commands::graph::shortest_path(&idx, from, to) {
            Some(path_list) => {
                serde_json::json!({
                    "from": from,
                    "to": to,
                    "path": path_list,
                    "length": path_list.len(),
                })
            }
            None => {
                serde_json::json!({
                    "from": from,
                    "to": to,
                    "path": null,
                    "message": "No path found"
                })
            }
        }
    } else if let Some(p) = path {
        match commands::graph::file_graph(&idx, p) {
            Some(graph) => {
                serde_json::json!({
                    "node": serde_json::json!({
                        "id": graph.nodes.first().map(|n| n.id.clone()),
                        "title": graph.nodes.first().and_then(|n| n.title.clone()),
                    }),
                    "nodes": graph.nodes.iter().map(|n| serde_json::json!({
                        "id": n.id,
                        "title": n.title,
                        "tags": n.tags,
                    })).collect::<Vec<_>>(),
                    "edges": graph.edges.iter().map(|e| serde_json::json!({
                        "source": e.source,
                        "target": e.target,
                        "type": e.link_type,
                    })).collect::<Vec<_>>(),
                })
            }
            None => {
                serde_json::json!({
                    "error": format!("File not found: {}", p)
                })
            }
        }
    } else {
        serde_json::json!({
            "error": "Specify a file path, --full, --orphans, or --path-between"
        })
    };

    if pretty {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("{}", serde_json::to_string(&result)?);
    }
    Ok(())
}

fn cmd_tags(root: &str, pretty: bool) -> Result<(), Box<dyn std::error::Error>> {
    let idx = index::build_index(root)?;

    let tags: Vec<_> = idx.tags.iter().map(|(tag, paths)| {
        serde_json::json!({
            "tag": tag,
            "count": paths.len(),
            "files": paths.iter().map(|p| p.to_str().unwrap_or("")).collect::<Vec<_>>(),
        })
    }).collect();

    let result = serde_json::json!({
        "tags": tags,
        "unique_count": idx.tags.len(),
    });

    if pretty {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("{}", serde_json::to_string(&result)?);
    }
    Ok(())
}

fn cmd_template(action: cli::TemplateAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        cli::TemplateAction::List => {
            let templates = template::engine::builtin_templates();
            let list: Vec<_> = templates.keys().map(|k| k.to_string()).collect();
            let result = serde_json::json!({
                "templates": list,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        cli::TemplateAction::Apply { name, path } => {
            let templates = template::engine::builtin_templates();
            match templates.get(name.as_str()) {
                Some(tmpl) => {
                    let mut vars = std::collections::HashMap::new();
                    vars.insert("title".to_string(), path.clone());
                    vars.insert("date".to_string(), chrono::Utc::now().format("%Y-%m-%d").to_string());
                    vars.insert("status".to_string(), "Proposed".to_string());
                    vars.insert("number".to_string(), "0001".to_string());
                    vars.insert("tag".to_string(), "untagged".to_string());
                    vars.insert("type".to_string(), "added".to_string());
                    vars.insert("description".to_string(), "Description".to_string());

                    let content = template::engine::render(tmpl, &vars);
                    let file_path = PathBuf::from(&path);
                    if let Some(parent) = file_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&file_path, &content)?;

                    let result = serde_json::json!({
                        "status": "ok",
                        "template": name,
                        "path": path,
                    });
                    println!("{}", serde_json::to_string(&result)?);
                }
                None => {
                    let result = serde_json::json!({
                        "status": "error",
                        "message": format!("Template '{}' not found", name),
                    });
                    println!("{}", serde_json::to_string(&result)?);
                }
            }
        }
    }
    Ok(())
}

fn cmd_batch(action: cli::BatchAction) -> Result<(), Box<dyn std::error::Error>> {
    let idx = index::build_index(".")?;

    match action {
        cli::BatchAction::Set { pairs, query, glob, dry_run } => {
            let files = select_files(&idx, query.as_deref(), glob.as_deref())?;
            let mut results = Vec::new();

            for path in &files {
                let mut changes = Vec::new();
                for pair in &pairs {
                    if let Some((key, value)) = pair.split_once('=') {
                        changes.push((key.to_string(), value.to_string()));
                    }
                }

                if dry_run {
                    results.push(serde_json::json!({
                        "path": path,
                        "changes": changes,
                        "action": "would_set",
                    }));
                } else {
                    for (key, value) in &changes {
                        edit_frontmatter(path, key, value)?;
                    }
                    results.push(serde_json::json!({
                        "path": path,
                        "changes": changes,
                        "action": "set",
                    }));
                }
            }

            let result = serde_json::json!({
                "results": results,
                "count": results.len(),
                "dry_run": dry_run,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        cli::BatchAction::Tag { action: tag_action, tag, query, glob, dry_run } => {
            let files = select_files(&idx, query.as_deref(), glob.as_deref())?;
            let mut results = Vec::new();

            for path in &files {
                if dry_run {
                    results.push(serde_json::json!({
                        "path": path,
                        "action": format!("would_{}_tag", tag_action),
                        "tag": tag,
                    }));
                } else {
                    let content = fs::read_to_string(path)?;
                    let fm_result = parser::frontmatter::parse(&content);
                    let body = &content[fm_result.body_start..];
                    let mut frontmatter = fm_result.frontmatter;

                    let current_tags = match frontmatter.get("tags") {
                        Some(serde_yaml::Value::Sequence(seq)) => seq.clone(),
                        _ => Vec::new(),
                    };

                    let new_tags = match tag_action.as_str() {
                        "add" => {
                            let mut t = current_tags;
                            if !t.iter().any(|v| v.as_str() == Some(tag.as_str())) {
                                t.push(serde_yaml::Value::String(tag.clone()));
                            }
                            t
                        }
                        "remove" => {
                            current_tags
                                .into_iter()
                                .filter(|v| v.as_str() != Some(tag.as_str()))
                                .collect()
                        }
                        _ => current_tags,
                    };

                    frontmatter.insert(
                        "tags".to_string(),
                        serde_yaml::Value::Sequence(new_tags),
                    );

                    let new_content = format!(
                        "{}{}",
                        parser::frontmatter::serialize(&frontmatter),
                        body
                    );
                    fs::write(path, new_content)?;

                    results.push(serde_json::json!({
                        "path": path,
                        "action": format!("{}_tag", tag_action),
                        "tag": tag,
                    }));
                }
            }

            let result = serde_json::json!({
                "results": results,
                "count": results.len(),
                "dry_run": dry_run,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        cli::BatchAction::Move { dest, query, dry_run } => {
            let files = select_files(&idx, query.as_deref(), None)?;
            let mut results = Vec::new();

            for path in &files {
                let file_name = std::path::Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or("");
                let new_path = format!("{}/{}", dest.trim_end_matches('/'), file_name);

                if dry_run {
                    results.push(serde_json::json!({
                        "from": path,
                        "to": new_path,
                        "action": "would_move",
                    }));
                } else {
                    let dst = PathBuf::from(&new_path);
                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::rename(path, &dst)?;
                    results.push(serde_json::json!({
                        "from": path,
                        "to": new_path,
                        "action": "moved",
                    }));
                }
            }

            let result = serde_json::json!({
                "results": results,
                "count": results.len(),
                "dry_run": dry_run,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}

fn select_files(
    idx: &models::Index,
    query_expr: Option<&str>,
    glob_pattern: Option<&str>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut files: Vec<String> = if let Some(expr) = query_expr {
        query::executor::execute(idx, expr)
            .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?
            .iter()
            .map(|e| e.path.to_str().unwrap_or("").to_string())
            .collect()
    } else {
        idx.files.keys().map(|p| p.to_str().unwrap_or("").to_string()).collect()
    };

    if let Some(pattern) = glob_pattern {
        let glob = glob::Pattern::new(pattern)
            .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string())))?;
        files.retain(|f| glob.matches(f));
    }

    Ok(files)
}

const FOLIO_DIR: &str = ".folio";

fn cmd_init() -> Result<(), Box<dyn std::error::Error>> {
    let folio_path = PathBuf::from(FOLIO_DIR);
    if folio_path.exists() {
        let result = serde_json::json!({
            "status": "already_exists",
            "path": FOLIO_DIR,
        });
        println!("{}", serde_json::to_string(&result)?);
        return Ok(());
    }

    fs::create_dir_all(format!("{}/plugins", FOLIO_DIR))?;
    fs::create_dir_all(format!("{}/templates", FOLIO_DIR))?;

    let config = r#"# Folio workspace configuration
[workspace]
# Paths to exclude from indexing
exclude = [".git", "node_modules"]

# Custom frontmatter field types
# [fields]
# priority = "string"
# due_date = "date"
"#;
    fs::write(format!("{}/config.toml", FOLIO_DIR), config)?;

    // Add .folio/ to .gitignore if it exists
    let gitignore = PathBuf::from(".gitignore");
    if gitignore.exists() {
        let content = fs::read_to_string(&gitignore)?;
        if !content.contains(".folio") {
            fs::write(&gitignore, format!("{}\n.folio/\n", content.trim_end()))?;
        }
    }

    let result = serde_json::json!({
        "status": "ok",
        "path": FOLIO_DIR,
        "created": ["plugins/", "templates/", "config.toml"],
    });
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn cmd_status() -> Result<(), Box<dyn std::error::Error>> {
    let folio_path = PathBuf::from(FOLIO_DIR);
    let is_workspace = folio_path.exists();

    if !is_workspace {
        let result = serde_json::json!({
            "workspace": false,
            "mode": "stateless",
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    let idx = index::build_index(".")?;

    let result = serde_json::json!({
        "workspace": true,
        "mode": "workspace",
        "folio_dir": FOLIO_DIR,
        "files_indexed": idx.files.len(),
        "unique_tags": idx.tags.len(),
        "total_links": idx.files.values()
            .map(|e| e.outgoing_links.len())
            .sum::<usize>(),
    });
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn cmd_index() -> Result<(), Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    let idx = index::build_index(".")?;
    let elapsed = start.elapsed();

    let result = serde_json::json!({
        "status": "ok",
        "files_indexed": idx.files.len(),
        "unique_tags": idx.tags.len(),
        "elapsed_ms": elapsed.as_millis(),
    });
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn cmd_plugin(action: cli::PluginAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        cli::PluginAction::List => {
            let discovered = plugins::loader::discover();
            let list: Vec<_> = discovered.iter().map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "path": p.path.to_str().unwrap_or(""),
                    "source": format!("{:?}", p.source),
                })
            }).collect();

            let result = serde_json::json!({
                "plugins": list,
                "count": list.len(),
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        cli::PluginAction::Run { name, args } => {
            let discovered = plugins::loader::discover();
            let plugin = discovered.iter().find(|p| p.name == name);

            match plugin {
                Some(p) => {
                    let result = serde_json::json!({
                        "status": "ok",
                        "plugin": name,
                        "args": args,
                        "source": p.path.to_str().unwrap_or(""),
                        "note": "Lua execution not yet implemented - plugin loaded successfully",
                    });
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                None => {
                    let result = serde_json::json!({
                        "status": "error",
                        "message": format!("Plugin '{}' not found", name),
                    });
                    println!("{}", serde_json::to_string(&result)?);
                }
            }
        }
    }
    Ok(())
}
