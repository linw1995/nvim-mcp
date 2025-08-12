local clients = vim.lsp.get_clients()
local client_name, code_action_raw, timeout_ms, bufnr = unpack({ ... })
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

local code_action = vim.json.decode(code_action_raw)
local result, err = client:request_sync("codeAction/resolve", code_action, timeout_ms, bufnr)
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
