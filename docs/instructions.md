# Neovim MCP

## Features

### Tools

The server provides 23 MCP tools for interacting with Neovim instances:

#### Connection Management

- **`get_targets`**: Discover available Neovim socket paths
  - **Parameters**: None
  - **Returns**: Array of socket paths created by the nvim-mcp plugin
  - **Usage**: Call first to find available Neovim instances

- **`connect`**: Connect via Unix socket/named pipe
  - **Parameters**:
    - `target` (string): Socket path from get_targets
  - **Returns**: Object with `connection_id`, `target`, and `message`
  - **Usage**: Establishes connection and returns deterministic connection ID

- **`connect_tcp`**: Connect via TCP address
  - **Parameters**:
    - `target` (string): TCP address (e.g., "127.0.0.1:6666")
  - **Returns**: Object with `connection_id`, `target`, and `message`
  - **Usage**: For manual TCP connections to Neovim with --listen

- **`disconnect`**: Disconnect from Neovim instance
  - **Parameters**:
    - `connection_id` (string): ID from connect/connect_tcp response
  - **Returns**: Confirmation message with target info
  - **Usage**: Clean up specific connection when done

#### Connection-Aware Tools

All tools below require a `connection_id` parameter from connection establishment:

- **`list_buffers`**: List all open buffers
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
  - **Returns**: Array of buffer objects with ID, name, and line count
  - **Usage**: Get overview of available buffers for file operations

- **`exec_lua`**: Execute Lua code in Neovim
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `code` (string): Lua code to execute
  - **Returns**: Object with execution result
  - **Usage**: Run Neovim commands, get editor state, or modify configuration

- **`buffer_diagnostics`**: Get diagnostics for specific buffer
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `id` (number): Buffer ID from list_buffers
  - **Returns**: Array of diagnostic objects with severity, message, and position
  - **Usage**: Analyze errors/warnings in specific file

- **`lsp_clients`**: Get workspace LSP clients
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
  - **Returns**: Array of active LSP client objects
  - **Usage**: Check available language servers before requesting code actions

- **`lsp_workspace_symbols`**: Search workspace symbols by query
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `lsp_client_name` (string): LSP client name from lsp_clients
    - `query` (string): Search query to filter symbols (empty string returns all)
  - **Returns**: Array of workspace symbol objects with names, locations, and kinds
  - **Usage**: Find symbols across the entire workspace for navigation and code exploration

- **`lsp_code_actions`**: Get LSP code actions with universal document identification
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `document` (DocumentIdentifier): Universal document identifier
      (BufferId, ProjectRelativePath, or AbsolutePath)
    - `lsp_client_name` (string): LSP client name from lsp_clients
    - `start_line` (number): Start line (0-indexed)
    - `start_character` (number): Start character (0-indexed)
    - `end_line` (number): End line (0-indexed)
    - `end_character` (number): End character (0-indexed)
  - **Returns**: Array of available code action objects
  - **Usage**: Get refactoring options, quick fixes, and code suggestions
    for any document

- **`lsp_hover`**: Get LSP hover information with universal document identification
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `document` (DocumentIdentifier): Universal document identifier
      (BufferId, ProjectRelativePath, or AbsolutePath)
    - `lsp_client_name` (string): LSP client name from lsp_clients
    - `line` (number): Symbol position line (0-indexed)
    - `character` (number): Symbol position character (0-indexed)
  - **Returns**: Object with hover information including documentation and type details
  - **Usage**: Get detailed information about symbols, functions, variables
    at cursor position in any document

- **`lsp_document_symbols`**: Get document symbols with universal document identification
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `document` (DocumentIdentifier): Universal document identifier
      (BufferId, ProjectRelativePath, or AbsolutePath)
    - `lsp_client_name` (string): LSP client name from lsp_clients
  - **Returns**: Array of document symbol objects with names, kinds, and ranges
  - **Usage**: Navigate and understand code structure within any document

- **`lsp_references`**: Get LSP references with universal document identification
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `document` (DocumentIdentifier): Universal document identifier
      (BufferId, ProjectRelativePath, or AbsolutePath)
    - `lsp_client_name` (string): LSP client name from lsp_clients
    - `line` (number): Symbol position line (0-indexed)
    - `character` (number): Symbol position character (0-indexed)
    - `include_declaration` (boolean): Include the declaration of the
      current symbol in the results
  - **Returns**: Array of reference objects with locations
  - **Usage**: Find all references to a symbol across the workspace in any document

- **`lsp_resolve_code_action`**: Resolve code actions with incomplete data
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `lsp_client_name` (string): LSP client name from lsp_clients
    - `code_action` (CodeAction): Code action object to resolve
  - **Returns**: Resolved CodeAction object with complete data
  - **Usage**: Resolve code actions that may have incomplete edit or command data

- **`lsp_apply_edit`**: Apply workspace edits using Neovim's LSP utility functions
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `lsp_client_name` (string): LSP client name from lsp_clients
    - `workspace_edit` (WorkspaceEdit): Workspace edit object to apply
  - **Returns**: Success confirmation
  - **Usage**: Apply code changes from resolved code actions to files using
    `vim.lsp.util.apply_workspace_edit()` with proper position encoding handling

- **`lsp_definition`**: Get LSP definition with universal document identification
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `document` (DocumentIdentifier): Universal document identifier
      (BufferId, ProjectRelativePath, or AbsolutePath)
    - `lsp_client_name` (string): LSP client name from lsp_clients
    - `line` (number): Symbol position line (0-indexed)
    - `character` (number): Symbol position character (0-indexed)
  - **Returns**: Definition result supporting Location arrays, LocationLink
    arrays, or null responses
  - **Usage**: Find symbol definitions with enhanced type information and
    robust result handling

- **`lsp_type_definition`**: Get LSP type definition with universal document identification
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `document` (DocumentIdentifier): Universal document identifier
      (BufferId, ProjectRelativePath, or AbsolutePath)
    - `lsp_client_name` (string): LSP client name from lsp_clients
    - `line` (number): Symbol position line (0-indexed)
    - `character` (number): Symbol position character (0-indexed)
  - **Returns**: Type definition result supporting Location arrays, LocationLink
    arrays, or null responses
  - **Usage**: Find type definitions for symbols, variables, and expressions with
    universal document support

- **`lsp_implementations`**: Get LSP implementations with universal document identification
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `document` (DocumentIdentifier): Universal document identifier
      (BufferId, ProjectRelativePath, or AbsolutePath)
    - `lsp_client_name` (string): LSP client name from lsp_clients
    - `line` (number): Symbol position line (0-indexed)
    - `character` (number): Symbol position character (0-indexed)
  - **Returns**: Implementation result supporting Location arrays, LocationLink
    arrays, or null responses
  - **Usage**: Find interface/abstract class implementations with universal
    document identification for enhanced code navigation

- **`lsp_declaration`**: Get LSP declaration with universal document identification
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `document` (DocumentIdentifier): Universal document identifier
      (BufferId, ProjectRelativePath, or AbsolutePath)
    - `lsp_client_name` (string): LSP client name from lsp_clients
    - `line` (number): Symbol position line (0-indexed)
    - `character` (number): Symbol position character (0-indexed)
  - **Returns**: Declaration result supporting Location arrays, LocationLink
    arrays, or null responses
  - **Usage**: Find symbol declarations with universal document identification
    for enhanced code navigation

- **`lsp_rename`**: Rename symbol across workspace using LSP
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `document` (DocumentIdentifier): Universal document identifier
      (BufferId, ProjectRelativePath, or AbsolutePath)
    - `lsp_client_name` (string): LSP client name from lsp_clients
    - `line` (number): Symbol position line (0-indexed)
    - `character` (number): Symbol position character (0-indexed)
    - `new_name` (string): New name for the symbol
    - `prepare_first` (boolean, optional): Whether to run prepare rename first
      for validation (default: true)
  - **Returns**: WorkspaceEdit with file changes or validation errors
  - **Usage**: Rename symbols across workspace with optional validation via
    prepare rename

- **`lsp_formatting`**: Format document using LSP
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `document` (DocumentIdentifier): Universal document identifier
      (BufferId, ProjectRelativePath, or AbsolutePath)
    - `lsp_client_name` (string): LSP client name from lsp_clients
    - `options` (FormattingOptions): LSP formatting preferences
    - `apply_edits` (boolean, optional): Whether to automatically apply formatting
      changes (default: false)
  - **Returns**: Array of TextEdit objects or success confirmation if auto-applied
  - **Usage**: Format documents using LSP with support for LSP 3.15.0+ formatting
    preferences including tab size, insert final newline, trim trailing whitespace

- **`lsp_range_formatting`**: Format a specific range in a document using LSP
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `document` (DocumentIdentifier): Universal document identifier
      (BufferId, ProjectRelativePath, or AbsolutePath)
    - `lsp_client_name` (string): LSP client name from lsp_clients
    - `start_line` (number): Range start position, line number starts from 0
    - `start_character` (number): Range start position, character number starts
      from 0
    - `end_line` (number): Range end position, line number starts from 0
    - `end_character` (number): Range end position, character number starts
      from 0
    - `options` (FormattingOptions): LSP formatting preferences
    - `apply_edits` (boolean, optional): Whether to automatically apply formatting
      changes (default: false)
  - **Returns**: Array of TextEdit objects or success confirmation if auto-applied
  - **Usage**: Format a specific range in documents using LSP with support for
    LSP 3.15.0+ formatting preferences including tab size, insert final newline,
    trim trailing whitespace

- **`lsp_organize_imports`**: Sort and organize imports using LSP
  - **Parameters**:
    - `connection_id` (string): Target Neovim instance ID
    - `document` (DocumentIdentifier): Universal document identifier
      (BufferId, ProjectRelativePath, or AbsolutePath)
    - `lsp_client_name` (string): LSP client name from lsp_clients
    - `apply_edits` (boolean, optional): Whether to automatically apply formatting
      changes (default: true)
  - **Returns**: Array of TextEdit objects or success confirmation if auto-applied
  - **Usage**: Sort and organize imports using LSP with auto-apply enabled by default

### Resources

### Universal Document Identifier System

The server includes a universal document identifier system that enhances LSP operations
by supporting multiple ways of referencing documents:

**DocumentIdentifier Enum**:

- **BufferId(u64)**: Reference by Neovim buffer ID (for currently open files)
  - JSON format: `{"buffer_id": 123}`
- **ProjectRelativePath(PathBuf)**: Reference by project-relative path
  - JSON format: `{"project_relative_path": "src/main.rs"}`
- **AbsolutePath(PathBuf)**: Reference by absolute file path
  - JSON format: `{"absolute_path": "/home/user/project/src/main.rs"}`

This system enables LSP operations on files that may not be open in Neovim buffers,
providing enhanced flexibility for code analysis and navigation. The universal LSP
tools (`lsp_code_actions`, `lsp_hover`, `lsp_document_symbols`,
`lsp_references`, `lsp_definition`, `lsp_type_definition`,
`lsp_implementations`, `lsp_declaration`, `lsp_rename`, `lsp_formatting`,
`lsp_range_formatting`, `lsp_organize_imports`) accept
any of these
document identifier types.

### MCP Resources

The server provides connection-aware MCP resources via multiple URI schemes:

#### Connection Management Resource

- **`nvim-connections://`**: Lists active Neovim connections
  - **Content**: JSON array of connection objects with `id` and `target`
  - **Usage**: Monitor active connections across multiple Neovim instances

#### Tool Registration Resources ⚠️ **(Experimental)**

Connection-aware tool registration resources using `nvim-tools://` scheme:

- **`nvim-tools://`**: Overview of all tools and their connection mappings
  - **Content**: JSON object showing static tools (available to all connections)
    and dynamic tools (connection-specific)
  - **Usage**: Understand tool availability and connection mappings across the system

- **`nvim-tools://{connection_id}`**: List of tools available for a specific connection
  - **Content**: JSON array of tool objects available for the specific connection
  - **Usage**: Get detailed view of tools available for a particular Neovim instance,
    including both static and connection-specific dynamic tools

*Note: Tool registration resources are experimental and may change in future versions.*

#### Diagnostic Resources

Connection-scoped diagnostic resources using `nvim-diagnostics://` scheme:

- **`nvim-diagnostics://{connection_id}/workspace`**: All workspace diagnostics
  - **Content**: JSON array of diagnostic messages across entire workspace
  - **Usage**: Get comprehensive error/warning overview for project

- **`nvim-diagnostics://{connection_id}/buffer/{buffer_id}`**: Buffer-specific diagnostics
  - **Content**: JSON array of diagnostic messages for single buffer
  - **Usage**: Focus on errors/warnings in specific file

**Diagnostic Object Structure**:

```json
{
  "severity": 1,
  "message": "Error description",
  "source": "lsp_client_name",
  "range": {
    "start": { "line": 0, "character": 0 },
    "end": { "line": 0, "character": 10 }
  },
  "filename": "/path/to/file.ext"
}
```

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

#### Multi-Instance Management

1. Use get_targets to find all available instances
2. Connect to each target (generates separate connection_ids, cache all IDs)
3. Work with each connection independently using cached IDs
4. Use nvim-connections:// resource to monitor all connections
5. Maintain connections for cross-instance operations
6. Optionally disconnect when completely finished with all instances
