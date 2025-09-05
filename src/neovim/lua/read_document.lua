local params_raw = ...
local params = vim.json.decode(params_raw)

local start_line = params.start_line
local end_line = params.end_line

if params.buffer_id ~= nil then
    local lines = vim.api.nvim_buf_get_lines(params.buffer_id, start_line, end_line, false)
    return vim.json.encode({ result = table.concat(lines, "\n") })
else
    local lines = vim.fn.readfile(params.file_path)
    local result = {}

    if end_line < 0 then
        end_line = #lines + 1 + end_line
    end

    for i = start_line, math.min(end_line, #lines) do
        result[#result + 1] = lines[i]
    end

    return vim.json.encode({ result = table.concat(result, "\n") })
end
