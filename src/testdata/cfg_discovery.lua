vim.opt.runtimepath:append(vim.env.NVIM_MCP_ROOT)
local plugin = require("nvim-mcp")

-- A failed start must not prevent the user from retrying setup.
assert(not pcall(plugin.setup, { pipe = vim.fn.getcwd() .. "/missing/socket" }))
plugin.setup()
local servers = vim.fn.serverlist()
plugin.setup()
assert(vim.deep_equal(servers, vim.fn.serverlist()), "Repeated setup created another server")
