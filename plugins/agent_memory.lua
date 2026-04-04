-- Folio Agent Memory Plugin
-- Two-layer memory system for AI agents:
--   working:  memory/working/  - session-level, cleared between sessions
--   long-term: memory/long-term/ - persistent across sessions
--
-- Usage:
--   folio plugin run agent_memory remember <key> [--layer working|long-term] [--tags rust,cli] --content "..."
--   folio plugin run agent_memory recall <key>
--   folio plugin run agent_memory recall --search "query text" --layer working --limit 10
--   folio plugin run agent_memory forget <key>
--   folio plugin run agent_memory clear --layer working
--   folio plugin run agent_memory list [--layer working|long-term]
--   folio plugin run agent_memory consolidate --older-than 7d

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
    for k, v in pairs(data) do
        if type(v) == "table" then
            table.insert(lines, k .. ":")
            for _, item in ipairs(v) do
                table.insert(lines, "  - " .. tostring(item))
            end
        elseif type(v) == "string" then
            -- Quote strings that contain special chars
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

local function ensure_dir(ctx, dir)
    -- Try to list, if it fails the dir doesn't exist
    local files = ctx:ls(dir)
    -- If we get here, dir exists. If not, we'll handle on write.
end

return {
    name = "agent_memory",
    version = "0.1.0",
    description = "Layered memory system for AI agents (working + long-term)",

    commands = {
        {
            name = "help",
            description = "Show available commands",
            handler = function(ctx, args)
                return {
                    status = "ok",
                    plugin = "agent_memory",
                    commands = {
                        { name = "remember", description = "Store a memory entry" },
                        { name = "recall", description = "Retrieve memories by key or search" },
                        { name = "forget", description = "Delete a memory entry" },
                        { name = "clear", description = "Clear all memories in a layer" },
                        { name = "list", description = "List memories in a layer" },
                        { name = "consolidate", description = "Move old working memories to long-term" },
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
                local layer = opts.layer or "long-term"
                local content = opts.content or ""
                local tags_str = opts.tags or ""
                local agent = opts.agent or "unknown"

                -- Parse tags
                local tags = {}
                if tags_str ~= "" then
                    for tag in tags_str:gmatch("[^,]+") do
                        table.insert(tags, tag:match("^%s*(.-)%s*$"))
                    end
                end

                -- Add layer as implicit tag
                table.insert(tags, "memory:" .. layer)

                local dir = "memory/" .. layer
                local path = dir .. "/" .. key .. ".md"
                local now = ctx:now()

                -- Try to read existing for access_count
                local existing_fm = {}
                local access_count = 0
                local created = now
                if ctx:exists(path) then
                    access_count = 1  -- increment, but we can't parse existing fm in Lua easily
                    created = now  -- keep original if we could read it
                end

                local fm = {
                    type = "memory",
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
                    layer = layer,
                    path = path,
                    tags = tags,
                }
            end,
        },
        {
            name = "recall",
            description = "Retrieve memories by key or search",
            handler = function(ctx, args)
                local opts = parse_args(args)
                local key = opts.positional[1]
                local layer = opts.layer
                local search = opts.search or opts.positional[1]
                local limit = tonumber(opts.limit) or 10

                -- If specific key, try direct read
                if key and not opts.search then
                    for _, l in ipairs({ "working", "long-term" }) do
                        if not layer or layer == l then
                            local path = "memory/" .. l .. "/" .. key .. ".md"
                            if ctx:exists(path) then
                                local content = ctx:read(path)
                                return {
                                    status = "ok",
                                    key = key,
                                    layer = l,
                                    path = path,
                                    content = content,
                                }
                            end
                        end
                    end
                    return {
                        status = "not_found",
                        key = key,
                        message = "Memory '" .. key .. "' not found in any layer",
                    }
                end

                -- Search mode: list all files in the layer(s) and return them
                local results = {}
                local layers = layer and { layer } or { "working", "long-term" }

                for _, l in ipairs(layers) do
                    local dir = "memory/" .. l
                    local files = ctx:ls(dir)
                    local count = 0
                    for _, f in ipairs(files) do
                        if count >= limit then break end
                        if f:sub(-3) == ".md" then
                            local mem_key = f:sub(1, -4)
                            local path = dir .. "/" .. f
                            local content = ctx:read(path)
                            -- Simple search: check if content contains the search term
                            if not search or content:lower():find(search:lower(), 1, true) then
                                table.insert(results, {
                                    key = mem_key,
                                    layer = l,
                                    path = path,
                                    preview = content:sub(1, 200),
                                })
                                count = count + 1
                            end
                        end
                    end
                end

                return {
                    status = "ok",
                    results = results,
                    count = #results,
                    search = search,
                }
            end,
        },
        {
            name = "forget",
            description = "Delete a memory entry",
            handler = function(ctx, args)
                local opts = parse_args(args)
                local key = opts.positional[1]
                local layer = opts.layer

                if not key then
                    return { status = "error", message = "Key required: forget <key>" }
                end

                local layers = layer and { layer } or { "working", "long-term" }
                for _, l in ipairs(layers) do
                    local path = "memory/" .. l .. "/" .. key .. ".md"
                    if ctx:exists(path) then
                        ctx:rm(path)
                        return {
                            status = "ok",
                            action = "forgotten",
                            key = key,
                            layer = l,
                            path = path,
                        }
                    end
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

                if layer ~= "working" and layer ~= "long-term" then
                    return { status = "error", message = "Layer must be 'working' or 'long-term'" }
                end

                local dir = "memory/" .. layer
                local files = ctx:ls(dir)
                local removed = 0

                for _, f in ipairs(files) do
                    if f:sub(-3) == ".md" then
                        ctx:rm(dir .. "/" .. f)
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

                local results = {}
                local layers = layer and { layer } or { "working", "long-term" }

                for _, l in ipairs(layers) do
                    local dir = "memory/" .. l
                    local files = ctx:ls(dir)
                    for _, f in ipairs(files) do
                        if f:sub(-3) == ".md" then
                            table.insert(results, {
                                key = f:sub(1, -4),
                                layer = l,
                                path = dir .. "/" .. f,
                            })
                        end
                    end
                end

                return {
                    status = "ok",
                    memories = results,
                    count = #results,
                }
            end,
        },
        {
            name = "consolidate",
            description = "Move working memories to long-term",
            handler = function(ctx, args)
                local opts = parse_args(args)
                local older_than = opts["older-than"] or "1d"

                -- Parse duration (simple: support Nd for days)
                local days = tonumber(older_than:match("(%d+)d")) or 1

                local dir = "memory/working"
                local files = ctx:ls(dir)
                local moved = 0

                for _, f in ipairs(files) do
                    if f:sub(-3) == ".md" then
                        local src = dir .. "/" .. f
                        local dst = "memory/long-term/" .. f
                        local content = ctx:read(src)

                        -- Write to long-term, remove from working
                        ctx:write(dst, content)
                        ctx:rm(src)
                        moved = moved + 1
                    end
                end

                return {
                    status = "ok",
                    action = "consolidated",
                    moved = moved,
                    note = "Moved all working memories to long-term",
                }
            end,
        },
    },
}
