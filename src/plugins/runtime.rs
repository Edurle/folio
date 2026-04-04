use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use mlua::{Function, Lua, Result as LuaResult, Table, Value};

/// Execute a Lua plugin with given command and arguments.
pub fn run_plugin(
    plugin_path: &PathBuf,
    command: &str,
    args: &[String],
    workspace_root: &str,
) -> LuaResult<String> {
    let lua = Lua::new();

    // Create the ctx (context) object available to plugins
    let ctx = create_context(&lua, workspace_root)?;

    // Load and execute the plugin script
    let source = fs::read_to_string(plugin_path)
        .map_err(|e| mlua::Error::runtime(format!("Failed to read plugin: {}", e)))?;
    let plugin_table: Table = lua.load(&source).eval()?;

    // Find the command handler
    let commands: Table = plugin_table.get("commands")?;
    let mut handler: Option<Function> = None;

    for pair in commands.sequence_values::<Table>() {
        let cmd = pair?;
        let name: String = cmd.get("name")?;
        if name == command {
            handler = Some(cmd.get("handler")?);
            break;
        }
    }

    let handler = handler
        .ok_or_else(|| mlua::Error::runtime(format!("Command '{}' not found in plugin", command)))?;

    // Build args table
    let args_table = lua.create_table()?;
    for (i, arg) in args.iter().enumerate() {
        args_table.set(i + 1, arg.as_str())?;
    }

    // Call the handler
    let result = handler.call::<Value>((ctx, args_table))?;

    // Serialize result to JSON string
    match result {
        Value::Nil => Ok("{}".to_string()),
        Value::String(s) => Ok(s.to_str()?.to_string()),
        Value::Table(t) => {
            // Convert Lua table to a simple JSON representation
            Ok(table_to_json(&t))
        }
        _ => Ok(format!("{:?}", result)),
    }
}

/// List all commands provided by a plugin.
pub fn list_commands(plugin_path: &PathBuf) -> LuaResult<Vec<(String, String)>> {
    let lua = Lua::new();
    let source = fs::read_to_string(plugin_path)
        .map_err(|e| mlua::Error::runtime(format!("Failed to read plugin: {}", e)))?;
    let plugin_table: Table = lua.load(&source).eval()?;

    let commands: Table = plugin_table.get("commands")?;
    let mut result = Vec::new();

    for pair in commands.sequence_values::<Table>() {
        let cmd = pair?;
        let name: String = cmd.get("name")?;
        let desc: String = cmd.get("description").unwrap_or_else(|_| String::new());
        result.push((name, desc));
    }

    Ok(result)
}

fn create_context(lua: &Lua, workspace_root: &str) -> LuaResult<Table> {
    let ctx = lua.create_table()?;

    let root = workspace_root.to_string();
    let root2 = root.clone();
    let root3 = root.clone();
    let root4 = root.clone();
    let root5 = root.clone();
    let root6 = root.clone();
    let root7 = root.clone();

    // ctx:read(path) - read file content
    ctx.set(
        "read",
        lua.create_function(move |_lua, (_self, path): (mlua::Value, String)| {
            let full_path = PathBuf::from(&root).join(&path);
            fs::read_to_string(&full_path)
                .map_err(|e| mlua::Error::runtime(format!("Read error: {}", e)))
        })?,
    )?;

    // ctx:write(path, content) - write file
    ctx.set(
        "write",
        lua.create_function(move |_lua, (_self, path, content): (mlua::Value, String, String)| {
            let full_path = PathBuf::from(&root2).join(&path);
            if let Some(parent) = full_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&full_path, &content)
                .map_err(|e| mlua::Error::runtime(format!("Write error: {}", e)))
        })?,
    )?;

    // ctx:exists(path) - check if file exists
    ctx.set(
        "exists",
        lua.create_function(move |_lua, (_self, path): (mlua::Value, String)| {
            let full_path = PathBuf::from(&root3).join(&path);
            Ok(full_path.exists())
        })?,
    )?;

    // ctx:rm(path) - delete file
    ctx.set(
        "rm",
        lua.create_function(move |_lua, (_self, path): (mlua::Value, String)| {
            let full_path = PathBuf::from(&root4).join(&path);
            fs::remove_file(&full_path)
                .map_err(|e| mlua::Error::runtime(format!("Remove error: {}", e)))
        })?,
    )?;

    // ctx:ls(dir) - list files in directory
    ctx.set(
        "ls",
        lua.create_function(move |lua, (_self, dir): (mlua::Value, String)| {
            let full_path = PathBuf::from(&root5).join(&dir);
            let mut files = Vec::new();
            if full_path.is_dir() {
                if let Ok(entries) = fs::read_dir(&full_path) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            files.push(name.to_string());
                        }
                    }
                }
            }
            let table = lua.create_table()?;
            for (i, f) in files.iter().enumerate() {
                table.set(i + 1, f.as_str())?;
            }
            Ok(table)
        })?,
    )?;

    // ctx:glob(pattern) - simple glob matching
    ctx.set(
        "glob",
        lua.create_function(move |lua, (_self, pattern): (mlua::Value, String)| {
            let full_pattern = PathBuf::from(&root6).join(&pattern);
            let mut files = Vec::new();
            if let Ok(paths) = glob::glob(full_pattern.to_str().unwrap_or("")) {
                for entry in paths.flatten() {
                    if let Some(s) = entry.to_str() {
                        let relative = s.strip_prefix(&root6).unwrap_or(s);
                        let relative = relative.trim_start_matches('/');
                        files.push(relative.to_string());
                    }
                }
            }
            let table = lua.create_table()?;
            for (i, f) in files.iter().enumerate() {
                table.set(i + 1, f.as_str())?;
            }
            Ok(table)
        })?,
    )?;

    // ctx:uuid() - generate UUID
    ctx.set(
        "uuid",
        lua.create_function(move |_lua, _self: mlua::Value| {
            Ok(uuid::Uuid::new_v4().to_string())
        })?,
    )?;

    // ctx:now() - current UTC time in ISO format
    ctx.set(
        "now",
        lua.create_function(move |_lua, _self: mlua::Value| {
            Ok(chrono::Utc::now().to_rfc3339())
        })?,
    )?;

    // ctx:read_stdin() - not available in plugin context
    // ctx:query() - would need index integration, skip for now

    Ok(ctx)
}

/// Simple Lua table to JSON conversion.
fn table_to_json(table: &Table) -> String {
    let mut map = HashMap::new();
    for pair in table.pairs::<String, Value>() {
        if let Ok((k, v)) = pair {
            map.insert(k, value_to_json(v));
        }
    }
    // Also handle array-like tables
    let mut arr = Vec::new();
    for val in table.sequence_values::<Value>() {
        if let Ok(v) = val {
            arr.push(value_to_json(v));
        }
    }

    if !arr.is_empty() && map.is_empty() {
        format!("[{}]", arr.join(","))
    } else if !map.is_empty() {
        let entries: Vec<String> = map.iter().map(|(k, v)| format!("\"{}\":{}", k, v)).collect();
        format!("{{{}}}", entries.join(","))
    } else {
        "{}".to_string()
    }
}

fn value_to_json(val: Value) -> String {
    match val {
        Value::Nil => "null".to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(n) => n.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            let owned: String = s.to_str().map(|b| b.to_string()).unwrap_or_default();
            let escaped = owned.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{}\"", escaped)
        }
        Value::Table(t) => table_to_json(&t),
        _ => "null".to_string(),
    }
}
