-- Add current project's lua directory to package path for nvim-mcp plugin
package.path = "./lua/?.lua;./lua/?/init.lua;" .. package.path

vim.lsp.config["luals"] = {
    cmd = { "lua-language-server" },
    filetypes = { "lua" },
    root_markers = { ".root" },
    settings = {
        luals = {
            runtime = {
                version = "LuaJIT",
            },
        },
    },
}
vim.lsp.enable("luals")

vim.lsp.config["gopls"] = {
    cmd = { "gopls" },
    filetypes = { "go" },
    root_markers = { ".root" },
}
vim.lsp.enable("gopls")

vim.lsp.config["zls"] = {
    cmd = { "zls" },
    filetypes = { "zig" },
    root_markers = { ".root" },
}
vim.lsp.enable("zls")

vim.lsp.config["ts_ls"] = {
    cmd = { "typescript-language-server", "--stdio" },
    filetypes = { "typescript", "typescriptreact", "typescript.tsx" },
    root_markers = { ".root" },
}
vim.lsp.enable("ts_ls")

local M = require("nvim-mcp")
local MCP = M.MCP
M.setup({
    custom_tools = {
        format = {
            -- Do nothing actually. Test various of configuration options
            description = "Run format on the current buffer.",
            handler = function()
                return MCP.success("success")
            end,
        },
        save_buffer = {
            description = "Save a specific buffer by ID",
            parameters = {
                type = "object",
                properties = {
                    buffer_id = {
                        type = "integer",
                        description = "The buffer ID to save",
                        minimum = 1,
                    },
                },
                required = { "buffer_id" },
            },
            handler = function(params)
                local buf_id = params.buffer_id

                -- Validate buffer
                if not vim.api.nvim_buf_is_valid(buf_id) then
                    return MCP.error("INVALID_PARAMS", "Buffer " .. buf_id .. " is not valid")
                end

                local buf_name = vim.api.nvim_buf_get_name(buf_id)
                if buf_name == "" then
                    return MCP.error("INVALID_PARAMS", "Buffer " .. buf_id .. " has no associated file")
                end

                -- Save the buffer
                local success, err = pcall(function()
                    vim.api.nvim_buf_call(buf_id, function()
                        vim.cmd("write")
                    end)
                end)

                if success then
                    return MCP.success({
                        buffer_id = buf_id,
                        filename = buf_name,
                        message = "Buffer saved successfully",
                    })
                else
                    return MCP.error("INTERNAL_ERROR", "Failed to save buffer: " .. tostring(err))
                end
            end,
        },
    },
})
