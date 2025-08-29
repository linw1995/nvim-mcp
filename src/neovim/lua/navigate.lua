-- Navigate tool implementation
-- Args: uri, row, col, buffer_id
local params_raw = ...

local params = vim.json.decode(params_raw)

-- Convert to 1-based row for Vim (col is already 0-based)
params.position.line = params.position.line + 1

-- Function to navigate to file by URI
local function navigate_to_file(file_uri)
    -- Convert URI to file path
    local file_path = vim.uri_to_fname(file_uri)

    -- Check if file exists first
    if vim.fn.filereadable(file_path) == 0 then
        return vim.json.encode({ err_msg = "File not found or not readable: " .. file_path })
    end

    -- Check if the file is already open in a buffer
    for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        if vim.api.nvim_buf_is_valid(buf) then
            local bufname = vim.api.nvim_buf_get_name(buf)
            local buf_path = vim.fn.fnamemodify(bufname, ":p")
            local target_path = vim.fn.fnamemodify(file_path, ":p")
            if buf_path == target_path then
                vim.cmd("buffer " .. buf)
                return true
            end
        end
    end

    -- If not open, edit the file
    vim.cmd("edit " .. vim.fn.fnameescape(file_path))
    return true
end

local result = navigate_to_file(params.textDocument.uri)
if type(result) == "string" then
    -- navigate_to_file returned JSON encoded error
    return result
end

-- Set the cursor position
local current_buf = vim.api.nvim_get_current_buf()
local line_count = vim.api.nvim_buf_line_count(current_buf)

-- Set cursor position
vim.api.nvim_win_set_cursor(0, { params.position.line, params.position.character })

-- Return success information
local current_bufname = vim.api.nvim_buf_get_name(current_buf)
local line = vim.api.nvim_get_current_line()
return vim.json.encode({
    result = {
        success = true,
        buffer_name = current_bufname,
        line = line,
    },
})
