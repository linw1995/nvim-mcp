# MCP Tools Reference

The server provides 33 MCP tools for interacting with Neovim:

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

- **`read`**: Read document content with universal document identification
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `start` (number, optional, default: 0) - Start line index (0-based),
    `end` (number, optional, default: -1) - End line index, exclusive
    (0-based, -1 for end of buffer)
  - Returns: Document content as text
  - Notes: Supports reading from buffer IDs, project-relative paths, and
    absolute file paths with optional line range specification

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

- **`lsp_type_hierarchy_prepare`**: Prepare type hierarchy for a symbol at a
  specific position
  - Parameters: `connection_id` (string), `document` (DocumentIdentifier),
    `lsp_client_name` (string), `line` (number), `character` (number)
    (all positions are 0-indexed)
  - Returns: Array of TypeHierarchyItem objects or null if no type hierarchy
    available
  - Notes: First step in type hierarchy workflow; prepares symbol for
    supertypes/subtypes analysis

- **`lsp_type_hierarchy_supertypes`**: Get supertypes for a type hierarchy item
  - Parameters: `connection_id` (string), `lsp_client_name` (string),
    `item` (TypeHierarchyItem) - Type hierarchy item from prepare step
  - Returns: Array of TypeHierarchyItem objects showing parent types/interfaces
  - Notes: Shows all parent types, interfaces, or base classes that the symbol
    extends or implements

- **`lsp_type_hierarchy_subtypes`**: Get subtypes for a type hierarchy item
  - Parameters: `connection_id` (string), `lsp_client_name` (string),
    `item` (TypeHierarchyItem) - Type hierarchy item from prepare step
  - Returns: Array of TypeHierarchyItem objects showing derived types/implementations
  - Notes: Shows all derived types, implementations, or subclasses of the symbol

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
