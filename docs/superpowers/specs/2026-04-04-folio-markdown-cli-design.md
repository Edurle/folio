# Folio — Markdown CLI for AI Agents

## Context

AI agents (Claude Code, etc.) manipulate markdown files via shell commands but lack a unified, structured interface. Current workflows rely on ad-hoc `cat`, `grep`, `sed` calls that are fragile and unstructured. Folio provides a purpose-built CLI that treats markdown file systems as queryable document stores, outputting JSON by default for easy agent consumption.

## Overview

**Folio** is a Rust-based CLI tool for managing markdown files, designed as a general-purpose tool layer with an embedded Lua plugin system. It combines the query power of a document database with the simplicity of flat markdown files.

- **Command name**: `folio`
- **Output format**: JSON by default; `--pretty` for human-readable output
- **Agent interaction**: Agent calls `folio` via shell, parses JSON output
- **Plugin system**: Embedded Lua scripts (via mlua)
- **Workspace**: Optional `.folio/` directory for config, plugins, templates, and persistent index cache

## Architecture

```
folio (CLI binary)
├── Index Engine
│   ├── Scans directory tree → in-memory index on startup
│   ├── Indexes: file paths, frontmatter fields, heading hierarchy, link relationships
│   ├── Incremental updates via checksum comparison
│   └── Persistent cache: .folio/cache.bin (workspace mode only)
├── Query Engine
│   ├── Accepts query expression → parses → executes → JSON output
│   ├── Supports field filtering, full-text search, link traversal, aggregation
│   └── Plugins register custom query functions
├── Document Operations
│   ├── CRUD: create/read/update/delete markdown files
│   ├── Section-level ops: locate by heading, replace specific sections
│   ├── Frontmatter ops: read/modify/merge YAML frontmatter
│   ├── Template engine: variable substitution via templates
│   └── Bulk ops: batch frontmatter, tags, move, template apply (with --dry-run)
├── Plugin Runtime
│   ├── Embedded Lua VM (mlua crate)
│   ├── Hooks: on_index, on_query, on_create, on_update, on_delete
│   ├── Custom subcommand registration
│   └── Sandbox: filesystem access scoped to workspace
└── Workspace (optional)
    ├── folio init → creates .folio/ directory
    ├── .folio/config.toml: workspace configuration
    ├── .folio/plugins/: workspace-level Lua plugins
    ├── .folio/templates/: workspace-level templates
    └── .folio/cache.bin: persistent index cache
```

### Mode Behavior

- **Without workspace**: Folio operates statelessly on the current directory. It builds an in-memory index on each invocation. No config files, no cache.
- **With workspace** (`folio init`): Enables persistent index cache, workspace-level plugins, templates, and configuration.

## CLI Commands

### Document Operations

```bash
folio new <path>                           # Create new markdown file (optionally apply template)
folio cat <path>                           # Output file content as JSON (frontmatter + sections)
folio edit frontmatter <path> <key> <val>  # Modify frontmatter field
folio edit section <path> <heading> <content>  # Replace a section by heading
folio edit append <path> <content>         # Append content to file
folio rm <path>                            # Delete file
folio mv <src> <dst>                       # Move file (auto-update link references)
```

### Query & Search

```bash
folio ls [path]                            # List files (with filter options)
folio query <expression>                   # Run query expression
folio search <text>                        # Full-text search
folio tags                                 # List all tags with counts
folio graph [path]                         # Output link relationship graph
```

### Bulk Operations

```bash
# Batch frontmatter update — apply to files matching a query
folio batch set status='published' --query "status = 'draft' AND tags contains 'rust'"

# Batch tag operations
folio batch tag add 'reviewed' --query "modified > '2026-03-01'"
folio batch tag remove 'wip' --glob "docs/**/*.md"

# Batch move/restructure — reorganize files based on frontmatter
folio batch move --query "type = 'note'" --dest notes/

# Batch template application
folio batch template apply meeting-notes --query "type = 'meeting' AND frontmatter.date > '2026-01-01'"

# Dry run support — preview changes without executing
folio batch set status='archived' --query "age > 365d" --dry-run
```

Bulk operations accept the same query expressions as `folio query`, plus `--glob` patterns for file matching. All batch commands support `--dry-run` to preview changes and `--confirm` to require explicit approval.

### Template

```bash
folio template list                        # List available templates
folio template apply <name> <path>         # Create file from template
```

### Workspace

```bash
folio init                                 # Initialize workspace (.folio/ directory)
folio status                               # Show workspace status
folio index                                # Rebuild index
```

### Plugin

```bash
folio plugin list                          # List loaded plugins
folio plugin run <name> <args>             # Run plugin command
```

### Global Options

```
--json              Force JSON output (default)
--pretty            Human-readable format
--workspace <path>  Specify workspace path
--no-index          Skip indexing, operate directly on files
```

## Query Expression Language

### Syntax

```
<expression> ::= <comparison> (AND <comparison>)* | <comparison> (OR <comparison>)*
<comparison> ::= <field> <op> <value>
<field>      ::= frontmatter.<key> | content | path | title | tags | created | modified
<op>         ::= = | != | > | < | >= | <= | contains | matches | in
<value>      ::= '<string>' | <number> | [<value>, ...]
```

### Examples

```bash
# Frontmatter filtering
folio query "status = 'draft' AND tags contains 'rust'"

# Combined full-text + metadata
folio query "content matches 'async runtime' AND frontmatter.date > '2024-01-01'"

# Link traversal
folio query "linked_from('architecture.md')" --depth 2

# Aggregation
folio query "tags" --aggregate count --group-by month
```

### Output Format

```json
{
  "results": [
    {
      "path": "notes/rust-async.md",
      "frontmatter": {"status": "draft", "tags": ["rust", "async"]},
      "title": "Rust Async Runtime",
      "links": ["notes/tokio.md", "notes/futures.md"],
      "backlinks": ["notes/project-plan.md"],
      "size": 2048,
      "word_count": 450,
      "created": "2026-04-01T10:00:00Z",
      "modified": "2026-04-03T10:30:00Z"
    }
  ],
  "count": 1,
  "query": "status = 'draft' AND tags contains 'rust'"
}
```

## Link Graph

### Supported Link Syntax

- `[[wikilink]]` — standard bidirectional link
- `[[path/to/file|display text]]` — aliased link
- `[text](relative/path.md)` — standard markdown link
- `#tag` — tag (parsed as frontmatter tag or inline tag)

### Graph Operations

```bash
folio graph notes/rust-async.md            # Links for one file
folio graph --full                          # Full workspace graph
folio graph --orphans                       # Files with no links
folio graph --path notes/a.md notes/z.md   # Shortest link path
```

Graph output:

```json
{
  "nodes": [
    {"id": "notes/rust-async.md", "title": "Rust Async Runtime", "tags": ["rust"]},
    {"id": "notes/tokio.md", "title": "Tokio Runtime", "tags": ["rust", "tokio"]}
  ],
  "edges": [
    {"source": "notes/rust-async.md", "target": "notes/tokio.md", "type": "wikilink"}
  ]
}
```

## Plugin System

### Plugin Discovery Order

1. Built-in plugins (compiled into binary)
2. User-level: `~/.config/folio/plugins/*.lua`
3. Workspace-level: `.folio/plugins/*.lua`
4. Later-loaded plugins override earlier ones with the same name

### Plugin Structure

```lua
-- ~/.config/folio/plugins/knowledge_base.lua

return {
  name = "knowledge_base",
  version = "0.1.0",
  description = "Knowledge base management",

  -- Register custom commands
  commands = {
    {
      name = "kb",
      description = "Knowledge base operations",
      handler = function(ctx, args)
        return { entries = ctx:query("type = 'note'") }
      end
    }
  },

  -- Register hooks
  hooks = {
    on_create = function(ctx, path, content)
      ctx:set_frontmatter(path, {
        created = os.date("!%Y-%m-%dT%H:%M:%SZ"),
        id = ctx:uuid()
      })
    end,

    on_index = function(ctx, entry)
      entry.word_count = #entry.content:split(" ")
      return entry
    end
  },

  -- Register custom query functions
  query_functions = {
    recent = function(ctx, days)
      local cutoff = os.time() - days * 86400
      return ctx:where("modified > " .. cutoff)
    end
  }
}
```

### Plugin Context API

Plugins receive a `ctx` object with:

- `ctx:query(expr)` — run a query expression
- `ctx:read(path)` — read file content
- `ctx:write(path, content)` — write file content
- `ctx:set_frontmatter(path, table)` — set frontmatter fields
- `ctx:get_frontmatter(path)` — get frontmatter as table
- `ctx:uuid()` — generate UUID
- `ctx:where(condition)` — filter indexed entries

### Example: Agent Memory Plugin

```lua
-- ~/.config/folio/plugins/agent_memory.lua
-- Provides persistent memory for AI agents

local json = require("json")

return {
  name = "agent_memory",
  version = "0.1.0",
  description = "Persistent memory layer for AI agents",

  commands = {
    {
      name = "remember",
      description = "Store a memory entry",
      handler = function(ctx, args)
        local key = args[1]
        local content = args[2] or ctx:read_stdin()
        local path = "memory/" .. key .. ".md"
        ctx:set_frontmatter(path, {
          type = "memory",
          key = key,
          created = os.date("!%Y-%m-%dT%H:%M:%SZ"),
          agent = args.agent or "unknown"
        })
        ctx:write(path, content)
        return { status = "ok", path = path, key = key }
      end
    },
    {
      name = "recall",
      description = "Retrieve memories by key or search",
      handler = function(ctx, args)
        if args.key then
          return ctx:query("frontmatter.key = '" .. args.key .. "' AND type = 'memory'")
        else
          return ctx:query("content matches '" .. args[1] .. "' AND type = 'memory'")
            :sort_by("modified")
            :limit(args.limit or 10)
        end
      end
    },
    {
      name = "forget",
      description = "Delete a memory entry",
      handler = function(ctx, args)
        ctx:rm("memory/" .. args[1] .. ".md")
        return { status = "ok", forgotten = args[1] }
      end
    }
  },

  hooks = {
    on_create = function(ctx, path, content)
      if path:match("^memory/") then
        ctx:set_frontmatter(path, {
          type = ctx:get_frontmatter(path).type or "memory",
          access_count = 0
        })
      end
    end
  }
}
```

Usage by an agent:
```bash
folio plugin run agent_memory remember "rust-patterns" "Observer pattern in Rust uses trait objects..."
folio plugin run agent_memory recall --key "rust-patterns"
folio plugin run agent_memory recall "async runtime" --limit 5
folio plugin run agent_memory forget "rust-patterns"
```

### Example: Engineering Docs Plugin

```lua
-- ~/.config/folio/plugins/engineering_docs.lua
-- Enforces structure and conventions for project documentation

return {
  name = "engineering_docs",
  version = "0.1.0",
  description = "Project documentation management with ADR support",

  commands = {
    {
      name = "adr",
      description = "Architecture Decision Records",
      handler = function(ctx, args)
        local action = args[1]
        if action == "new" then
          local title = args[2]
          local num = #ctx:query("path matches '^docs/adr/'") + 1
          local padded = string.format("%04d", num)
          local path = "docs/adr/" .. padded .. "-" .. title:gsub(" ", "-"):lower() .. ".md"
          ctx:apply_template("adr", path, {
            number = padded,
            title = title,
            date = os.date("!%Y-%m-%d"),
            status = "Proposed"
          })
          return { status = "ok", path = path, number = num }
        elseif action == "list" then
          return ctx:query("path matches '^docs/adr/'")
            :sort_by("frontmatter.number")
        end
      end
    },
    {
      name = "changelog",
      description = "Generate or update CHANGELOG.md from git-style conventions",
      handler = function(ctx, args)
        local entries = ctx:query("path matches '^changes/'")
          :sort_by("modified", "desc")
          :limit(tonumber(args.limit) or 20)
        local sections = { added = {}, changed = {}, fixed = {}, removed = {} }
        for _, e in ipairs(entries) do
          local t = e.frontmatter.type or "changed"
          table.insert(sections[t], e)
        end
        -- Generate CHANGELOG.md from sections
        local changelog = ctx:render_template("changelog", { sections = sections })
        ctx:write("CHANGELOG.md", changelog)
        return { status = "ok", entries = #entries }
      end
    }
  },

  hooks = {
    on_create = function(ctx, path, content)
      -- Enforce docs/ directory structure
      if path:match("^docs/adr/") then
        local fm = ctx:get_frontmatter(path)
        if not fm.status then
          ctx:set_frontmatter(path, { status = "Proposed" })
        end
      end
      if path:match("^changes/") then
        ctx:set_frontmatter(path, {
          type = ctx:get_frontmatter(path).type or "changed",
          date = os.date("!%Y-%m-%d")
        })
      end
    end,
    on_index = function(ctx, entry)
      -- Track doc freshness
      if entry.path:match("^docs/") then
        local days_since_modified = (os.time() - (entry.modified or 0)) / 86400
        entry.stale = days_since_modified > 90
      end
      return entry
    end
  }
}
```

Usage by an agent:
```bash
folio plugin run engineering_docs adr new "Use Rust for CLI tool"
folio plugin run engineering_docs adr list
folio plugin run engineering_docs changelog --limit 30
```

### Sandbox Restrictions

- Filesystem access scoped to workspace directory
- No `os.execute`, `io.popen`, or other system calls
- No network access
- No loading external Lua modules outside plugin directory

## Index Data Structure

```rust
struct Index {
    files: HashMap<PathBuf, FileEntry>,
    links: petgraph::Graph<PathBuf, LinkType>,
    tags: HashMap<String, Vec<PathBuf>>,
    frontmatter_schema: BTreeMap<String, FieldType>,
}

struct FileEntry {
    path: PathBuf,
    title: Option<String>,
    frontmatter: HashMap<String, serde_yaml::Value>,
    sections: Vec<Section>,
    outgoing_links: Vec<Link>,
    backlinks: Vec<PathBuf>,
    tags: Vec<String>,
    word_count: usize,
    created: Option<chrono::DateTime<chrono::Utc>>,
    modified: Option<chrono::DateTime<chrono::Utc>>,
    checksum: u64,
}

struct Section {
    level: u8,
    heading: String,
    content_range: (usize, usize),  // byte offsets
}

struct Link {
    target: PathBuf,
    line_number: usize,
    link_type: LinkType,  // WikiLink | MarkdownLink | TagReference
}
```

## Technology Stack

| Component | Crate | Reason |
|-----------|-------|--------|
| CLI parsing | `clap` (derive) | Standard Rust CLI framework |
| Markdown parsing | `pulldown-cmark` | Streaming parser, good performance |
| YAML frontmatter | `serde_yaml` | Serde ecosystem compatibility |
| Lua plugins | `mlua` | Mainstream Rust Lua bindings |
| Full-text search | `tantivy` | Native Rust search engine |
| Query expressions | Custom Pratt parser | Lightweight, avoids heavy dependencies |
| JSON output | `serde_json` | Standard |
| File watching | `notify` | Optional incremental indexing |
| Graph data | `petgraph` | Mature graph library |

## Performance Targets

- Index 10,000 files in < 1 second
- Query response in < 50ms for typical queries on indexed data
- Startup time < 100ms for workspace mode (with cached index)

## Project Name

- Binary name: `folio`
- Workspace directory: `.folio/`
- Config directory: `~/.config/folio/`
