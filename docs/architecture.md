# Architecture Overview

The codebase follows a layered architecture with clear separation of concerns.

## Core Components

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
  - Includes configurable LSP timeout settings via `NeovimClientConfig`
  - Features comprehensive notification tracking system for LSP synchronization
  - Provides methods for waiting on LSP readiness and diagnostic availability

- **`src/neovim/connection.rs`**: Connection management layer
  - Wraps `nvim-rs` client with lifecycle management
  - Tracks connection address and background I/O tasks

## Architecture Benefits

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

## Data Flow

1. **MCP Communication**: stdio/HTTP transport ↔ MCP client ↔ `NeovimMcpServer`
2. **Tool Routing** ⚠️ **(Experimental)**: MCP tool request →
   `HybridToolRouter` → static/dynamic tool execution
3. **Neovim Integration**: `NeovimMcpServer` → `NeovimClientTrait` → `nvim-rs` →
   TCP/Unix socket → Neovim instance
4. **Tool Execution**: Routed tool call → async Neovim API call → response
5. **Resource Access**: MCP resource request → diagnostic data retrieval →
   structured JSON response

## Connection Management

- **Multi-connection support**: Multiple concurrent Neovim instances managed simultaneously
- **Thread-safe access** using `Arc<DashMap<String, Box<dyn NeovimClientTrait + Send>>>`
- **Deterministic connection IDs** generated using BLAKE3 hash of target string
- **Connection isolation**: Each connection operates independently with
  proper session isolation
- **Dynamic tool lifecycle** ⚠️ **(Experimental)**: Connection-scoped tools
  automatically cleaned up on disconnect
- **Proper cleanup** of TCP connections and background tasks on disconnect
- **Connection validation** before tool execution using connection ID lookup

## Multi-Connection Architecture Benefits

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
