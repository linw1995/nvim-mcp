local clients = vim.lsp.get_clients()
local client_name, params_raw, timeout_ms, bufnr = unpack({ ... })
local client
for _, v in ipairs(clients) do
    if v.name == client_name then
        client = v
    end
end
if client == nil then
    return vim.json.encode({
        err_msg = string.format("LSP client %s not found", vim.json.encode(client_name)),
    })
end

local params = vim.json.decode(params_raw)
local result, err = client:request_sync("textDocument/references", params, timeout_ms, bufnr)
if err then
    return vim.json.encode({
        err_msg = string.format(
            "LSP client %s request_sync error: %s",
            vim.json.encode(client_name),
            vim.json.encode(err)
        ),
    })
end

return vim.json.encode(result)