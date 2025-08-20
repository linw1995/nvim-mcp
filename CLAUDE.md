# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working
with code in this repository.

## Project Overview

This is a Rust-based Model Context Protocol (MCP) server that provides AI
assistants with programmatic access to Neovim instances. The server supports
both Unix socket/named pipe and TCP connections, implements 23 core MCP
tools for Neovim interaction, and provides diagnostic resources through the
`nvim-diagnostics://` URI scheme. The server now supports multiple transport
modes: stdio (default), and HTTP server for web-based integrations.
The project uses Rust 2024 edition and focuses on async/concurrent operations
with proper error handling throughout.

## Development Commands

### Building and Running

```bash
# Development build and run
cargo build
cargo run

# Auto-connect to current project Neovim instances
cargo run -- --connect auto

# Connect to specific target
cargo run -- --connect 127.0.0.1:6666
cargo run -- --connect /tmp/nvim.sock

# With custom logging options
cargo run -- --log-file ./nvim-mcp.log --log-level debug

# HTTP server mode with auto-connection
cargo run -- --http-port 8080 --connect auto

# HTTP server mode with custom bind address
cargo run -- --http-port 8080 --http-host 0.0.0.0

# Production build and run
cargo build --release
nix run .

# Enter Nix development environment (skip if IN_NIX_SHELL is set)
nix develop .
```

**CLI Options:**

- `--connect <MODE>`: Connection mode (defaults to manual)
  - `manual`: Traditional workflow using get_targets and connect tools
  - `auto`: Automatically connect to all project-associated Neovim instances
  - Specific target: Direct connection to TCP address or socket path
- `--log-file <PATH>`: Log file path (defaults to stderr)
- `--log-level <LEVEL>`: Log level (trace, debug, info, warn, error;
  defaults to info)
- `--http-port <PORT>`: Enable HTTP server mode on the specified port
- `--http-host <HOST>`: HTTP server bind address (defaults to 127.0.0.1)

### Testing

```bash
# Run all tests
cargo test -- --show-output

# Run single specific module test
cargo test -- --show-output neovim::integration_tests

# Run single specific test
cargo test -- --show-output neovim::integration_tests::test_tcp_connection_lifecycle

# Skip integration tests (which require Neovim)
cargo test -- --skip=integration_tests --show-output 1

# Run tests in Nix environment (requires IN_NIX_SHELL not set)
nix develop . --command cargo test -- --show-output 1
```

**Note**: The `nix develop . --command` syntax only works when the
`IN_NIX_SHELL` environment variable is not set. If you're already in a Nix
shell, use the commands directly without the `nix develop . --command` prefix.

## Architecture Overview

The codebase follows a layered architecture:

### Core Components

The codebase follows a modular architecture with clear separation of concerns:

- **`src/server/core.rs`**: Core infrastructure and server foundation
  - Contains `NeovimMcpServer` struct and core methods
  - Integrates `HybridToolRouter` for static and dynamic tool management ⚠️ **(Experimental)**
  - Manages multiple concurrent connections via
    `Arc<DashMap<String, Box<dyn NeovimClientTrait + Send>>>`
  - Handles multi-connection lifecycle with deterministic connection IDs
  - Provides utility functions (BLAKE3 hashing, socket discovery, etc.)
  - Error conversion between `NeovimError` and `McpError`
  - Exposes dynamic tool registration API for connection-scoped tools ⚠️ **(Experimental)**

- **`src/server/tools.rs`**: MCP tool implementations
  - Implements 23 MCP tools using the `#[tool]` attribute
  - Contains parameter structs for tool requests
  - Focuses purely on MCP tool logic and protocol implementation
  - Clean separation from core infrastructure

- **`src/server/hybrid_router.rs`** ⚠️ **(Experimental)**: Dynamic tool routing system
  - Implements `HybridToolRouter` that combines static and dynamic tools
  - Supports connection-scoped dynamic tool registration
  - Provides automatic tool lifecycle management with connection cleanup
  - Uses lock-free concurrent data structures for high performance
  - Enables extensibility while maintaining backwards compatibility

- **`src/server/lua_tools.rs`** ⚠️ **(Experimental)**: Lua custom tool
  integration system
  - Implements `LuaToolConfig` structure for Lua-defined tool configuration
  - Provides `discover_lua_tools()` function for automatic tool discovery from Neovim
  - Implements `LuaToolValidator` for JSON Schema parameter validation
  - Handles conversion between Neovim msgpack values and JSON for seamless integration
  - Provides `discover_and_register_lua_tools()` for automatic registration
    during connection setup
  - Implements error handling and response conversion for Lua MCP responses

- **`src/server/resources.rs`**: MCP resource handlers
  - Implements `ServerHandler` trait for MCP capabilities
  - Uses `HybridToolRouter` for dynamic tool discovery and execution
  - Supports `nvim-diagnostics://` URI scheme for diagnostic resources
  - Handles resource listing and reading operations

- **`src/neovim/client.rs`**: Neovim client abstraction layer
  - Implements `NeovimClientTrait` for unified client interface
  - Supports both TCP and Unix socket/named pipe connections
  - Provides high-level operations: buffer management, diagnostics, LSP integration
  - Handles Lua code execution and autocmd setup

- **`src/neovim/connection.rs`**: Connection management layer
  - Wraps `nvim-rs` client with lifecycle management
  - Tracks connection address and background I/O tasks

### Architecture Benefits

This modular architecture provides several advantages:

- **Clear Separation of Concerns**: Core infrastructure, MCP tools, and
  resource handlers are cleanly separated (dynamic routing ⚠️
  **experimental**)
- **Extensibility**: `HybridToolRouter` enables dynamic tool registration
  without code changes ⚠️ **(Experimental)**
- **Performance**: Lock-free concurrent data structures for high-throughput
  tool routing ⚠️ **(Experimental)**
- **Easier Maintenance**: Each file has a single, well-defined responsibility
- **Better Testing**: Components can be tested independently with focused unit tests
- **Improved Readability**: Developers can quickly find relevant code based on functionality
- **Scalable Development**: New tools and resources can be added without
  affecting core logic
- **Reduced Coupling**: Changes to tool implementations don't impact core
  server infrastructure

### Data Flow

1. **MCP Communication**: stdio/HTTP transport ↔ MCP client ↔ `NeovimMcpServer`
2. **Tool Routing** ⚠️ **(Experimental)**: MCP tool request →
   `HybridToolRouter` → static/dynamic tool execution
3. **Neovim Integration**: `NeovimMcpServer` → `NeovimClientTrait` → `nvim-rs` →
   TCP/Unix socket → Neovim instance
4. **Tool Execution**: Routed tool call → async Neovim API call → response
5. **Resource Access**: MCP resource request → diagnostic data retrieval →
   structured JSON response

### Connection Management

- **Multi-connection support**: Multiple concurrent Neovim instances managed simultaneously
- **Thread-safe access** using `Arc<DashMap<String, Box<dyn NeovimClientTrait + Send>>>`
- **Deterministic connection IDs** generated using BLAKE3 hash of target string
- **Connection isolation**: Each connection operates independently with
  proper session isolation
- **Dynamic tool lifecycle** ⚠️ **(Experimental)**: Connection-scoped tools
  automatically cleaned up on disconnect
- **Proper cleanup** of TCP connections and background tasks on disconnect
- **Connection validation** before tool execution using connection ID lookup

### Multi-Connection Architecture Benefits

**Performance Advantages:**

- **Lock-free reads**: DashMap enables concurrent read access without blocking
- **Fine-grained locking**: Only write operations require locks, not
  entire connection map access
- **Fast hashing**: BLAKE3 provides extremely fast deterministic connection ID generation
- **Independent operations**: Each connection operates concurrently
  without affecting others

**Reliability Features:**

- **Deterministic IDs**: Same target always produces same connection ID
  for predictable behavior
- **Connection replacement**: Connecting to existing target gracefully
  replaces previous connection
- **Session isolation**: Connections don't interfere with each other's state
- **Graceful cleanup**: Proper resource deallocation on disconnect
  prevents memory leaks

**Developer Experience:**

- **Predictable workflow**: Connection IDs are consistent across sessions
- **Clear separation**: Connection-scoped resources eliminate ambiguity
- **Concurrent debugging**: Multiple development environments can run simultaneously

### Available MCP Tools

The server provides these 23 tools (implemented with `#[tool]` attribute):

**Connection Management:**

1. **`get_targets`**: Discover available Neovim socket paths created by the
   nvim-mcp plugin
2. **`connect`**: Connect via Unix socket/named pipe, returns deterministic `connection_id`
3. **`connect_tcp`**: Connect via TCP address, returns deterministic `connection_id`
4. **`disconnect`**: Disconnect from specific Neovim instance by `connection_id`

**Connection-Aware Tools** (require `connection_id` parameter):

1. **`list_buffers`**: List all open buffers for specific connection
2. **`exec_lua`**: Execute arbitrary Lua code in specific Neovim instance
3. **`buffer_diagnostics`**: Get diagnostics for specific buffer on specific connection
4. **`lsp_clients`**: Get workspace LSP clients for specific connection
5. **`lsp_workspace_symbols`**: Search workspace symbols by query on specific
   connection
6. **`lsp_code_actions`**: Get LSP code actions with universal document
   identification (supports buffer IDs, project-relative paths, and absolute paths)
7. **`lsp_hover`**: Get LSP hover information with universal document
   identification (supports buffer IDs, project-relative paths, and absolute paths)
8. **`lsp_document_symbols`**: Get document symbols with universal document
   identification (supports buffer IDs, project-relative paths, and absolute paths)
9. **`lsp_references`**: Get LSP references with universal document
   identification (supports buffer IDs, project-relative paths, and absolute paths)
10. **`lsp_resolve_code_action`**: Resolve code actions that may have
    incomplete data
11. **`lsp_apply_edit`**: Apply workspace edits using Neovim's LSP utility
    functions
12. **`lsp_definition`**: Get LSP definition with universal document identification
13. **`lsp_type_definition`**: Get LSP type definition with universal document identification
14. **`lsp_implementations`**: Get LSP implementations with universal document identification
15. **`lsp_declaration`**: Get LSP declaration with universal document identification
16. **`lsp_rename`**: Rename symbol across workspace using LSP
17. **`lsp_formatting`**: Format document using LSP with optional auto-apply
18. **`lsp_range_formatting`**: Format a specific range in a document using LSP
19. **`lsp_organize_imports`**: Sort and organize imports using LSP with
    auto-apply by default

### Universal Document Identifier System

The server now includes a universal document identifier system that enhances
LSP operations
by supporting multiple ways of referencing documents:

- **Buffer IDs**: For currently open files in Neovim (`BufferId(u64)`)
  - JSON format: `{"buffer_id": 123}`
- **Project-relative paths**: For files relative to the project root (`ProjectRelativePath(PathBuf)`)
  - JSON format: `{"project_relative_path": "src/main.rs"}`
- **Absolute file paths**: For files with absolute filesystem paths (`AbsolutePath(PathBuf)`)
  - JSON format: `{"absolute_path": "/home/user/project/src/main.rs"}`

This system enables LSP operations on files that may not be open in Neovim
buffers, providing
enhanced flexibility for code analysis and navigation. The universal LSP tools
(`lsp_code_actions`, `lsp_hover`, `lsp_document_symbols`, `lsp_references`,
`lsp_definition`, `lsp_type_definition`, `lsp_implementations`,
`lsp_declaration`, `lsp_rename`, `lsp_formatting`, `lsp_range_formatting`,
`lsp_organize_imports`) accept any of these
document identifier types.

### MCP Resources

The server provides connection-aware resources via multiple URI schemes:

**Connection Management:**

- **`nvim-connections://`**: Lists all active Neovim connections with
  their IDs and targets

**Tool Registration Overview** ⚠️ **(Experimental)**:

- **`nvim-tools://`**: Overview of all tools and their connection mappings,
  showing static tools (available to all connections) and dynamic tools
  (connection-specific)
- **`nvim-tools://{connection_id}`**: List of tools available for a specific
  connection, including both static and connection-specific dynamic tools

*Note: Tool registration resources are experimental and may change in future versions.*

**Connection-Scoped Diagnostics** via `nvim-diagnostics://` URI scheme:

- **`nvim-diagnostics://{connection_id}/workspace`**: All diagnostic
  messages across workspace for specific connection
- **`nvim-diagnostics://{connection_id}/buffer/{buffer_id}`**: Diagnostics
  for specific buffer on specific connection

Resources return structured JSON with diagnostic information including severity,
messages, file paths, and line/column positions. Connection IDs are deterministic
BLAKE3 hashes of the target string for consistent identification.

## Dynamic Tool System ⚠️ **(Experimental)**

⚠️ **Warning**: The dynamic tool system is experimental and unstable.
It may change significantly or be removed in future versions without
prior notice. Use at your own risk in production environments.

The server includes a sophisticated dynamic tool registration system through
`HybridToolRouter` with comprehensive Lua integration for user-extensible
custom tools:

### HybridToolRouter Architecture

**Core Design:**

- **Dual Tool Support**: Combines static tools (from `#[tool_router]` macro)
  with dynamic tools
- **Connection-Scoped Tools**: Tools that are automatically
  registered/unregistered with connection lifecycle
- **Conflict Resolution**: Prevents naming conflicts between static and dynamic tools
- **Performance Optimized**: Lock-free concurrent access using `Arc<DashMap>`
- **Clean Tool Names**: Connection-specific tools maintain clean names without prefixes
- **Tool Visibility**: New resource system provides insight into
  tool-connection mappings

**Key Components:**

```rust
pub struct HybridToolRouter {
    /// Static tools from #[tool_router] macro
    static_router: ToolRouter<NeovimMcpServer>,

    /// Dynamic tools using nested structure:
    /// tool_name -> connection_id -> tool
    dynamic_tools: Arc<DashMap<String, DashMap<String, DynamicToolBox>>>,

    /// Connection-specific tool mapping: connection_id -> tool_names
    connection_tools: Arc<DashMap<String, HashSet<String>>>,
}
```

### Lua Dynamic Tools System

**User-Extensible Tool Registration:**

The system now supports custom tool registration through Neovim
configuration using Lua functions:

```lua
-- In your Neovim configuration (init.lua or plugin config)
require("nvim-mcp").setup({
    custom_tools = {
        save_buffer = {
            description = "Save a specific buffer by ID",
            parameters = {
                type = "object",
                properties = {
                    buffer_id = {
                        type = "integer",
                        description = "The buffer ID to save",
                        minimum = 1,
                    },
                },
                required = { "buffer_id" },
            },
            handler = function(params)
                -- Tool implementation using MCP helpers
                return MCP.success({
                    buffer_id = params.buffer_id,
                    message = "Buffer saved successfully",
                })
            end,
        },
    },
})
```

**MCP Helper Functions:**

The Lua plugin provides helper functions for creating compatible MCP responses:

- `MCP.success(data)`: Create successful tool response
- `MCP.error(code, message, data)`: Create error response
- `MCP.text(text)`: Create plain text response
- `MCP.json(data)`: Create JSON response

### Dynamic Tool Registration

**Connection-Scoped Tools:**

- Tools registered with format: `{connection_id}_{tool_name}`
- Automatically cleaned up when connection disconnects
- Isolated per connection to prevent interference
- Perfect for connection-specific functionality

**Tool Lifecycle Management:**

1. **Configuration**: Users define tools in Neovim setup with Lua
   handlers
2. **Discovery**: MCP server queries Neovim for registered tools via
   `get_registered_tools()`
3. **Registration**: Tools are registered in `HybridToolRouter` during
   connection setup
4. **Validation**: JSON Schema validation using `jsonschema` crate for
   parameter checking
5. **Execution**: Routed through `HybridToolRouter` with priority:
   dynamic → static
6. **Cleanup**: Connection-scoped tools automatically removed on
   disconnect

### Integration with Core Server

**NeovimMcpServer Integration:**

```rust
impl NeovimMcpServer {
    /// Register a connection-specific tool with clean name
    pub fn register_dynamic_tool(
        &self,
        connection_id: &str,
        tool: DynamicToolBox,
    ) -> Result<(), McpError>

    /// Remove all dynamic tools for a connection
    pub fn unregister_dynamic_tools(&self, connection_id: &str)
}
```

**Lua Tool Integration:**

The server automatically discovers and registers Lua-defined tools through
the new `lua_tools.rs` module:

- **`LuaToolConfig`**: Structure for Lua tool configuration and execution
- **`discover_lua_tools()`**: Queries Neovim for configured custom tools
- **`discover_and_register_lua_tools()`**: Automatic registration during
  connection setup
- **JSON Schema Validation**: Parameter validation using `jsonschema` crate
- **Response Conversion**: Seamless conversion between Lua MCP responses
  and Rust `CallToolResult`

**Benefits:**

- **User Extensibility**: Users can define custom tools without modifying
  server code
- **Project-Specific Workflows**: Different Neovim instances can have
  different tool sets
- **Automatic Discovery**: Tools are discovered and registered during
  connection setup
- **Parameter Validation**: Robust input validation using JSON Schema
- **Error Handling**: Structured error reporting with safe Lua execution
- **Connection Isolation**: Tools are scoped to specific connections
- **Performance**: Efficient tool routing with minimal overhead
- **Reliability**: Automatic cleanup prevents resource leaks

## Key Dependencies

- **`rmcp`**: MCP protocol implementation with stdio transport, streamable
  HTTP server transport, and client features
- **`nvim-rs`**: Neovim msgpack-rpc client (with tokio feature)
- **`tokio`**: Async runtime for concurrent operations (full feature set)
- **`tracing`**: Structured logging with subscriber and appender support
- **`clap`**: CLI argument parsing with derive features
- **`thiserror`**: Ergonomic error handling and error type derivation

**Multi-Connection Support Dependencies:**

- **`dashmap`**: Lock-free concurrent HashMap for connection storage
- **`regex`**: Pattern matching for connection-scoped resource URI parsing
- **`blake3`**: Fast, deterministic hashing for connection ID generation

**Dynamic Tool System Dependencies** ⚠️ **(Experimental)**:

- **`jsonschema`**: JSON Schema validation for Lua custom tool parameters
- **`serde_json`**: JSON serialization/deserialization with enhanced
  support for Lua integration
- **`async-trait`**: Async trait support for dynamic tool execution

**HTTP Server Transport Dependencies:**

- **`hyper`**: High-performance HTTP library for HTTP server transport
- **`hyper-util`**: Utilities for hyper with server and service features
- **`tower-http`**: HTTP middleware and utilities with CORS support

**Testing and Development Dependencies:**

- **`tempfile`**: Temporary file and directory management for integration tests
- **Enhanced deserialization**: Support for both string and struct formats
  in CodeAction and WorkspaceEdit types
- **Lua tool testing** ⚠️ **(Experimental)**: Integration tests for custom tool registration
  and execution

## Testing Architecture

- **Integration tests**: Located in `src/server/integration_tests.rs` and
  `src/neovim/integration_tests.rs`
- **Global mutex**: Prevents port conflicts during concurrent test execution
- **Automated setup**: Tests spawn and manage Neovim instances automatically
- **Full MCP flow**: Tests cover complete client-server communication
- **LSP testing**: Comprehensive Go integration tests with gopls language server
- **Code action testing**: End-to-end tests for lsp_resolve_code_action and
  lsp_apply_edit
- **Test data**: Includes Go source files and LSP configuration for realistic
  testing scenarios

## Error Handling

- **Layered errors**: `ServerError` (top-level) and `NeovimError` (Neovim-specific)
- **MCP compliance**: Errors are properly formatted for MCP protocol responses
- **Comprehensive propagation**: I/O and nvim-rs errors are properly converted

## Adding New MCP Tools

To add a new connection-aware tool to the server:

1. **Add parameter struct** in `src/server/tools.rs` with `serde::Deserialize` and
   `schemars::JsonSchema` derives
   - **For connection-aware tools**: Include `connection_id: String` parameter
   - **For connection management**: Use existing parameter types or create new ones

2. **Add tool method** to `NeovimMcpServer` impl in `src/server/tools.rs`
   - Use the `#[tool(description = "...")]` attribute with `#[instrument(skip(self))]`
   - Return `Result<CallToolResult, McpError>`
   - Import `NeovimMcpServer` from `super::core`

3. **Connection validation**: Use `self.get_connection(&connection_id)?` to validate
   and retrieve the specific Neovim connection (method available from core)

4. **Tool implementation**: Use the retrieved client reference for Neovim operations

5. **Testing**: Update integration tests in `src/server/integration_tests.rs`

6. **Registration**: The tool is automatically registered by the
   `#[tool_router]` macro and handled through `HybridToolRouter` ⚠️ **(Experimental)**

**New Tool Parameter Structures:**

For the recently added LSP tools, the following parameter structures are used:

```rust
/// Resolve code action parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ResolveCodeActionParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// LSP client name
    pub lsp_client_name: String,
    /// Code action to resolve
    pub code_action: CodeAction,
}

/// Apply workspace edit parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ApplyWorkspaceEditParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// LSP client name (used for position encoding detection)
    pub lsp_client_name: String,
    /// Workspace edit to apply using vim.lsp.util.apply_workspace_edit()
    pub workspace_edit: WorkspaceEdit,
}
```

**Example connection-aware tool pattern:**

```rust
// In src/server/tools.rs

/// Your parameter struct
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct YourConnectionRequest {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    // Add other parameters as needed
}

// In the NeovimMcpServer impl block
#[tool(description = "Your tool description")]
#[instrument(skip(self))]
pub async fn your_tool(
    &self,
    Parameters(YourConnectionRequest { connection_id, /* other_params */ }): Parameters<YourConnectionRequest>,
) -> Result<CallToolResult, McpError> {
    let client = self.get_connection(&connection_id)?;
    // Use client for Neovim operations...
    Ok(CallToolResult::success(vec![Content::json(result)?]))
}
```

**Required imports in tools.rs:**

```rust
use super::core::{NeovimMcpServer, /* other utilities */};
use rmcp::{ErrorData as McpError, /* other MCP types */};
```

## Development Environment

This project uses Nix flakes for reproducible development environments.
The flake provides:

- Rust toolchain (stable) with clippy, rustfmt, and rust-analyzer
- Neovim 0.11.3+ for integration testing
- Pre-commit hooks for code quality

Use `nix develop .` to enter the development shell (only if `IN_NIX_SHELL` is
not already set) or set up direnv with `echo 'use flake' > .envrc` for
automatic environment activation.

### Code Formatting

The project uses `stylua.toml` for Lua code formatting. Rust code follows
standard rustfmt conventions.

## Neovim Lua Plugin

The project includes a comprehensive Neovim Lua plugin at
`lua/nvim-mcp/init.lua` that:

**Core RPC Functionality:**

- Automatically starts a Neovim RPC server on a Unix socket/named pipe
- Generates unique pipe paths based on git root and process ID
- Provides a `setup()` function for initialization
- Enables seamless MCP server connection without manual TCP setup

**Custom Tool Registration** ⚠️ **(Experimental)**:

- Supports user-defined custom tools through `custom_tools` configuration
- Maintains a global tool registry (`_tool_registry`) for configured tools
- Provides `get_registered_tools()` API for MCP server tool discovery
- Implements `execute_tool()` API for safe tool execution with error handling

**MCP Helper Functions:**

- `MCP.success(data)`: Create successful tool responses
- `MCP.error(code, message, data)`: Create structured error responses
- `MCP.text(text)`: Create plain text responses
- `MCP.json(data)`: Create JSON responses

**Tool Management Features:**

- Automatic tool validation during setup
- Safe tool execution using `pcall()` for error handling
- Structured error reporting with MCP-compatible format
- Connection-scoped tool isolation

This eliminates the need to manually start Neovim with `--listen` for MCP
server connections and enables users to define project-specific custom tools
through their Neovim configuration.
