local buffer_id = vim.api.nvim_get_current_buf()
local buffer_name = vim.api.nvim_buf_get_name(buffer_id)
local window_id = vim.api.nvim_get_current_win()
local row, col = unpack(vim.api.nvim_win_get_cursor(window_id))
-- row is one-indexed
-- col is zero-indexed
return { buffer_name = buffer_name, buffer_id = buffer_id, window_id = window_id, row = row - 1, col = col }
