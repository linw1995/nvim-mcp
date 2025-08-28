# Changelog

<!-- markdownlint-configure-file
{
  "no-duplicate-heading": false
}
-->

All notable changes to this project will be documented in this file.

## [Unreleased]

### Fixed

- **FormattingOptions Deserialization**: Fixed `FormattingOptions` to support both
  string and struct deserialization formats for better compatibility with various
  MCP clients
- **Dynamic Tools Plugin Dependency** (#62): Added plugin availability check before
  Lua tool discovery to prevent errors when nvim-mcp plugin is not installed.
  Server now gracefully handles missing plugin and continues with static tools only

### New Features

- **Cursor Position Tool**: Added `cursor_position` tool to get current cursor
  position with buffer name and zero-based row/col coordinates
- **LSP Readiness Tool**: Added `wait_for_lsp_ready` tool for ensuring LSP client
  readiness before performing operations, improving reliability of LSP workflows

### Improved

- **Configurable LSP Timeout**: Added `NeovimClientConfig` with configurable LSP
  timeout settings (default: 3000ms) for better control over LSP operation timing
- **Enhanced Notification Tracking**: Comprehensive notification tracking system
  for robust LSP synchronization and event handling
- **Autocmd Setup**: Unified autocmd setup replacing diagnostics-specific
  implementation with more comprehensive event handling
- **Test Performance**: Optimized integration tests with better timing and
  one-time binary compilation for improved CI performance
- **Code Coverage**: Use **grcov** for LLVM-based code coverage
  with HTML, Cobertura, and Markdown report generation
- **CI Coverage Integration**: Added codecov.io integration with automated
  coverage reporting in GitHub Actions

## [v0.5.0] - 2025-08-20

### Fixed

- **LSP Workspace Symbols**: Fixed `lsp_workspace_symbols` return type to use
  `Option<DocumentSymbolResult>` instead of `WorkspaceSymbolResult` for
  consistency with other LSP tools
- **Diagnostic Schema**: Relaxed Diagnostic JSON schema to accept code as
  number or string for better compatibility

### New Features

- **Automatic Connection**: Added automatic connection feature with CLI support
  for seamless integration with current project Neovim instances
- **Project-Scoped Auto-Discovery**: Automatically find and connect to Neovim
  instances associated with the current project directory
- **Flexible Connection Modes**: Support for manual, automatic, and specific
  target connection modes via CLI
- **HTTP Server Transport**: Added HTTP server mode for web-based integrations
  with streamable HTTP transport support
- **Multi-Transport Support**: Server now supports both stdio (default) and
  HTTP server transport modes for different integration scenarios

### Experimental Features (Unstable)

⚠️ **Warning**: The following features are experimental and unstable. They may
change significantly or be removed in future versions without prior notice.
Use at your own risk.

- **Dynamic Tool System**: Sophisticated dynamic tool registration system through
  `HybridToolRouter` enabling extensible tool functionality without code changes
- **Lua Integration**: Custom tool registration through Neovim configuration
  using Lua functions with automatic discovery and validation
- **Connection-Scoped Tools**: Tools automatically registered/unregistered with
  connection lifecycle for enhanced modularity
- **Tool Visibility**: Enhanced tool visibility through new resource system
- **Tool Registration Resources**: New `nvim-tools://` URI scheme for monitoring
  tool availability and connection mappings
- **HybridToolRouter**: Combines static tools (from `#[tool_router]` macro) with
  dynamic tools using lock-free concurrent data structures
- **Conflict Resolution**: Prevents naming conflicts between static and dynamic tools
- **Dynamic Routing**: Enhanced modular architecture with dynamic routing
  capabilities
- **Tool Registration API**: Improved extensibility through dynamic tool
  registration API
- **MCP Helper Functions**: Lua plugin provides helper functions (`MCP.success`,
  `MCP.error`, `MCP.text`, `MCP.json`) for creating compatible MCP responses

### New CLI Options

- `--connect <MODE>` - Connection mode: 'manual' (default), 'auto', or specific
  target (TCP address/socket path)
  - `manual`: Traditional workflow using get_targets and connect tools
  - `auto`: Automatically connect to all project-associated Neovim instances
  - Specific target: Direct connection to TCP address or socket path
- `--http-port <PORT>` - Enable HTTP server mode on the specified port
- `--http-host <HOST>` - HTTP server bind address (defaults to 127.0.0.1)

### Auto-Connection Behavior

- **Project Detection**: Automatically detects current project root using git
  repository or working directory
- **Socket Pattern Matching**: Finds Neovim instances using project-specific
  socket naming patterns
- **Graceful Fallback**: Continues serving with manual connection capability
  if auto-connection fails
- **Connection Validation**: Validates target formats and provides clear error
  messages for invalid targets

### Dependencies

- Added `hyper` for high-performance HTTP server transport
- Added `hyper-util` for HTTP utilities with server and service features
- Added `tower-http` for HTTP middleware and CORS support
- Added `jsonschema` for JSON Schema validation in Lua custom tool parameters
- Updated `rmcp` to include streamable HTTP server transport features

## [v0.4.0] - 2025-08-16

### New Features

- **LSP Import Organization**: Added `lsp_organize_imports` tool for sorting and
  organizing imports using LSP with auto-apply enabled by default
- **LSP Document Range Formatting**: Added `lsp_range_formatting` tool for
  formatting specific ranges in documents using LSP with support for LSP 3.15.0+
  formatting preferences
- **LSP Document Formatting**: Added `lsp_formatting` tool for formatting documents
  using LSP with support for LSP 3.15.0+ formatting preferences
- **LSP Symbol Renaming**: Added `lsp_rename` tool for renaming symbols across
  workspace with optional prepare rename validation
- **LSP Declaration Support**: Added `lsp_declaration` tool for finding symbol
  declarations with universal document identification

### New Tools (5 additional, 23 total)

**Enhanced LSP Integration:**

- `lsp_organize_imports` - Sort and organize imports using LSP with auto-apply
  enabled by default (buffer IDs, project paths, absolute paths)
- `lsp_range_formatting` - Format a specific range in a document using LSP with
  support for LSP 3.15.0+ formatting preferences and optional auto-apply
  (buffer IDs, project paths, absolute paths)
- `lsp_formatting` - Format document using LSP with support for LSP 3.15.0+
  formatting preferences and optional auto-apply (buffer IDs, project paths,
  absolute paths)
- `lsp_rename` - Rename symbol across workspace using LSP with optional
  validation via prepare rename (buffer IDs, project paths, absolute paths)
- `lsp_declaration` - Get LSP declaration with universal document identification
  (buffer IDs, project paths, absolute paths)

## [v0.3.0] - 2025-08-15

### New Features

- **LSP Implementation Support**: Added `lsp_implementations` tool for finding
  interface/abstract class implementations with universal document
  identification (#33)
- **LSP Definition and Type Definition Support**: Added `lsp_definition` and
  `lsp_type_definition` tools for comprehensive symbol navigation with universal
  document identification

### New Tools (3 additional, 18 total)

**Enhanced LSP Integration:**

- `lsp_implementations` - Get LSP implementations with universal document
  identification (buffer IDs, project paths, absolute paths)
- `lsp_definition` - Get LSP definition with universal document identification
  (buffer IDs, project paths, absolute paths)
- `lsp_type_definition` - Get LSP type definition with universal document
  identification (buffer IDs, project paths, absolute paths)

### Fixed

- **Package Metadata**: Fixed commit SHA detection for crates.io packages (#38)
- **Rust Compatibility**: Added minimum supported Rust version (MSRV) requirement
  to prevent cryptic let-chains errors on older Rust compilers (#37)

### Infrastructure

- **Build System**: Enhanced crate metadata and build-time information

## [v0.2.0] - 2025-08-14

### New Features

- **Universal Document Identifier System**: Enhanced LSP operations
  supporting buffer IDs, project-relative paths, and absolute file paths (#15)
- **Complete LSP Code Action Workflow**: Full lifecycle support for code
  actions with resolve and apply capabilities (#20)
- **Enhanced Symbol Navigation**: Workspace symbol search and document symbol analysis
- **Advanced LSP Integration**: References tracking and comprehensive code
  analysis tools

### New Tools (3 additional, 13 total)

**Enhanced LSP Integration:**

- `lsp_workspace_symbols` - Search workspace symbols by query
- `lsp_references` - Get LSP references with universal document identification
- `lsp_resolve_code_action` - Resolve code actions with incomplete data
- `lsp_apply_edit` - Apply workspace edits using Neovim's LSP utility functions

**Universal LSP Tools** (enhanced existing tools):

- `lsp_code_actions` - Now supports universal document identification
  (buffer IDs, project paths, absolute paths)
- `lsp_hover` - Enhanced with universal document identification
- `lsp_document_symbols` - Get document symbols with universal document identification

### Installation Improvements

- **Primary Installation**: Now available via `cargo install nvim-mcp` from crates.io
- **Alternative Methods**: Nix and source installation still supported

### Technical Enhancements

- Build-time metadata with Git information and timestamp (#28)
- Enhanced DocumentIdentifier deserialization for Claude Code compatibility
- Complete LSP code action lifecycle with native Neovim integration

### Fixed

- Connection resource leak in connect and connect_tcp tools (#13)
- Updated dependencies and fixed rmcp API compatibility

## [v0.1.0] - 2025-08-08

### Features

- **Multi-Connection Support**: Manage multiple concurrent Neovim instances
  with deterministic connection IDs
- **Connection Management**: Connect via TCP or Unix socket/named pipe
  with automatic discovery
- **Buffer Operations**: List and inspect all open buffers with detailed information
- **Diagnostics Access**: Retrieve diagnostics for buffers with error/warning details
- **LSP Integration**: Access code actions and LSP client information
- **MCP Resources**: Structured diagnostic data via connection-aware URI schemes
- **Lua Execution**: Execute arbitrary Lua code directly in Neovim
- **Plugin Integration**: Automatic setup through Neovim plugin
- **Modular Architecture**: Clean separation between core infrastructure,
  MCP tools, and resource handlers

### Tools (10 available)

**Connection Management:**

- `get_targets` - Discover available Neovim targets
- `connect` - Connect via Unix socket/named pipe
- `connect_tcp` - Connect via TCP
- `disconnect` - Disconnect from specific Neovim instance

**Buffer Operations:**

- `list_buffers` - List all open buffers with names and line counts
- `buffer_diagnostics` - Get diagnostics for a specific buffer

**LSP Integration:**

- `lsp_clients` - Get workspace LSP clients
- `buffer_code_actions` - Get available code actions for buffer range
- `buffer_hover` - Get symbol hover information via LSP

**Code Execution:**

- `exec_lua` - Execute Lua code in Neovim

### Resources

**Connection Monitoring:**

- `nvim-connections://` - List all active Neovim connections

**Connection-Scoped Diagnostics:**

- `nvim-diagnostics://{connection_id}/workspace` - All diagnostic messages
  across workspace
- `nvim-diagnostics://{connection_id}/buffer/{buffer_id}` - Diagnostics
  for specific buffer
