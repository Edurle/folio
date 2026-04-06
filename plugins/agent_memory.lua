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
