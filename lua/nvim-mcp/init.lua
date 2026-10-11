local M = {}

local has_setup = false

-- Global registry to store configured tools
M._tool_registry = {}

---@class SetupOptions
---@field custom_tools table<string, CustomTool>|nil Custom tools configuration
---@field pipe string|nil Explicit RPC address (socket, named pipe, or TCP)

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

-- Get git root directory
local function get_git_root()
    local ok, result = pcall(function()
        return vim.system({ "git", "rev-parse", "--show-toplevel" }, {
            cwd = vim.fn.getcwd(),
            text = true,
        }):wait()
    end)
    if ok and result.code == 0 then
        return result.stdout:gsub("[\r\n]+$", "")
    end
    return nil
end

local function normalize_windows_project_path(path)
    -- Resolve directory aliases before hashing, matching Rust's canonicalize.
    path = (vim.uv.fs_realpath(path) or path):gsub("\\", "/")
    path = path:gsub("^//%?/UNC/", "//"):gsub("^//%?/", "")
    path = path:gsub("^%a:", string.lower)
    return path:gsub("/+$", "")
end

-- Get the base directory for socket files
-- Prefers XDG_RUNTIME_DIR (typically /run/user/<uid>, already mode 700)
-- for security, then TMPDIR (e.g. macOS per-user temp), falls back to /tmp
local function get_socket_dir()
    local xdg = os.getenv("XDG_RUNTIME_DIR")
    if xdg and xdg ~= "" then
        return xdg
    end
    local tmpdir = os.getenv("TMPDIR")
    if tmpdir and tmpdir ~= "" then
        return tmpdir
    end
    return "/tmp"
end

-- Generate pipe file path based on git root
local function generate_pipe_path()
    local git_root = get_git_root()
    if not git_root then
        -- Fallback to current working directory if not in git repo
        git_root = vim.fn.getcwd()
    end

    local windows = vim.fn.has("win32") == 1
    if windows then
        git_root = normalize_windows_project_path(git_root)
    end
    local project_id = vim.fn.sha256(git_root):sub(1, 16)
    local pid = vim.fn.getpid()
    local filename = string.format("nvim-mcp.%s.%d.sock", project_id, pid)
    if windows then
        return "\\\\.\\pipe\\" .. filename
    end
    local socket_dir = get_socket_dir()
    return socket_dir:gsub("/+$", "") .. "/" .. filename
end

--- Setup nvim-mcp with custom tools and configuration
---@param opts SetupOptions|nil Configuration options
function M.setup(opts)
    if has_setup then
        return
    end
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
    local pipe_path = opts.pipe or generate_pipe_path()
    -- vim.notify("Using pipe path: " .. pipe_path, vim.log.levels.INFO)

    -- Start Neovim RPC server on the pipe
    vim.fn.serverstart(pipe_path)
    has_setup = true
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
