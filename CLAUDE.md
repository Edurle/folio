# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test

```bash
cargo build                  # debug build
cargo build --release        # optimized build (~3.4MB binary)
cargo test                   # run all tests (29 tests)
cargo test query::           # run only query module tests
cargo test parser::          # run only parser module tests
cargo test index::           # run only index module tests
cargo run -- --help          # run CLI with help output
cargo run -- <command>       # run any folio command
```

No external build tools or system dependencies beyond the Rust toolchain. The `mlua` crate vendors LuaJIT automatically.

## Architecture

Folio is a CLI tool that treats a directory of markdown files as a queryable document store, outputting JSON by default (use `--pretty` for human-readable output). Five subsystems work together:

**Index Engine** (`src/index/`) — Scans the workspace directory tree on startup, parsing each `.md` file into `FileEntry` structs stored in an in-memory `Index` (HashMap keyed by PathBuf). Builds tag indexes and backlink maps from outgoing links. Supports incremental updates via checksums. Workspace mode persists cache to `.folio/cache.bin`.

**Parser Pipeline** (`src/parser/`) — Two-stage: `frontmatter.rs` extracts YAML between `---` delimiters into `HashMap<String, Value>`, then `markdown.rs` parses the body with pulldown-cmark to extract headings/sections, wikilinks (`[[target]]`, `[[path|alias]]`), standard markdown links, and `#tags`.

**Query Engine** (`src/query/`) — Custom expression language with lexer → Pratt parser → executor pipeline. Supports field comparisons (`frontmatter.status = 'draft'`), full-text search (`content matches 'pattern'`), tag queries, and logical AND/OR. The AST evaluator filters against the in-memory Index.

**Plugin Runtime** (`src/plugins/`) — Embedded Lua VM via `mlua` with LuaJIT. Plugins are `.lua` files discovered from `~/.config/folio/plugins/` (user-level) and `.folio/plugins/` (workspace-level). Plugins can hook into `on_index`, `on_query`, `on_create`, `on_update`, `on_delete` lifecycle events and register custom subcommands. Sandboxed: no `os.execute`, `io.popen`, network, or external module loading. Filesystem access is scoped to workspace root.

**Template Engine** (`src/template/`) — Simple variable substitution from `.folio/templates/`.

## Source Layout

- `src/main.rs` — Entry point and all command handler functions
- `src/cli.rs` — CLI argument definitions using clap derive macros
- `src/models.rs` — Core data structures (`FileEntry`, `Index`, `Link`, `Section`)
- `src/commands/graph.rs` — Link graph analysis using petgraph
- `src/index/` — Scanner (`scanner.rs`), builder (`builder.rs`), orchestration (`mod.rs`)
- `src/query/` — Lexer, Pratt parser, and AST executor
- `src/parser/` — Frontmatter and markdown parsing
- `src/plugins/` — Plugin loader and Lua runtime with context API
- `plugins/` — Example Lua plugins (`agent_memory.lua` demonstrates two-layer AI memory)

## Key Design Decisions

- **JSON-first output**: All commands output JSON by default for agent consumption; `--pretty` flag for humans
- **Query language**: Custom DSL with Pratt parser rather than SQL/jq — keeps the binary self-contained
- **Lua plugins**: Chosen for sandboxing control and small runtime footprint vs WASM
- **In-memory index**: No database dependency; full index rebuilt on each invocation, with optional persistent cache in workspace mode
- **Rust 2024 edition**: Uses latest edition features
