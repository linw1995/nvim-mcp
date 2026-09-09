vim.opt.runtimepath:append(vim.env.NVIM_MCP_ROOT)
require("nvim-mcp").setup()
local expected = string.format(
    "%s/nvim-mcp.%s.%d.sock",
    vim.env.NVIM_MCP_EXPECTED_DIR,
    vim.env.NVIM_MCP_EXPECTED_ID,
    vim.fn.getpid()
)
assert(#expected <= 103, "Socket path exceeds the portable limit")
assert(vim.tbl_contains(vim.fn.serverlist(), expected), "Lua and Rust socket names differ")
local channel = vim.fn.sockconnect("pipe", expected, { rpc = true })
assert(channel > 0, "Socket is not connectable")
vim.fn.chanclose(channel)
vim.fn.serverstop(expected)
