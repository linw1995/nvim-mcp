local bufname = vim.api.nvim_buf_get_name(0)
local row, col = unpack(vim.api.nvim_win_get_cursor(0))
-- row is one-indexed
-- col is zero-indexed
return { bufname = bufname, row = row - 1, col = col }
