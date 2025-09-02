# Neovim MCP

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

## Complete LSP Type Hierarchy Workflow

The server supports full LSP type hierarchy navigation:

1. **Prepare Type Hierarchy**: Use `lsp_type_hierarchy_prepare` to get type
   hierarchy items at a specific position
2. **Get Supertypes**: Use `lsp_type_hierarchy_supertypes` to find all parent
   types, interfaces, or base classes that the selected symbol extends or implements
3. **Get Subtypes**: Use `lsp_type_hierarchy_subtypes` to find all derived
   types, implementations, or subclasses of the selected symbol

**Example Workflow**:

```text
1. lsp_type_hierarchy_prepare → Get TypeHierarchyItem at symbol position
2. lsp_type_hierarchy_supertypes → Find all parent types/interfaces
3. lsp_type_hierarchy_subtypes → Find all implementations/derived types
```

This enables comprehensive type hierarchy analysis for understanding inheritance
relationships, polymorphism tracking, and navigation through type hierarchies.
Supports languages with LSP servers that implement type hierarchy capabilities
(LSP 3.17.0+).

## File Analysis Workflow

1. get_targets → connect → list_buffers (cache connection_id)
2. buffer_diagnostics (for each relevant buffer, reuse connection_id)
3. Read nvim-diagnostics://{connection_id}/workspace resource
4. Keep connection active for future operations

## Resource Reading Strategy

- **Use workspace diagnostics**: For project-wide error analysis
- **Use buffer diagnostics**: For file-specific issue investigation
- **Monitor connections**: Use nvim-connections:// to track active instances
- **Parse diagnostic severity**: 1=Error, 2=Warning, 3=Information, 4=Hint

## Safe Code Execution

- **Read-only operations**: Prefer `vim.inspect()`, `vim.fn.getline()`, `vim.api.nvim_buf_get_lines()`
- **State queries**: Use `vim.fn.getcwd()`, `vim.bo.filetype`, `vim.api.nvim_get_current_buf()`
- **Avoid modifications**: Don't use `vim.api.nvim_buf_set_lines()` or similar
  write operations
- **Error handling**: Wrap Lua code in `pcall()` for safe execution

## Diagnostic Analysis

1. Connect to Neovim instance (cache connection_id) or use auto-connected IDs
2. Read workspace diagnostics resource
3. Group diagnostics by severity and file
4. Use buffer_diagnostics for detailed file analysis (reuse connection_id)
5. Provide structured error report
6. Keep connection active for follow-up analysis

## Symbol Navigation Workflow

1. Connect to Neovim instance (cache connection_id)
2. Get available LSP clients (reuse connection_id)
3. Use lsp_workspace_symbols with search query to find symbols across project
4. Use lsp_document_symbols with DocumentIdentifier to understand structure of files
5. Navigate to symbol locations using returned position information
6. Keep connection active for continued navigation

## Navigation Workflow

1. Connect to Neovim instance (cache connection_id)
2. Use navigate tool to go to a specific position in current buffer or open a file
3. Specify target document using DocumentIdentifier (buffer_id,
   project_relative_path, or absolute_path)
4. Provide zero-based line and character coordinates for precise positioning
5. Receive navigation result with success status, buffer name, and current line content
6. Keep connection active for continued navigation operations

## Cursor Position Workflow

1. Connect to Neovim instance (cache connection_id)
2. Use cursor_position tool to get current cursor location with zero-based coordinates
3. Retrieve buffer name and row/col position for navigation or context understanding
4. Use position information for targeted operations like hover, references, or definitions
5. Keep connection active for continued cursor-based operations

## Multi-Instance Management

1. Use get_targets to find all available instances
2. Connect to each target (generates separate connection_ids, cache all IDs)
3. Work with each connection independently using cached IDs
4. Use nvim-connections:// resource to monitor all connections
5. Maintain connections for cross-instance operations
6. Optionally disconnect when completely finished with all instances
