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
