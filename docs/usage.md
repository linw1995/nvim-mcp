# Usage Guide

This guide covers detailed usage patterns, workflows, and transport modes for the
nvim-mcp server.

## Quick Start

### 1. Start the Server

```bash
# Start as stdio MCP server (default, manual connection mode)
nvim-mcp

# Auto-connect to current project Neovim instances
nvim-mcp --connect auto

# Connect to specific target (TCP address or socket path)
nvim-mcp --connect 127.0.0.1:6666
nvim-mcp --connect /tmp/nvim.sock

# With custom logging
nvim-mcp --log-file ./nvim-mcp.log --log-level debug

# HTTP server mode with auto-connection
nvim-mcp --http-port 8080 --connect auto

# HTTP server mode with custom bind address
nvim-mcp --http-port 8080 --http-host 0.0.0.0
```

### 2. Setup Neovim Integration

#### Option A: Using Neovim Plugin (Recommended)

With a plugin manager like `lazy.nvim`:

```lua
return {
    "linw1995/nvim-mcp",
    -- install the mcp server binary automatically
    -- build = "cargo install --path .",
    build = [[
      nix build .#nvim-mcp
      nix profile remove nvim-mcp
      nix profile install .#nvim-mcp
    ]],
    opts = {},
}
```

This plugin automatically creates a Unix socket/pipe for MCP connections.

#### Option B: Manual TCP Setup

Start Neovim with TCP listening:

```bash
nvim --listen 127.0.0.1:6666
```

Or add to your Neovim config:

```lua
vim.fn.serverstart("127.0.0.1:6666")
```

## Command Line Options

- `--connect <MODE>`: Connection mode (default: manual)
  - `manual`: Traditional workflow using get_targets and connect tools
  - `auto`: Automatically connect to all project-associated Neovim instances
  - Specific target: TCP address (e.g., `127.0.0.1:6666`) or absolute socket path
- `--log-file <PATH>`: Path to log file (defaults to stderr)
- `--log-level <LEVEL>`: Log level (trace, debug, info, warn, error;
  defaults to info)
- `--http-port <PORT>`: Enable HTTP server mode on the specified port
- `--http-host <HOST>`: HTTP server bind address (defaults to 127.0.0.1)

## Usage Workflows

Once both the MCP server and Neovim are running, here are the available workflows:

### Automatic Connection Mode (Recommended)

When using `--connect auto`, the server automatically discovers and connects to
Neovim instances associated with your current project:

1. **Start server with auto-connect**:

   ```bash
   nvim-mcp --connect auto
   ```

2. **Server automatically**:
   - Detects current project root (git repository or working directory)
   - Finds all Neovim instances for the current project
   - Establishes connections with deterministic `connection_id`s
   - Reports connection status and IDs
3. **Use connection-aware tools directly**:
   - Server logs will show the `connection_id`s for connected instances
   - Use tools like `list_buffers`, `buffer_diagnostics`, etc. with these IDs
   - Access resources immediately without manual connection setup

### Specific Target Mode

For direct connection to a known target:

1. **Connect to specific target**:

   ```bash
   # TCP connection
   nvim-mcp --connect 127.0.0.1:6666

   # Unix socket connection
   nvim-mcp --connect /tmp/nvim.sock
   ```

2. **Server automatically connects and reports the `connection_id`**
3. **Use connection-aware tools with the reported ID**

### Manual Connection Mode (Traditional)

For traditional discovery-based workflow:

1. **Discover available Neovim instances**:
   - Use `get_targets` tool to list available socket paths
2. **Connect to Neovim**:
   - Use `connect` tool with a socket path from step 1
   - Save the returned `connection_id` for subsequent operations
3. **Perform operations**:
   - Use tools like `list_buffers`, `buffer_diagnostics`, etc. with your
     `connection_id`
   - Access resources like `nvim-connections://` or
     `nvim-diagnostics://{connection_id}/workspace`
4. **Optional cleanup**:
   - Use `disconnect` tool when completely done

## HTTP Server Transport

The server supports HTTP transport mode for web-based integrations and
applications that cannot use stdio transport. This is useful for web
applications, browser extensions, or other HTTP-based MCP clients.

### Starting HTTP Server Mode

```bash
# Start HTTP server on default localhost:8080
nvim-mcp --http-port 8080

# Bind to all interfaces
nvim-mcp --http-port 8080 --http-host 0.0.0.0

# With custom logging
nvim-mcp --http-port 8080 --log-file ./nvim-mcp.log --log-level debug
```

### HTTP Transport Features

- **Streamable HTTP**: Uses streamable HTTP server transport for real-time communication
- **Stateful Mode**: Maintains session state across HTTP requests
- **CORS Support**: Includes CORS middleware for cross-origin requests
- **Concurrent Sessions**: Supports multiple concurrent HTTP client sessions

### HTTP Client Integration

When using HTTP transport, MCP clients should connect to the HTTP endpoint
instead of stdio. The exact integration depends on your MCP client library,
but generally involves:

1. Configure client to use HTTP transport instead of stdio
2. Point to the server URL (e.g., `http://localhost:8080`)
3. Use the same MCP tools and resources as with stdio transport

The HTTP transport maintains full compatibility with all existing MCP tools
and resources - only the transport layer changes.

## Multi-Connection Architecture

The server supports managing multiple concurrent Neovim instances through a
multi-connection architecture with several key benefits:

### Architecture Features

- **Deterministic Connection IDs**: Each connection gets a consistent ID based
  on BLAKE3 hashing of the target string
- **Independent Sessions**: Each Neovim instance operates independently without
  interfering with others
- **Thread-Safe Operations**: Concurrent access to multiple connections using
  lock-free data structures
- **Connection Isolation**: Diagnostics and resources are scoped to specific
  connections

### Typical Workflow

1. **Discovery**: Use `get_targets` to find available Neovim socket paths
2. **Connection**: Use `connect` or `connect_tcp` to establish connection and
   get `connection_id`
3. **Operations**: Use connection-aware tools with the `connection_id` parameter
4. **Resource Access**: Read connection-scoped resources using the
   `connection_id` in URI patterns
5. **Cleanup**: Optionally use `disconnect` when done (connections persist
   until explicitly closed)

### Benefits

- **Concurrent Development**: Work with multiple Neovim instances simultaneously
- **Session Persistence**: Connection IDs remain consistent across MCP server
  restarts
- **Resource Efficiency**: Each connection operates independently without
  blocking others
- **Clear Separation**: Connection-scoped resources eliminate ambiguity about
  which Neovim instance data belongs to
