# Neovim MCP Server

A Model Context Protocol (MCP) server that provides seamless integration with
Neovim instances, enabling AI assistants to interact with your editor through
connections and access diagnostic information via structured resources.

## Features

- **Multi-Connection Support**: Manage multiple concurrent Neovim instances with
  deterministic connection IDs
- **Connection Management**: Connect via TCP or Unix socket/named pipe with
  automatic discovery
- **Buffer Operations**: List and inspect all open buffers with detailed information
- **Diagnostics Access**: Retrieve diagnostics for buffers with error/warning details
- **LSP Integration**: Access code actions and LSP client information
- **MCP Resources**: Structured diagnostic data via connection-aware URI schemes
- **Lua Execution**: Execute arbitrary Lua code directly in Neovim
- **Plugin Integration**: Automatic setup through Neovim plugin
- **Modular Architecture**: Clean separation between core infrastructure,
  MCP tools, and resource handlers

## Installation

### Use Cargo install from crates.io

```bash
cargo install nvim-mcp
```

### Using Nix

```bash
nix profile install github:linw1995/nvim-mcp#nvim-mcp
```

### From Source

```bash
git clone https://github.com/linw1995/nvim-mcp.git && cd nvim-mcp
cargo install --path .
```

## Quick Start

### 1. Start the Server

```bash
# Start as stdio MCP server (default)
nvim-mcp
# With custom logging
nvim-mcp --log-file ./nvim-mcp.log --log-level debug
```

#### Command Line Options

- `--log-file <PATH>`: Path to log file (defaults to stderr)
- `--log-level <LEVEL>`: Log level (trace, debug, info, warn, error;
  defaults to info)

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

### 3. Basic Usage Workflow

Once both the MCP server and Neovim are running, here's a typical workflow:

#### Using Unix Socket (Recommended)

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

#### Using TCP Connection

1. **Connect to TCP endpoint**:
   - Use `connect_tcp` tool with address like "127.0.0.1:6666"
   - Save the returned `connection_id`
2. **Follow steps 3-4 above** with your connection ID

## Available Tools

The server provides these MCP tools for interacting with Neovim:

### Connection Management

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

### Connection-Aware Tools

All tools below require a `connection_id` parameter from the connection
establishment phase:

#### Buffer Operations

- **`list_buffers`**: List all open buffers with names and line counts
  - Parameters: `connection_id` (string) - Target Neovim connection

- **`buffer_diagnostics`**: Get diagnostics for a specific buffer
  - Parameters: `connection_id` (string), `id` (number) - Buffer ID

#### LSP Integration

- **`lsp_clients`**: Get workspace LSP clients
  - Parameters: `connection_id` (string) - Target Neovim connection

- **`buffer_code_actions`**: Get available code actions for buffer range
  - Parameters: `connection_id` (string), `id` (number), `lsp_client_name`
    (string), `line` (number), `character` (number), `end_line` (number),
    `end_character` (number) (all positions are 0-indexed)

- **`buffer_hover`**: Get symbol hover information via LSP
  - Parameters: `connection_id` (string), `id` (number), `lsp_client_name`
    (string), `line` (number), `character` (number) (all positions are 0-indexed)

#### Code Execution

- **`exec_lua`**: Execute Lua code in Neovim
  - Parameters: `connection_id` (string), `code` (string) - Lua code to execute

## MCP Resources

Access diagnostic and connection information through structured URI schemes:

### Available Resources

#### Connection Monitoring

- **`nvim-connections://`**: List all active Neovim connections
  - Returns array of connection objects with `id` and `target` information
  - Useful for monitoring multiple concurrent Neovim instances

#### Connection-Scoped Diagnostics

Diagnostic resources use connection-specific URIs via the
`nvim-diagnostics://` scheme:

- **`nvim-diagnostics://{connection_id}/workspace`**: All diagnostic messages
  across workspace for specific connection
- **`nvim-diagnostics://{connection_id}/buffer/{buffer_id}`**: Diagnostics for
  specific buffer on specific connection

### Usage Examples

#### List Active Connections

```json
{
  "method": "resources/read",
  "params": {
    "uri": "nvim-connections://"
  }
}
```

#### Get Connection-Specific Workspace Diagnostics

```json
{
  "method": "resources/read",
  "params": {
    "uri": "nvim-diagnostics://abc123def456/workspace"
  }
}
```

#### Get Buffer Diagnostics for Specific Connection

```json
{
  "method": "resources/read",
  "params": {
    "uri": "nvim-diagnostics://abc123def456/buffer/1"
  }
}
```

All diagnostic resources return structured JSON with diagnostic information
including severity levels, messages, file paths, and line/column positions.
Connection IDs are deterministic BLAKE3 hashes of the target string for
consistent identification across sessions.

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

## Development

This project uses Nix flakes for reproducible development environments.

### Setup

```bash
# Enter development shell
nix develop .

# Auto-activate with direnv (optional)
echo 'use flake' >.envrc
```

### Testing

```bash
# Run all tests
cargo test -- --show-output

# Skip integration tests (which require Neovim)
cargo test -- --skip=integration_tests --show-output

# In Nix environment
nix develop . --command cargo test -- --show-output
```

**Note**: If already in a Nix shell, omit the `nix develop . --command` prefix.

### Building and Running

```bash
# Build debug version
cargo build

# Build and run debug version
cargo run

# Build and run release version
cargo run --release

# Build and run with custom logging
cargo run -- --log-file ./debug.log --log-level debug

# Using Nix
nix run .
```

### Plugin Development

For local development with `lazy.nvim`, create `.lazy.lua` in the project root:

```lua
return {
    {
        "linw1995/nvim-mcp",
        dir = ".",
        opts = {},
    },
}
```
