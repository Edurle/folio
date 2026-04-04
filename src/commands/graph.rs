use std::collections::{HashMap, VecDeque};

use crate::models::Index;

/// Graph operations result types.
pub struct GraphResult {
    pub nodes: Vec<NodeInfo>,
    pub edges: Vec<EdgeInfo>,
}

pub struct NodeInfo {
    pub id: String,
    pub title: Option<String>,
    pub tags: Vec<String>,
}

pub struct EdgeInfo {
    pub source: String,
    pub target: String,
    pub link_type: String,
}

/// Get the full graph for all files in the index.
pub fn full_graph(index: &Index) -> GraphResult {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for (path, entry) in &index.files {
        let id = path.to_str().unwrap_or("").to_string();
        nodes.push(NodeInfo {
            id: id.clone(),
            title: entry.title.clone(),
            tags: entry.tags.clone(),
        });

        for link in &entry.outgoing_links {
            edges.push(EdgeInfo {
                source: id.clone(),
                target: link.target.to_str().unwrap_or("").to_string(),
                link_type: format!("{:?}", link.link_type),
            });
        }
    }

    GraphResult { nodes, edges }
}

/// Get graph info for a single file.
pub fn file_graph(index: &Index, path: &str) -> Option<GraphResult> {
    let path_buf = std::path::PathBuf::from(path);
    let entry = index.files.get(&path_buf)?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Add the main node
    let id = path.to_string();
    nodes.push(NodeInfo {
        id: id.clone(),
        title: entry.title.clone(),
        tags: entry.tags.clone(),
    });

    // Add outgoing link nodes and edges
    for link in &entry.outgoing_links {
        let target_id = link.target.to_str().unwrap_or("").to_string();
        edges.push(EdgeInfo {
            source: id.clone(),
            target: target_id.clone(),
            link_type: format!("{:?}", link.link_type),
        });

        if let Some(target_entry) = index.files.get(&link.target) {
            nodes.push(NodeInfo {
                id: target_id,
                title: target_entry.title.clone(),
                tags: target_entry.tags.clone(),
            });
        }
    }

    // Add backlink nodes and edges
    for backlink in &entry.backlinks {
        let source_id = backlink.to_str().unwrap_or("").to_string();
        edges.push(EdgeInfo {
            source: source_id.clone(),
            target: id.clone(),
            link_type: "backlink".to_string(),
        });

        if let Some(source_entry) = index.files.get(backlink) {
            nodes.push(NodeInfo {
                id: source_id,
                title: source_entry.title.clone(),
                tags: source_entry.tags.clone(),
            });
        }
    }

    Some(GraphResult { nodes, edges })
}

/// Find files with no links (orphans).
pub fn orphans(index: &Index) -> Vec<&str> {
    index
        .files
        .values()
        .filter(|entry| entry.outgoing_links.is_empty() && entry.backlinks.is_empty())
        .map(|entry| entry.path.to_str().unwrap_or(""))
        .collect()
}

/// Find shortest path between two files via links (BFS).
pub fn shortest_path(index: &Index, from: &str, to: &str) -> Option<Vec<String>> {
    let from_path = std::path::PathBuf::from(from);
    let to_path = std::path::PathBuf::from(to);

    if !index.files.contains_key(&from_path) || !index.files.contains_key(&to_path) {
        return None;
    }

    // Build adjacency list (bidirectional)
    let mut adj: HashMap<&std::path::PathBuf, Vec<&std::path::PathBuf>> = HashMap::new();
    for (path, entry) in &index.files {
        for link in &entry.outgoing_links {
            if index.files.contains_key(&link.target) {
                adj.entry(path).or_default().push(&link.target);
                adj.entry(&link.target).or_default().push(path);
            }
        }
    }

    // BFS
    let mut queue: VecDeque<(&std::path::PathBuf, Vec<String>)> = VecDeque::new();
    let mut visited: std::collections::HashSet<&std::path::PathBuf> = std::collections::HashSet::new();

    queue.push_back((&from_path, vec![from.to_string()]));
    visited.insert(&from_path);

    while let Some((current, path)) = queue.pop_front() {
        if current == &to_path {
            return Some(path);
        }

        if let Some(neighbors) = adj.get(current) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    visited.insert(neighbor);
                    let mut new_path = path.clone();
                    new_path.push(neighbor.to_str().unwrap_or("").to_string());
                    queue.push_back((neighbor, new_path));
                }
            }
        }
    }

    None
}
