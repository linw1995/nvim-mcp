local group = vim.api.nvim_create_augroup("NVIM_MCP_DiagnosticsChanged", { clear = true })

vim.api.nvim_create_autocmd("DiagnosticChanged", {
    group = group,
    callback = function(args)
        vim.rpcnotify(0, "NVIM_MCP_DiagnosticsChanged", {
            buf = args.buf,
            diagnostics = args.data.diagnostics,
        })
    end,
})

vim.api.nvim_create_autocmd("LspAttach", {
    group = group,
    callback = function(args)
        local client_id = args.data.client_id
        local client = vim.lsp.get_client_by_id(client_id)

        if client then
            vim.rpcnotify(0, "NVIM_MCP_LspAttach", {
                client_name = client.name,
                client_id = client_id,
                buffer_id = args.buf,
                server_capabilities = client.server_capabilities,
                initialized = client.initialized,
                attach_time = vim.uv.now(),
            })
        end
    end,
})

vim.api.nvim_create_autocmd("LspDetach", {
    group = group,
    callback = function(args)
        local client_id = args.data.client_id
        local client = vim.lsp.get_client_by_id(client_id)

        if client then
            vim.rpcnotify(0, "NVIM_MCP_LspDetach", {
                client_name = client.name,
                client_id = client_id,
                buffer_id = args.buf,
                detach_time = vim.uv.now(),
            })
        end
    end,
})

vim.rpcnotify(0, "NVIM_MCP", "setup diagnostics changed autocmd")
