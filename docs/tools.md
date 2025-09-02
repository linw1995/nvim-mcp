# MCP Tools Reference

The server provides 29 MCP tools for interacting with Neovim:

## Connection Management

- **`get_targets`**: Discover available Neovim targets
  - Returns list of discoverable Neovim socket paths created by the plugin
  - No parameters required

- **`connect`**: Connect via Unix socket/named pipe
  - Parameters: `target` (string) - Socket path from get_targets
  - Returns: `connection_id` (string) - Deterministic connection identifier

- **`connect_tcp`**: Connect via TCP
  - Parameters: `target` (string) - TCP address (e.g., "127.0.0.1:6666")
  - Returns: `connection_id` (string) - Deterministic connection identifier

- **`disconnect`**: Disconnect from specific Neovim instance
  - Parameters: `connection_id` (string) - Connection identifier to disconnect

## Connection-Aware Tools

All tools below require a `connection_id` parameter from the connection
establishment phase:

### Navigation and Positioning

- **`navigate`**: Navigate to a specific position in the current buffer or open
  a file at a specific position
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `line` (number), `character` (number) (all positions are 0-indexed)
  - Returns: Navigation result with success status, buffer name, and current
    line content

- **`cursor_position`**: Get the current cursor position: buffer name,
  and zero-based row/col index
  - Parameters: `connection_id` (string) - Target Neovim connection

### Buffer Operations

- **`list_buffers`**: List all open buffers with names and line counts
  - Parameters: `connection_id` (string) - Target Neovim connection

- **`buffer_diagnostics`**: Get diagnostics for a specific buffer
  - Parameters: `connection_id` (string), `id` (number) - Buffer ID

### LSP Integration

- **`lsp_clients`**: Get workspace LSP clients
  - Parameters: `connection_id` (string) - Target Neovim connection

- **`lsp_workspace_symbols`**: Search workspace symbols by query
  - Parameters: `connection_id` (string), `lsp_client_name` (string), `query`
    (string) - Search query for filtering symbols

- **`lsp_code_actions`**: Get LSP code actions with universal document identification
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `lsp_client_name` (string), `start_line` (number), `start_character` (number),
    `end_line` (number), `end_character` (number) (all positions are 0-indexed)

- **`lsp_hover`**: Get LSP hover information with universal document identification
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `lsp_client_name` (string), `line` (number), `character` (number)
    (all positions are 0-indexed)

- **`lsp_document_symbols`**: Get document symbols with universal document identification
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `lsp_client_name` (string)

- **`lsp_references`**: Get LSP references with universal document identification
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `lsp_client_name` (string), `line` (number), `character` (number),
    `include_declaration` (boolean)

- **`lsp_resolve_code_action`**: Resolve code actions with incomplete data
  - Parameters: `connection_id` (string), `lsp_client_name` (string),
    `code_action` (CodeAction object) - Code action to resolve

- **`lsp_apply_edit`**: Apply workspace edits using Neovim's LSP utility functions
  - Parameters: `connection_id` (string), `lsp_client_name` (string),
    `workspace_edit` (WorkspaceEdit object) - Workspace edit to apply

- **`lsp_definition`**: Get LSP definition with universal document identification
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `lsp_client_name` (string), `line` (number), `character` (number)
    (all positions are 0-indexed)
  - Returns: Definition result supporting Location arrays, LocationLink arrays,
    or null responses

- **`lsp_type_definition`**: Get LSP type definition with universal document identification
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `lsp_client_name` (string), `line` (number), `character` (number)
    (all positions are 0-indexed)
  - Returns: Type definition result supporting Location arrays, LocationLink arrays,
    or null responses

- **`lsp_implementations`**: Get LSP implementations with universal document identification
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `lsp_client_name` (string), `line` (number), `character` (number)
    (all positions are 0-indexed)
  - Returns: Implementation result supporting Location arrays, LocationLink arrays,
    or null responses

- **`lsp_declaration`**: Get LSP declaration with universal document identification
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `lsp_client_name` (string), `line` (number), `character` (number)
    (all positions are 0-indexed)
  - Returns: Declaration result supporting Location arrays, LocationLink arrays,
    or null responses

- **`lsp_rename`**: Rename symbol across workspace using LSP
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `lsp_client_name` (string), `line` (number), `character` (number),
    `new_name` (string), `prepare_first` (boolean, optional)
    (all positions are 0-indexed)
  - Returns: WorkspaceEdit with file changes or validation errors

- **`lsp_formatting`**: Format document using LSP
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `lsp_client_name` (string), `options` (FormattingOptions),
    `apply_edits` (boolean, optional) (all positions are 0-indexed)
  - Returns: Array of TextEdit objects or success confirmation if auto-applied
  - Notes: Supports LSP 3.15.0+ formatting preferences including tab size,
    insert final newline, trim trailing whitespace, etc.

- **`lsp_range_formatting`**: Format a specific range in a document using LSP
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `lsp_client_name` (string), `start_line` (number), `start_character` (number),
    `end_line` (number), `end_character` (number), `options` (FormattingOptions),
    `apply_edits` (boolean, optional) (all positions are 0-indexed)
  - Returns: Array of TextEdit objects or success confirmation if auto-applied
  - Notes: Formats only the specified range with LSP 3.15.0+ formatting preferences

- **`lsp_organize_imports`**: Sort and organize imports using LSP
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `lsp_client_name` (string), `apply_edits` (boolean, optional)
  - Returns: Array of TextEdit objects or success confirmation if auto-applied
  - Notes: Organizes and sorts imports with auto-apply enabled by default

- **`lsp_call_hierarchy_prepare`**: Prepare call hierarchy for a symbol at a
  specific position
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `lsp_client_name` (string), `line` (number), `character` (number)
    (all positions are 0-indexed)
  - Returns: Array of CallHierarchyItem objects or null if no call hierarchy
    available
  - Notes: First step in call hierarchy workflow; prepares symbol for
    incoming/outgoing calls analysis

- **`lsp_call_hierarchy_incoming_calls`**: Get incoming calls for a call
  hierarchy item
  - Parameters: `connection_id` (string), `lsp_client_name` (string),
    `item` (CallHierarchyItem) - Call hierarchy item from prepare step
  - Returns: Array of CallHierarchyIncomingCall objects showing callers
  - Notes: Shows all locations where the symbol is called from

- **`lsp_call_hierarchy_outgoing_calls`**: Get outgoing calls for a call
  hierarchy item
  - Parameters: `connection_id` (string), `lsp_client_name` (string),
    `item` (CallHierarchyItem) - Call hierarchy item from prepare step
  - Returns: Array of CallHierarchyOutgoingCall objects showing callees
  - Notes: Shows all symbols called by the selected symbol

## Universal Document Identifier

The `document` parameter in the universal LSP tools accepts a `DocumentIdentifier`
which can reference documents in three ways:

**DocumentIdentifier Enum**:

- **BufferId(u64)**: Reference by Neovim buffer ID (for currently open files)
  - JSON format: `{"buffer_id": 123}`
- **ProjectRelativePath(PathBuf)**: Reference by project-relative path
  - JSON format: `{"project_relative_path": "src/main.rs"}`
- **AbsolutePath(PathBuf)**: Reference by absolute file path
  - JSON format: `{"absolute_path": "/home/user/project/src/main.rs"}`

This system enables LSP operations on files that may not be open in Neovim buffers,
providing enhanced flexibility for code analysis and navigation.

## Code Execution

- **`exec_lua`**: Execute Lua code in Neovim
  - Parameters: `connection_id` (string), `code` (string) - Lua code to execute

- **`wait_for_lsp_ready`**: Wait for LSP client to be ready and attached
  - Parameters: `connection_id` (string), `client_name` (string, optional),
    `timeout_ms` (number, optional, default: 5000ms)
  - Returns: Success confirmation with LSP client readiness status

## Complete LSP Code Action Workflow

The server now supports the full LSP code action lifecycle:

1. **Get Available Actions**: Use `lsp_code_actions` to retrieve available
   code actions for a specific range
2. **Resolve Action**: Use `lsp_resolve_code_action` to resolve any code
   action that may have incomplete data
3. **Apply Changes**: Use `lsp_apply_edit` to apply the workspace edit from
   the resolved code action

**Example Workflow**:

```text
1. lsp_code_actions → Get available actions
2. lsp_resolve_code_action → Resolve incomplete action data
3. lsp_apply_edit → Apply the workspace edit to files
```

This enables AI assistants to perform complete code refactoring, quick fixes,
and other LSP-powered transformations. The implementation uses Neovim's native
`vim.lsp.util.apply_workspace_edit()` function with proper position encoding
handling, ensuring reliable and accurate file modifications.

## Complete LSP Call Hierarchy Workflow

The server supports full LSP call hierarchy navigation:

1. **Prepare Call Hierarchy**: Use `lsp_call_hierarchy_prepare` to get call
   hierarchy items at a specific position
2. **Get Incoming Calls**: Use `lsp_call_hierarchy_incoming_calls` to find all
   locations that call the selected symbol
3. **Get Outgoing Calls**: Use `lsp_call_hierarchy_outgoing_calls` to find all
   symbols called by the selected symbol

**Example Workflow**:

```text
1. lsp_call_hierarchy_prepare → Get CallHierarchyItem at symbol position
2. lsp_call_hierarchy_incoming_calls → Find all callers of the symbol
3. lsp_call_hierarchy_outgoing_calls → Find all symbols called by the symbol
```

This enables comprehensive call hierarchy analysis for understanding code
relationships, dependency tracking, and navigation through function call chains.
Supports languages with LSP servers that implement call hierarchy capabilities
(LSP 3.16.0+).
