local M = {}

local has_setup = false

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
    vim.fn.mkdir(temp_dir, "p")

    return string.format("%s/nvim-mcp.%s.%d.sock", temp_dir, escaped_path, pid)
end

function M.setup(opts)
    if has_setup then
        return
    end
    has_setup = true

    opts = opts or {}

    -- Generate pipe path based on git root
    local pipe_path = generate_pipe_path()
    -- vim.notify("Using pipe path: " .. pipe_path, vim.log.levels.INFO)

    -- Start Neovim RPC server on the pipe
    vim.fn.serverstart(pipe_path)

    -- Set proper permissions on Unix-like systems
    if vim.fn.has("win32") == 0 then
        vim.uv.fs_chmod(pipe_path, tonumber("700", 8))
    end
end

return M
