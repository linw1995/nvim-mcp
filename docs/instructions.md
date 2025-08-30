# Neovim MCP

## Guide

### Connection Workflow for LLMs

#### Automatic Connection (Recommended)

When the nvim-mcp server is started with `--connect auto`, connections are
established automatically:

1. **Pre-established Connections**: Server automatically discovers and connects
   to project-associated Neovim instances
2. **Connection ID Retrieval**: Use the `nvim-connections://` resource to get
   available `connection_id`s
3. **Direct Tool Usage**: Use connection-aware tools immediately with the
   retrieved `connection_id`s

#### Manual Connection Workflow

1. **Discovery Phase**: Use `get_targets` to find available Neovim instances
2. **Connection Phase**: Use `connect` with a target from the discovery results
3. **Caching Phase**: Store the `connection_id` for reuse across multiple operations
4. **Work Phase**: Use connection-aware tools with the cached `connection_id`
5. **Optional Cleanup**: Call `disconnect` only when you're completely done
   with a session

### Connection Caching and Management

- **Cache connections**: Store `connection_id` values and reuse them across operations
- **Connection IDs are deterministic**: Same target always produces same ID
- **Persistent connections**: Connections remain active until explicitly disconnected
- **Parallel operations**: Each connection operates independently
- **Connection replacement**: Connecting to existing target replaces previous connection
- **Resource isolation**: Each connection has separate diagnostic resources
- **Automatic cleanup**: Server handles connection cleanup on process termination

### Tool Usage Patterns

#### File Analysis Workflow

1. get_targets → connect → list_buffers (cache connection_id)
2. buffer_diagnostics (for each relevant buffer, reuse connection_id)
3. Read nvim-diagnostics://{connection_id}/workspace resource
4. Keep connection active for future operations

#### Complete LSP Code Action Workflow

1. get_targets → connect → list_buffers (cache connection_id)
2. lsp_clients (to find available language servers, reuse connection_id)
3. lsp_code_actions (with DocumentIdentifier and LSP client, reuse connection_id)
4. lsp_resolve_code_action (resolve any code action with incomplete data, reuse connection_id)
5. lsp_apply_edit (apply the workspace edit from resolved code action, reuse connection_id)
6. Keep connection active for additional operations

**Enhanced Workflow Benefits:**

- **Complete automation**: No manual exec_lua required for applying changes
- **Robust resolution**: Handles code actions with incomplete edit or command data
- **Native integration**: Uses Neovim's built-in `vim.lsp.util.apply_workspace_edit()`
  for reliable file modifications with proper position encoding handling
- **Error handling**: Proper validation and error reporting throughout the process

### Error Handling Guidelines

- **Connection errors**: Retry with different target from get_targets
- **Invalid connection_id**: Re-establish connection using connect/connect_tcp
- **Buffer not found**: Use list_buffers to get current buffer list
- **LSP errors**: Check lsp_clients for available language servers

### Resource Reading Strategy

- **Use workspace diagnostics**: For project-wide error analysis
- **Use buffer diagnostics**: For file-specific issue investigation
- **Monitor connections**: Use nvim-connections:// to track active instances
- **Parse diagnostic severity**: 1=Error, 2=Warning, 3=Information, 4=Hint

### Safe Code Execution

- **Read-only operations**: Prefer `vim.inspect()`, `vim.fn.getline()`, `vim.api.nvim_buf_get_lines()`
- **State queries**: Use `vim.fn.getcwd()`, `vim.bo.filetype`, `vim.api.nvim_get_current_buf()`
- **Avoid modifications**: Don't use `vim.api.nvim_buf_set_lines()` or similar
  write operations
- **Error handling**: Wrap Lua code in `pcall()` for safe execution

### Integration Workflows

#### Automatic Connection Workflow

1. Start server with `nvim-mcp --connect auto`
2. Read nvim-connections:// resource to get available connection IDs
3. Use connection IDs directly with connection-aware tools
4. Server maintains all connections automatically

#### Diagnostic Analysis

1. Connect to Neovim instance (cache connection_id) or use auto-connected IDs
2. Read workspace diagnostics resource
3. Group diagnostics by severity and file
4. Use buffer_diagnostics for detailed file analysis (reuse connection_id)
5. Provide structured error report
6. Keep connection active for follow-up analysis

#### Code Understanding

1. Connect to Neovim instance (cache connection_id)
2. Use exec_lua to get buffer content and metadata (reuse connection_id)
3. Check LSP clients for language-specific information (reuse connection_id)
4. Use lsp_code_actions with DocumentIdentifier for interesting ranges (reuse connection_id)
5. Use lsp_hover with DocumentIdentifier for detailed symbol information (reuse connection_id)
6. Use lsp_document_symbols with DocumentIdentifier to understand file
   structure (reuse connection_id)
7. Use lsp_workspace_symbols to find related code across project (reuse connection_id)
8. Combine information for comprehensive analysis
9. Maintain connection for iterative code exploration

#### Symbol Navigation Workflow

1. Connect to Neovim instance (cache connection_id)
2. Get available LSP clients (reuse connection_id)
3. Use lsp_workspace_symbols with search query to find symbols across project
4. Use lsp_document_symbols with DocumentIdentifier to understand structure of files
5. Navigate to symbol locations using returned position information
6. Keep connection active for continued navigation

#### Navigation Workflow

1. Connect to Neovim instance (cache connection_id)
2. Use navigate tool to go to a specific position in current buffer or open a file
3. Specify target document using DocumentIdentifier (buffer_id,
   project_relative_path, or absolute_path)
4. Provide zero-based line and character coordinates for precise positioning
5. Receive navigation result with success status, buffer name, and current line content
6. Keep connection active for continued navigation operations

#### Cursor Position Workflow

1. Connect to Neovim instance (cache connection_id)
2. Use cursor_position tool to get current cursor location with zero-based coordinates
3. Retrieve buffer name and row/col position for navigation or context understanding
4. Use position information for targeted operations like hover, references, or definitions
5. Keep connection active for continued cursor-based operations

#### Multi-Instance Management

1. Use get_targets to find all available instances
2. Connect to each target (generates separate connection_ids, cache all IDs)
3. Work with each connection independently using cached IDs
4. Use nvim-connections:// resource to monitor all connections
5. Maintain connections for cross-instance operations
6. Optionally disconnect when completely finished with all instances
