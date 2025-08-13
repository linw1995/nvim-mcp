local clients = vim.lsp.get_clients()
local client_name, workspace_edit_raw = unpack({ ... })
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

local workspace_edit = vim.json.decode(workspace_edit_raw)
local position_encoding = client.offset_encoding or "utf-16"
vim.lsp.util.apply_workspace_edit(workspace_edit, position_encoding)
return vim.json.encode({
    result = vim.NIL,
})
