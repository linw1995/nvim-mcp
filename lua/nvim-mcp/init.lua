local M = {}

local has_setup = false

-- Global registry to store configured tools
M._tool_registry = {}

---@class SetupOptions
---@field custom_tools table<string, CustomTool>|nil Custom tools configuration

---@class CustomTool
---@field description string Tool description
---@field parameters JSONSchema|nil JSON Schema specification for tool parameters (follows JSON Schema spec)
---@field handler function Tool execution handler

---@class JSONSchema
---@field type string Schema type ("object", "string", "number", "boolean", "array", etc.)
---@field properties table<string, JSONSchema>|nil Object properties (for type="object")
---@field required string[]|nil Required property names (for type="object")
---@field items JSONSchema|nil Array item schema (for type="array")
---@field minimum number|nil Minimum value (for numeric types)
---@field maximum number|nil Maximum value (for numeric types)
---@field description string|nil Parameter description

-- MCP helper functions for creating responses
M.MCP = {
    success = function(data)
        return {
            content = { { type = "text", text = vim.json.encode(data) } },
            isError = false,
        }
    end,

    error = function(code, message, data)
        return {
            content = { { type = "text", text = message } },
            isError = true,
            _meta = { error = { code = code, message = message, data = data } },
        }
    end,

    text = function(text)
        return {
            content = { { type = "text", text = text } },
            isError = false,
        }
    end,

    json = function(data)
        return {
            content = { { type = "text", text = vim.json.encode(data) } },
            isError = false,
        }
    end,
}

-- Escape path for use in filename by replacing problematic characters
local function escape_path(path)
    -- Remove leading/trailing whitespace and replace '/' with '%'
    return path:gsub("^%s+", ""):gsub("%s+$", ""):gsub("/", "%%")
end

-- Get git root directory
local function get_git_root()
    local handle = io.popen("git rev-parse --show-toplevel 2>/dev/null")
    if not handle then
        return nil
    end
    local result = handle:read("*a")
    handle:close()

    if result and result ~= "" then
        return result:gsub("^%s+", ""):gsub("%s+$", "") -- trim whitespace
    end
    return nil
end

-- Generate pipe file path based on git root
local function generate_pipe_path()
    local git_root = get_git_root()
    if not git_root then
        -- Fallback to current working directory if not in git repo
        git_root = vim.fn.getcwd()
    end

    local escaped_path = escape_path(git_root)
    local pid = vim.fn.getpid()
    local temp_dir = vim.fn.has("win32") == 1 and os.getenv("TEMP") or "/tmp"

    return string.format("%s/nvim-mcp.%s.%d.sock", temp_dir, escaped_path, pid)
end

--- Setup nvim-mcp with custom tools and configuration
---@param opts SetupOptions|nil Configuration options
function M.setup(opts)
    if has_setup then
        return
    end
    has_setup = true

    opts = opts or {}

    -- Store custom tools in registry with validation
    if opts.custom_tools then
        for tool_name, tool_config in pairs(opts.custom_tools) do
            -- VALIDATION: Ensure required fields exist
            if not tool_config.description or not tool_config.handler then
                vim.notify("Invalid tool config for: " .. tool_name, vim.log.levels.ERROR)
            else
                M._tool_registry[tool_name] = {
                    description = tool_config.description or "",
                    parameters = tool_config.parameters or {
                        type = "object",
                    },
                    handler = tool_config.handler,
                }
            end
        end
    end

    -- PRESERVE: Existing RPC server setup
    local pipe_path = generate_pipe_path()
    -- vim.notify("Using pipe path: " .. pipe_path, vim.log.levels.INFO)

    -- Start Neovim RPC server on the pipe
    vim.fn.serverstart(pipe_path)
end

-- Tool Discovery API for MCP Server
function M.get_registered_tools()
    local tools = {}
    for tool_name, tool_config in pairs(M._tool_registry) do
        tools[tool_name] = {
            name = tool_name,
            description = tool_config.description,
            input_schema = tool_config.parameters,
        }
    end
    if next(tools) == nil then
        return nil
    else
        return tools
    end
end

-- Tool Execution API with error handling
function M.execute_tool(tool_name, params)
    local tool_config = M._tool_registry[tool_name]

    if not tool_config then
        return M.MCP.error("TOOL_NOT_FOUND", "Tool '" .. tool_name .. "' not registered")
    end

    -- SAFE EXECUTION: Use pcall for error handling
    local success, result = pcall(tool_config.handler, params)

    if success then
        return result
    else
        return M.MCP.error("EXECUTION_ERROR", "Tool execution failed: " .. tostring(result))
    end
end

return M
