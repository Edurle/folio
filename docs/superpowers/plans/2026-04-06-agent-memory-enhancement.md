# Agent Memory Management Enhancement Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enhance the agent memory plugin with typed memory routing, three-layer lifecycle (working/long-term/archive), and improved retrieval.

**Architecture:** The plugin gains a `--type` flag that routes memories to type-specific subdirectories (decision → `memory/long-term/decisions/`, etc.). A new `archive` layer holds old long-term memories. All search/recall operations use recursive `ctx:glob()` to find files across nested directories. Smart consolidate and archive use date-based filtering.

**Tech Stack:** Rust (templates in `src/template/engine.rs`), Lua (plugin in `plugins/agent_memory.lua`), mlua plugin runtime (`src/plugins/runtime.rs`)

---

## File Structure

| File | Change | Responsibility |
|------|--------|---------------|
| `src/template/engine.rs` | Modify | Add 6 memory type templates to `builtin_templates()` |
| `plugins/agent_memory.lua` | Rewrite | Enhanced three-layer typed memory plugin (v0.2.0) |
| `~/.claude/skills/folio-cli/SKILL.md` | Modify | Updated memory plugin section |
| `~/.claude/skills/folio-cli/references/commands.md` | Modify | Updated plugin commands reference |

---

### Task 1: Add Memory Type Templates

**Files:**
- Modify: `src/template/engine.rs:13-28` (`builtin_templates` function)
- Test: `src/template/engine.rs:30-52` (inline tests module)

- [ ] **Step 1: Add test for new templates**

In `src/template/engine.rs`, add after the existing `test_builtin_templates` test (line 51):

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_memory_type_templates`
Expected: FAIL — template keys don't exist yet

- [ ] **Step 3: Add templates to `builtin_templates()`**

In `src/template/engine.rs`, add after the `templates.insert("changelog-entry", ...)` line (after line 25, before the closing `templates`):

```rust
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
```

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/template/engine.rs
git commit -m "feat(template): add 6 memory type templates for agent memory"
```

---

### Task 2: Rewrite agent_memory.lua Plugin

**Files:**
- Rewrite: `plugins/agent_memory.lua`

Replace the entire plugin with the enhanced version. Changes:
- `--type` flag routes memories to type-specific subdirectories
- Three layers: working, long-term, archive (new)
- Recursive `scan_all_memories()` helper using `ctx:glob()`
- `recall` searches all layers/subdirs, sorted by recency
- `forget` searches all layers/subdirs via glob
- `list` shows type info from frontmatter
- `clear` supports archive layer
- Smart `consolidate` uses date-based filtering
- New `archive` command (long-term → archive)
- New `stats` command (usage statistics)

- [ ] **Step 1: Write the new plugin file**

Replace the entire content of `plugins/agent_memory.lua` with:

```lua
-- Folio Agent Memory Plugin v0.2.0
-- Three-layer typed memory system for AI agents.
--
-- Layers:
--   working:   memory/working/        - session-level, cleared between sessions
--   long-term: memory/long-term/      - persistent, organized by type subdirs
--   archive:   memory/archive/        - old memories, preserved but rarely accessed
--
-- Memory types (--type flag) and their default locations:
--   decision   -> memory/long-term/decisions/
--   error      -> memory/long-term/errors/
--   preference -> memory/long-term/preferences/
--   fact       -> memory/long-term/facts/
--   procedure  -> memory/long-term/procedures/
--   task       -> memory/working/tasks/
--
-- Commands:
--   remember <key> [--type T] [--layer L] [--content "..."] [--tags "t1,t2"]
--   recall <key> | --search "text" [--type T] [--layer L] [--limit N]
--   forget <key>
--   clear --layer working|long-term|archive
--   list [--layer L] [--type T]
--   consolidate [--older-than Nd]
--   archive [--older-than Nd]
--   stats

-- ============================================================
-- Type routing: maps type name to default layer and subdirectory
-- ============================================================
local TYPE_DIRS = {
    decision   = { layer = "long-term", subdir = "decisions" },
    error      = { layer = "long-term", subdir = "errors" },
    preference = { layer = "long-term", subdir = "preferences" },
    fact       = { layer = "long-term", subdir = "facts" },
    procedure  = { layer = "long-term", subdir = "procedures" },
    task       = { layer = "working",   subdir = "tasks" },
}

-- ============================================================
-- Utility functions
-- ============================================================

local function parse_args(args)
    local parsed = {
        positional = {},
        flags = {},
    }
    local i = 1
    while i <= #args do
        local arg = args[i]
        if arg:sub(1, 2) == "--" then
            local key = arg:sub(3)
            local value = args[i + 1]
            if value and value:sub(1, 2) ~= "--" then
                parsed[key] = value
                i = i + 2
            else
                parsed[key] = true
                i = i + 1
            end
        else
            table.insert(parsed.positional, arg)
            i = i + 1
        end
    end
    return parsed
end

local function frontmatter_string(data)
    local lines = { "---" }
    local keys = {}
    for k in pairs(data) do table.insert(keys, k) end
    table.sort(keys)
    for _, k in ipairs(keys) do
        local v = data[k]
        if type(v) == "table" then
            table.insert(lines, k .. ":")
            for _, item in ipairs(v) do
                table.insert(lines, "  - " .. tostring(item))
            end
        elseif type(v) == "string" then
            if v:find("[:#{}[%],&*?|>!%@`]") or v == "" then
                table.insert(lines, k .. ': "' .. v .. '"')
            else
                table.insert(lines, k .. ": " .. v)
            end
        elseif type(v) == "number" then
            table.insert(lines, k .. ": " .. tostring(v))
        elseif type(v) == "boolean" then
            table.insert(lines, k .. ": " .. tostring(v))
        else
            table.insert(lines, k .. ': "' .. tostring(v) .. '"')
        end
    end
    table.insert(lines, "---")
    return table.concat(lines, "\n") .. "\n"
end

--- Parse simple key: value pairs from YAML frontmatter.
--- Returns a flat table of string key-value pairs.
--- Does not handle nested structures or lists.
local function parse_frontmatter(content)
    local fm = {}
    if not content or content:sub(1, 3) ~= "---" then return fm end
    local end_pos = content:find("---", 4, true)
    if not end_pos then return fm end
    local yaml = content:sub(4, end_pos - 1)
    for line in yaml:gmatch("[^\n]+") do
        local key, value = line:match("^%s*(%a[%w_]*)%s*:%s*(.+)$")
        if key and value then
            value = value:match('^"(.*)"$') or value:match("^'(.*)'$") or value
            value = value:match("^%s*(.-)%s*$")
            fm[key] = value
        end
    end
    return fm
end

--- Resolve the file path for a memory entry.
--- With type: memory/<layer>/<subdir>/<key>.md
--- Without type: memory/<layer>/<key>.md (backward compatible)
local function resolve_path(layer, mem_type, key)
    if mem_type and TYPE_DIRS[mem_type] then
        local info = TYPE_DIRS[mem_type]
        local effective_layer = layer or info.layer
        return "memory/" .. effective_layer .. "/" .. info.subdir .. "/" .. key .. ".md"
    end
    return "memory/" .. layer .. "/" .. key .. ".md"
end

--- Convert ISO date prefix (YYYY-MM-DD) to approximate day number.
local function to_day_num(iso_date)
    if not iso_date then return 0 end
    local y, m, d = iso_date:match("(%d%d%d%d)%-(%d%d)%-(%d%d)")
    if not y then return 0 end
    return tonumber(y) * 365 + tonumber(m) * 30 + tonumber(d)
end

--- Scan all memory files recursively via glob.
--- Returns a list of tables: { key, layer, type, path, frontmatter, preview }.
--- opts: { layer?, mem_type?, search?, limit? }
local function scan_all_memories(ctx, opts)
    opts = opts or {}
    local all_files = ctx:glob("memory/**/*.md")
    local results = {}

    for _, f in ipairs(all_files) do
        if opts.limit and #results >= opts.limit then break end

        local ok, content = pcall(function() return ctx:read(f) end)
        if not ok or not content then goto continue end

        local fm = parse_frontmatter(content)

        -- Apply filters
        if opts.layer and fm.layer ~= opts.layer then goto continue end
        if opts.mem_type and fm.type ~= opts.mem_type then goto continue end
        if opts.search then
            if not content:lower():find(opts.search:lower(), 1, true) then
                goto continue
            end
        end

        table.insert(results, {
            key = fm.key or f:match("([^/]+)%.md$"),
            layer = fm.layer,
            type = fm.type,
            path = f,
            frontmatter = fm,
            preview = content:sub(1, 200),
        })

        ::continue::
    end

    return results
end

--- Sort by modified date (newest first), then access_count (highest first).
local function sort_by_relevance(a, b)
    local a_mod = a.frontmatter.modified or ""
    local b_mod = b.frontmatter.modified or ""
    if a_mod ~= b_mod then return a_mod > b_mod end
    local a_acc = tonumber(a.frontmatter.access_count) or 0
    local b_acc = tonumber(b.frontmatter.access_count) or 0
    return a_acc > b_acc
end

--- Parse a duration string like "7d" into days (default 1).
local function parse_days(s)
    return tonumber(s and s:match("(%d+)d")) or 1
end

-- ============================================================
-- Plugin module
-- ============================================================

return {
    name = "agent_memory",
    version = "0.2.0",
    description = "Three-layer typed memory system for AI agents",

    commands = {
        {
            name = "help",
            description = "Show available commands",
            handler = function(ctx, args)
                return {
                    status = "ok",
                    plugin = "agent_memory",
                    version = "0.2.0",
                    commands = {
                        { name = "remember", description = "Store memory (--type, --layer, --content, --tags)" },
                        { name = "recall", description = "Retrieve by key, --search, --type, --layer" },
                        { name = "forget", description = "Delete a memory entry" },
                        { name = "clear", description = "Clear all memories in --layer" },
                        { name = "list", description = "List memories [--layer, --type]" },
                        { name = "consolidate", description = "working -> long-term [--older-than Nd]" },
                        { name = "archive", description = "long-term -> archive [--older-than Nd]" },
                        { name = "stats", description = "Show memory usage statistics" },
                    }
                }
            end,
        },
        {
            name = "remember",
            description = "Store a memory entry",
            handler = function(ctx, args)
                local opts = parse_args(args)
                local key = opts.positional[1] or ctx:uuid()
                local mem_type = opts.type
                local layer = opts.layer
                local content = opts.content or ""
                local tags_str = opts.tags or ""
                local agent = opts.agent or "unknown"

                -- Determine effective layer from type if not explicitly set
                if not layer and mem_type and TYPE_DIRS[mem_type] then
                    layer = TYPE_DIRS[mem_type].layer
                end
                layer = layer or "long-term"

                -- Parse tags
                local tags = {}
                if tags_str ~= "" then
                    for tag in tags_str:gmatch("[^,]+") do
                        table.insert(tags, tag:match("^%s*(.-)%s*$"))
                    end
                end
                table.insert(tags, "memory:" .. layer)
                if mem_type then
                    table.insert(tags, "memory-type:" .. mem_type)
                end

                local path = resolve_path(layer, mem_type, key)
                local now = ctx:now()

                -- Preserve created date and increment access_count on re-write
                local access_count = 0
                local created = now
                if ctx:exists(path) then
                    local existing = ctx:read(path)
                    local existing_fm = parse_frontmatter(existing)
                    access_count = (tonumber(existing_fm.access_count) or 0) + 1
                    created = existing_fm.created or now
                end

                local fm = {
                    type = mem_type or "memory",
                    key = key,
                    layer = layer,
                    agent = agent,
                    tags = tags,
                    created = created,
                    modified = now,
                    access_count = access_count,
                }

                local file_content = frontmatter_string(fm) .. "\n# " .. key .. "\n\n" .. content .. "\n"
                ctx:write(path, file_content)

                return {
                    status = "ok",
                    action = "remembered",
                    key = key,
                    type = fm.type,
                    layer = layer,
                    path = path,
                    tags = tags,
                }
            end,
        },
        {
            name = "recall",
            description = "Retrieve memories by key, type, or search",
            handler = function(ctx, args)
                local opts = parse_args(args)
                local key = opts.positional[1]
                local layer = opts.layer
                local mem_type = opts.type
                local search = opts.search
                local limit = tonumber(opts.limit) or 10

                -- Specific key lookup (without --search or --type filter mode)
                if key and not search and not mem_type then
                    local matches = ctx:glob("memory/**/" .. key .. ".md")
                    local candidates = {}
                    for _, f in ipairs(matches) do
                        local ok, content = pcall(function() return ctx:read(f) end)
                        if ok and content then
                            local fm = parse_frontmatter(content)
                            if (not layer or fm.layer == layer) then
                                table.insert(candidates, { path = f, fm = fm, content = content })
                            end
                        end
                    end
                    if #candidates > 0 then
                        -- Prefer working > long-term > archive
                        local layer_order = { working = 1, ["long-term"] = 2, archive = 3 }
                        table.sort(candidates, function(a, b)
                            return (layer_order[a.fm.layer] or 9) < (layer_order[b.fm.layer] or 9)
                        end)
                        local best = candidates[1]
                        return {
                            status = "ok",
                            key = key,
                            layer = best.fm.layer,
                            type = best.fm.type,
                            path = best.path,
                            content = best.content,
                        }
                    end
                    return {
                        status = "not_found",
                        key = key,
                        message = "Memory '" .. key .. "' not found in any layer",
                    }
                end

                -- Search/filter mode
                local results = scan_all_memories(ctx, {
                    layer = layer,
                    mem_type = mem_type,
                    search = search,
                    limit = limit,
                })

                table.sort(results, sort_by_relevance)

                return {
                    status = "ok",
                    results = results,
                    count = #results,
                    search = search,
                    type = mem_type,
                    layer = layer,
                }
            end,
        },
        {
            name = "forget",
            description = "Delete a memory entry",
            handler = function(ctx, args)
                local opts = parse_args(args)
                local key = opts.positional[1]
                if not key then
                    return { status = "error", message = "Key required: forget <key>" }
                end

                local matches = ctx:glob("memory/**/" .. key .. ".md")
                local removed = {}
                for _, f in ipairs(matches) do
                    local ok, content = pcall(function() return ctx:read(f) end)
                    if ok and content then
                        local fm = parse_frontmatter(content)
                        if (not opts.layer or fm.layer == opts.layer) then
                            ctx:rm(f)
                            table.insert(removed, { path = f, layer = fm.layer })
                        end
                    end
                end

                if #removed > 0 then
                    return {
                        status = "ok",
                        action = "forgotten",
                        key = key,
                        removed = removed,
                    }
                end

                return {
                    status = "not_found",
                    key = key,
                    message = "Memory '" .. key .. "' not found",
                }
            end,
        },
        {
            name = "clear",
            description = "Clear all memories in a layer",
            handler = function(ctx, args)
                local opts = parse_args(args)
                local layer = opts.layer or "working"

                if layer ~= "working" and layer ~= "long-term" and layer ~= "archive" then
                    return { status = "error", message = "Layer must be 'working', 'long-term', or 'archive'" }
                end

                local files = ctx:glob("memory/" .. layer .. "/**/*.md")
                -- Also match flat files directly in the layer directory
                local flat = ctx:glob("memory/" .. layer .. "/*.md")
                for _, f in ipairs(flat) do
                    table.insert(files, f)
                end

                local removed = 0
                local seen = {}
                for _, f in ipairs(files) do
                    if not seen[f] then
                        seen[f] = true
                        ctx:rm(f)
                        removed = removed + 1
                    end
                end

                return {
                    status = "ok",
                    action = "cleared",
                    layer = layer,
                    removed = removed,
                }
            end,
        },
        {
            name = "list",
            description = "List memories in a layer",
            handler = function(ctx, args)
                local opts = parse_args(args)
                local layer = opts.layer
                local mem_type = opts.type

                local results = scan_all_memories(ctx, {
                    layer = layer,
                    mem_type = mem_type,
                })

                -- Return simplified list (no full preview)
                local list = {}
                for _, r in ipairs(results) do
                    table.insert(list, {
                        key = r.key,
                        layer = r.layer,
                        type = r.type,
                        path = r.path,
                        modified = r.frontmatter.modified,
                    })
                end

                return {
                    status = "ok",
                    memories = list,
                    count = #list,
                }
            end,
        },
        {
            name = "consolidate",
            description = "Move working memories to long-term (age-based)",
            handler = function(ctx, args)
                local opts = parse_args(args)
                local older_than = opts["older-than"] or "1d"
                local days = parse_days(older_than)
                local now = ctx:now()
                local now_day = to_day_num(now)

                local files = ctx:glob("memory/working/**/*.md")
                local flat = ctx:glob("memory/working/*.md")
                for _, f in ipairs(flat) do table.insert(files, f) end

                local moved = 0
                local skipped = 0
                local seen = {}

                for _, f in ipairs(files) do
                    if seen[f] then goto continue end
                    seen[f] = true

                    local ok, content = pcall(function() return ctx:read(f) end)
                    if not ok or not content then goto continue end

                    local fm = parse_frontmatter(content)
                    local modified = fm.modified or ""
                    local modified_day = to_day_num(modified)
                    local age_days = now_day - modified_day

                    if age_days >= days then
                        -- Preserve subdirectory structure: memory/working/X -> memory/long-term/X
                        local relative = f:gsub("^memory/working/", "")
                        local dst = "memory/long-term/" .. relative

                        -- Update layer in frontmatter
                        local new_content = content:gsub("layer: working", "layer: long-term", 1)
                        ctx:write(dst, new_content)
                        ctx:rm(f)
                        moved = moved + 1
                    else
                        skipped = skipped + 1
                    end

                    ::continue::
                end

                return {
                    status = "ok",
                    action = "consolidated",
                    moved = moved,
                    skipped = skipped,
                    older_than = older_than,
                }
            end,
        },
        {
            name = "archive",
            description = "Move old long-term memories to archive",
            handler = function(ctx, args)
                local opts = parse_args(args)
                local older_than = opts["older-than"] or "30d"
                local days = parse_days(older_than)
                local now = ctx:now()
                local now_day = to_day_num(now)

                local files = ctx:glob("memory/long%-term/**/*.md")
                local flat = ctx:glob("memory/long%-term/*.md")
                for _, f in ipairs(flat) do table.insert(files, f) end

                local moved = 0
                local skipped = 0
                local seen = {}

                for _, f in ipairs(files) do
                    if seen[f] then goto continue end
                    seen[f] = true

                    local ok, content = pcall(function() return ctx:read(f) end)
                    if not ok or not content then goto continue end

                    local fm = parse_frontmatter(content)
                    local modified = fm.modified or ""
                    local modified_day = to_day_num(modified)
                    local age_days = now_day - modified_day

                    if age_days >= days then
                        -- memory/long-term/X -> memory/archive/X
                        local relative = f:gsub("^memory/long%-term/", "")
                        local dst = "memory/archive/" .. relative

                        local new_content = content:gsub("layer: long%-term", "layer: archive", 1)
                        ctx:write(dst, new_content)
                        ctx:rm(f)
                        moved = moved + 1
                    else
                        skipped = skipped + 1
                    end

                    ::continue::
                end

                return {
                    status = "ok",
                    action = "archived",
                    moved = moved,
                    skipped = skipped,
                    older_than = older_than,
                }
            end,
        },
        {
            name = "stats",
            description = "Show memory usage statistics",
            handler = function(ctx, args)
                local layer_names = { "working", "long-term", "archive" }
                local layer_stats = {}
                local total = 0
                local type_counts = {}

                for _, l in ipairs(layer_names) do
                    local files = ctx:glob("memory/" .. l .. "/**/*.md")
                    local flat = ctx:glob("memory/" .. l .. "/*.md")
                    for _, f in ipairs(flat) do table.insert(files, f) end

                    local count = 0
                    local seen = {}
                    for _, f in ipairs(files) do
                        if not seen[f] then
                            seen[f] = true
                            count = count + 1
                            local ok, content = pcall(function() return ctx:read(f) end)
                            if ok and content then
                                local fm = parse_frontmatter(content)
                                local t = fm.type or "memory"
                                type_counts[t] = (type_counts[t] or 0) + 1
                            end
                        end
                    end
                    layer_stats[l] = count
                    total = total + count
                end

                local type_list = {}
                for k, v in pairs(type_counts) do
                    table.insert(type_list, { type = k, count = v })
                end
                table.sort(type_list, function(a, b) return a.type < b.type end)

                return {
                    status = "ok",
                    total = total,
                    by_layer = layer_stats,
                    by_type = type_list,
                }
            end,
        },
    },
}
```

- [ ] **Step 2: Build to verify no compilation errors**

Run: `cargo build`
Expected: compiles successfully (Lua is only validated at runtime, not compile time)

- [ ] **Step 3: Test help command**

```bash
cd /tmp && mkdir -p folio-test && cd folio-test
cargo run --manifest-path /home/dzj/file/markdown-cli/Cargo.toml -- plugin run agent_memory help
```

Expected: JSON listing 8 commands (remember, recall, forget, clear, list, consolidate, archive, stats)

- [ ] **Step 4: Test remember with --type decision**

```bash
cargo run --manifest-path /home/dzj/file/markdown-cli/Cargo.toml -- plugin run agent_memory remember use-rust --type decision --content "Use Rust for all backend services" --tags "rust,backend"
```

Expected: `"action": "remembered"`, `"type": "decision"`, path containing `memory/long-term/decisions/use-rust.md`

Verify file was created:
```bash
cat memory/long-term/decisions/use-rust.md
```

Expected: file with YAML frontmatter containing `type: decision`, body with "Use Rust for all backend services"

- [ ] **Step 5: Test remember with --type task**

```bash
cargo run --manifest-path /home/dzj/file/markdown-cli/Cargo.toml -- plugin run agent_memory remember fix-auth --type task --content "Fix auth token expiry" --tags "auth"
```

Expected: path containing `memory/working/tasks/fix-auth.md`

- [ ] **Step 6: Test remember without --type (backward compat)**

```bash
cargo run --manifest-path /home/dzj/file/markdown-cli/Cargo.toml -- plugin run agent_memory remember plain-note --layer working --content "A plain note"
```

Expected: path containing `memory/working/plain-note.md` (flat, no subdirectory)

- [ ] **Step 7: Test recall by key**

```bash
cargo run --manifest-path /home/dzj/file/markdown-cli/Cargo.toml -- plugin run agent_memory recall use-rust
```

Expected: JSON with `"status": "ok"`, full content of the decision memory

- [ ] **Step 8: Test recall --search**

```bash
cargo run --manifest-path /home/dzj/file/markdown-cli/Cargo.toml -- plugin run agent_memory recall --search "rust"
```

Expected: JSON with `"count": 1` or more, results containing the use-rust memory

- [ ] **Step 9: Test recall --type error (empty)**

```bash
cargo run --manifest-path /home/dzj/file/markdown-cli/Cargo.toml -- plugin run agent_memory recall --type error
```

Expected: JSON with `"count": 0` (no error memories stored)

- [ ] **Step 10: Test stats**

```bash
cargo run --manifest-path /home/dzj/file/markdown-cli/Cargo.toml -- plugin run agent_memory stats
```

Expected: JSON with `by_layer.working >= 1`, `by_layer["long-term"] >= 1`, `by_type` containing decision and task types

- [ ] **Step 11: Test list**

```bash
cargo run --manifest-path /home/dzj/file/markdown-cli/Cargo.toml -- plugin run agent_memory list
```

Expected: JSON with all memories listed, each having key/layer/type/path

- [ ] **Step 12: Test forget**

```bash
cargo run --manifest-path /home/dzj/file/markdown-cli/Cargo.toml -- plugin run agent_memory forget plain-note
```

Expected: JSON with `"action": "forgotten"`

- [ ] **Step 13: Test consolidate (--older-than 0d moves everything)**

```bash
cargo run --manifest-path /home/dzj/file/markdown-cli/Cargo.toml -- plugin run agent_memory consolidate --older-than 0d
```

Expected: JSON with `"moved": 1` (the fix-auth task), skipped 0

- [ ] **Step 14: Test archive (--older-than 0d moves everything)**

```bash
cargo run --manifest-path /home/dzj/file/markdown-cli/Cargo.toml -- plugin run agent_memory archive --older-than 0d
```

Expected: JSON with `"moved": 1` (the use-rust decision), skipped 0

Verify:
```bash
ls memory/archive/
```

Expected: `decisions/` directory containing `use-rust.md`

- [ ] **Step 15: Cleanup test dir and commit**

```bash
rm -rf /tmp/folio-test
cd /home/dzj/file/markdown-cli
git add plugins/agent_memory.lua
git commit -m "feat(plugin): enhanced agent_memory with typed routing, 3-layer lifecycle, smart consolidation"
```

---

### Task 3: Update Skill Documentation

**Files:**
- Modify: `~/.claude/skills/folio-cli/SKILL.md` (replace Plugin: Agent Memory section)
- Modify: `~/.claude/skills/folio-cli/references/commands.md` (update Plugins section)

- [ ] **Step 1: Update SKILL.md**

In `~/.claude/skills/folio-cli/SKILL.md`, find the section starting with `### Plugin: Agent Memory` (around line 152). Replace everything from that heading to the end of that section (before any trailing blank lines at end of file) with:

```markdown
### Plugin: Agent Memory

Three-layer typed memory system: working (session), long-term (persistent), archive (old).

```bash
# Store with type routing
folio plugin run agent_memory remember db-timeout --type error --content "Pool size fix" --tags "database"
folio plugin run agent_memory remember use-rust --type decision --content "Rust for backend" --tags "rust"
folio plugin run agent_memory remember task-42 --type task --content "Fix auth" --tags "auth"

# Retrieve
folio plugin run agent_memory recall db-timeout                    # by key
folio plugin run agent_memory recall --search "timeout"            # full-text search
folio plugin run agent_memory recall --type error                  # by type
folio plugin run agent_memory recall --type error --limit 5        # limited results

# Manage
folio plugin run agent_memory list                                 # all memories
folio plugin run agent_memory list --layer working                 # working layer only
folio plugin run agent_memory list --type decision                 # decisions only
folio plugin run agent_memory forget db-timeout                    # delete
folio plugin run agent_memory clear --layer working                # clear all working

# Lifecycle
folio plugin run agent_memory consolidate --older-than 1d          # working -> long-term
folio plugin run agent_memory archive --older-than 30d             # long-term -> archive
folio plugin run agent_memory stats                                # usage statistics
```

Memory types and their default locations:
- `decision` -> `memory/long-term/decisions/` (architecture/tech decisions)
- `error` -> `memory/long-term/errors/` (error patterns + solutions)
- `preference` -> `memory/long-term/preferences/` (user preferences)
- `fact` -> `memory/long-term/facts/` (knowledge facts)
- `procedure` -> `memory/long-term/procedures/` (workflows/SOPs)
- `task` -> `memory/working/tasks/` (in-progress tasks)

All memory files have YAML frontmatter and are queryable by folio's core commands:
```bash
folio --scope memory/ query "frontmatter.type = 'error'"
folio --scope memory/ search "database"
folio --scope memory/ tags
```
```

- [ ] **Step 2: Update references/commands.md**

In `~/.claude/skills/folio-cli/references/commands.md`, find the `## Plugins` section (around line 199). Replace from `## Plugins` through to the end of the `### \`folio plugin run\`` subsection with:

```markdown
## Plugins

### `folio plugin list`

List discovered plugins from `~/.config/folio/plugins/` and `.folio/plugins/`.

### `folio plugin run <name> <args...>`

Run a plugin command. Plugins are Lua scripts with hooks and custom commands.

#### Plugin: agent_memory

Three-layer typed memory system for AI agents.

**Commands:**

| Command | Description |
|---------|-------------|
| `remember <key>` | Store a memory entry |
| `recall <key>` | Retrieve by key, search, type, or layer |
| `forget <key>` | Delete a memory entry |
| `clear --layer L` | Clear all memories in a layer |
| `list` | List memories (filter by --layer, --type) |
| `consolidate` | Move working -> long-term (--older-than Nd) |
| `archive` | Move long-term -> archive (--older-than Nd) |
| `stats` | Show memory usage statistics |
| `help` | Show available commands |

**remember options:**
- `--type <type>` — Memory type: decision, error, preference, fact, procedure, task
- `--layer <layer>` — Override default layer: working, long-term, archive
- `--content <text>` — Memory body text
- `--tags <tags>` — Comma-separated tags

**recall options:**
- `<key>` — Direct key lookup across all layers
- `--search <text>` — Full-text search across all memories
- `--type <type>` — Filter by memory type
- `--layer <layer>` — Filter by layer
- `--limit <n>` — Maximum results (default: 10)

**Type routing:**
```
decision   -> memory/long-term/decisions/
error      -> memory/long-term/errors/
preference -> memory/long-term/preferences/
fact       -> memory/long-term/facts/
procedure  -> memory/long-term/procedures/
task       -> memory/working/tasks/
(no type)  -> memory/<layer>/<key>.md  (backward compatible)
```

**Lifecycle:**
```
working --consolidate--> long-term --archive--> archive --forget--> deleted
```

**Examples:**

```bash
# Store typed memories
folio plugin run agent_memory remember arch-choice --type decision --content "Use PostgreSQL" --tags "database"
folio plugin run agent_memory remember conn-pool-bug --type error --content "Pool exhaustion" --tags "database"
folio plugin run agent_memory remember style --type preference --content "Functional style" --tags "coding"
folio plugin run agent_memory remember deploy-prod --type procedure --content "1. Run tests..." --tags "deploy"

# Retrieve
folio plugin run agent_memory recall arch-choice
folio plugin run agent_memory recall --search "database"
folio plugin run agent_memory recall --type error --limit 5

# Lifecycle
folio plugin run agent_memory consolidate --older-than 1d
folio plugin run agent_memory archive --older-than 30d
folio plugin run agent_memory stats
```
```

Leave the `## Global Options` section and everything after it unchanged.

- [ ] **Step 3: Commit**

```bash
cd ~/.claude/skills/folio-cli
git add SKILL.md references/commands.md
git commit -m "docs(skill): update folio-cli skill with enhanced agent memory docs"
```

Note: These files are in `~/.claude/skills/`, not in the main project repo. If they are not tracked by git, skip the commit.

---

## Verification

After all tasks are complete:

```bash
# 1. All tests pass
cargo test

# 2. Build release
cargo build --release

# 3. Full integration test
mkdir -p /tmp/folio-mem-test && cd /tmp/folio-mem-test
FOLIO="cargo run --release --manifest-path /home/dzj/file/markdown-cli/Cargo.toml --"

# Store one of each type
$FOLIO plugin run agent_memory remember arch --type decision --content "Use PostgreSQL" --tags "db"
$FOLIO plugin run agent_memory remember conn-bug --type error --content "Pool exhaustion" --tags "db"
$FOLIO plugin run agent_memory remember style --type preference --content "Functional style" --tags "code"
$FOLIO plugin run agent_memory remember pg-fact --type fact --content "PostgreSQL supports JSONB" --tags "db"
$FOLIO plugin run agent_memory remember deploy --type procedure --content "1. Test 2. Build 3. Push" --tags "ops"
$FOLIO plugin run agent_memory remember fix-auth --type task --content "Fix auth token" --tags "auth"

# Stats should show 6 total
$FOLIO plugin run agent_memory stats

# Recall by key
$FOLIO plugin run agent_memory recall arch

# Recall by type
$FOLIO plugin run agent_memory recall --type error

# Search
$FOLIO plugin run agent_memory recall --search "postgresql"

# Consolidate working -> long-term
$FOLIO plugin run agent_memory consolidate --older-than 0d

# Archive long-term -> archive
$FOLIO plugin run agent_memory archive --older-than 0d

# Verify everything is in archive now
$FOLIO plugin run agent_memory list --layer archive

# Cleanup
rm -rf /tmp/folio-mem-test
```
