use std::fs;
use std::path::PathBuf;

/// A discovered plugin.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub path: PathBuf,
    pub source: PluginSource,
}

#[derive(Debug, Clone)]
pub enum PluginSource {
    User,       // ~/.config/folio/plugins/
    Workspace,  // .folio/plugins/
}

/// Discover plugins from user and workspace directories.
pub fn discover() -> Vec<PluginInfo> {
    let mut plugins = Vec::new();

    // User-level plugins
    let user_dir = dirs_config_path();
    if user_dir.exists() {
        discover_in_dir(&user_dir, PluginSource::User, &mut plugins);
    }

    // Workspace-level plugins
    let ws_dir = PathBuf::from(".folio/plugins");
    if ws_dir.exists() {
        discover_in_dir(&ws_dir, PluginSource::Workspace, &mut plugins);
    }

    plugins
}

fn discover_in_dir(dir: &PathBuf, source: PluginSource, plugins: &mut Vec<PluginInfo>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "lua") {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or("")
                    .to_string();
                plugins.push(PluginInfo { name, path, source: source.clone() });
            }
        }
    }
}

fn dirs_config_path() -> PathBuf {
    // Try XDG_CONFIG_HOME first, then fallback to ~/.config
    if let Ok(config) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(config).join("folio/plugins")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config/folio/plugins")
    }
}

/// Read plugin source code.
pub fn read_plugin(path: &PathBuf) -> Result<String, std::io::Error> {
    fs::read_to_string(path)
}
