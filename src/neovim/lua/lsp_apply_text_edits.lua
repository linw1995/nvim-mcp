local clients = vim.lsp.get_clients()
local client_name, text_edits_raw, uri = unpack({ ... })
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

local text_edits = vim.json.decode(text_edits_raw)
local position_encoding = client.offset_encoding or "utf-16"

-- Find the buffer ID for the given URI
local bufnr = vim.uri_to_bufnr(uri)

-- Apply text edits to the buffer
-- vim.lsp.util.apply_text_edits expects text_edits, bufnr, and encoding
vim.lsp.util.apply_text_edits(text_edits, bufnr, position_encoding)

return vim.json.encode({
    result = vim.NIL,
})
