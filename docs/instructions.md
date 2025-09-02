# Neovim MCP Server Instructions

## Core Concepts

### Connection Management

- **Connection ID**: Unique identifier for each Neovim instance connection
- **Auto-connection**: Server automatically connects to available instances
- **Multi-instance Support**: Work with multiple Neovim instances simultaneously
- **Connection Persistence**: Keep connections active for multiple operations

### Document Identification

Use `DocumentIdentifier` to reference files in three ways:

- `buffer_id`: Reference by Neovim buffer ID (for open files)
- `project_relative_path`: Reference by project-relative path
- `absolute_path`: Reference by absolute file path

### Diagnostic Severity Levels

- `1` = Error
- `2` = Warning
- `3` = Information
- `4` = Hint

## LSP Features

### Code Actions Workflow

Complete LSP code action lifecycle for refactoring and quick fixes:

1. **Get Actions**: `lsp_code_actions` → Retrieve available actions for range
2. **Resolve Action**: `lsp_resolve_code_action` → Complete incomplete data
3. **Apply Changes**: `lsp_apply_edit` → Apply workspace edit to files

### Call Hierarchy Workflow

Navigate function call relationships:

1. **Prepare**: `lsp_call_hierarchy_prepare` → Get hierarchy item at position
2. **Incoming**: `lsp_call_hierarchy_incoming_calls` → Find all callers
3. **Outgoing**: `lsp_call_hierarchy_outgoing_calls` → Find all callees

Requires LSP 3.16.0+ servers with call hierarchy support.

### Type Hierarchy Workflow

Navigate inheritance and implementation relationships:

1. **Prepare**: `lsp_type_hierarchy_prepare` → Get hierarchy item at position
2. **Supertypes**: `lsp_type_hierarchy_supertypes` → Find parent types/interfaces
3. **Subtypes**: `lsp_type_hierarchy_subtypes` → Find implementations/derived types

Requires LSP 3.17.0+ servers with type hierarchy support.

## Common Workflows

### Initial Setup

```text
get_targets → connect → list_buffers
```

Cache the `connection_id` for all subsequent operations.

### File Analysis and Diagnostics

1. Connect to Neovim instance (cache `connection_id`)
2. Read `nvim-diagnostics://{connection_id}/workspace` resource for
   project-wide analysis
3. Use `buffer_diagnostics` for file-specific investigation
4. Group diagnostics by severity and file
5. Keep connection active for follow-up analysis

### Symbol Navigation

1. Connect and get LSP clients (reuse `connection_id`)
2. Use `lsp_workspace_symbols` with search query for project-wide symbol search
3. Use `lsp_document_symbols` to understand file structure
4. Navigate using `navigate` tool with precise positioning
5. Keep connection active for continued navigation

### Position-Based Operations

1. Get current position with `cursor_position` (zero-based coordinates)
2. Use position for targeted operations:
   - `lsp_hover` for documentation
   - `lsp_references` for usage sites
   - `lsp_definition` for declarations
3. Navigate to results using `navigate` tool

## Resource Strategy

- **Workspace diagnostics**: Use for project-wide error analysis
- **Buffer diagnostics**: Use for file-specific investigation
- **Connection monitoring**: Use `nvim-connections://` to track active instances

## Safety Guidelines

### Safe Code Execution

- **Read-only operations**: Use `vim.inspect()`, `vim.fn.getline()`,
  `vim.api.nvim_buf_get_lines()`
- **State queries**: Use `vim.fn.getcwd()`, `vim.bo.filetype`,
  `vim.api.nvim_get_current_buf()`
- **Avoid modifications**: Don't use `vim.api.nvim_buf_set_lines()` or similar
  write operations
- **Error handling**: Wrap Lua code in `pcall()` for safe execution

### Connection Best Practices

- Cache `connection_id` values and reuse them
- Keep connections active during multi-step operations
- Use auto-connected IDs when available
- Only disconnect when completely finished with all operations
