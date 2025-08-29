# Development Guide

This project uses Nix flakes for reproducible development environments.

## Setup

```bash
# Enter development shell
nix develop .

# Auto-activate with direnv (optional)
echo 'use flake' >.envrc
```

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
./scripts/run-test.sh -- --show-output

# Run single specific module test
./scripts/run-test.sh -- --show-output neovim::integration_tests

# Run single specific test
./scripts/run-test.sh -- --show-output neovim::integration_tests::test_tcp_connection_lifecycle

# Skip integration tests (which require Neovim)
./scripts/run-test.sh -- --skip=integration_tests --show-output

# Run tests with coverage using grcov
nix run .#cov -- --show-output

# Run specific tests with coverage
nix run .#cov -- --show-output neovim::integration_tests

# Run tests in Nix environment (requires IN_NIX_SHELL not set)
nix develop . --command ./scripts/run-test.sh -- --show-output

# Alternative: Use nix test app
nix run .#test -- --show-output
```

**Note**: The `nix develop . --command` syntax only works when the
`IN_NIX_SHELL` environment variable is not set. If you're already in a Nix
shell, use the commands directly without the `nix develop . --command` prefix.

## Plugin Development

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

## Development Environment

This project uses Nix flakes for reproducible development environments.
The flake provides:

- Rust toolchain (stable) with clippy, rustfmt, rust-analyzer, and LLVM tools
- grcov for code coverage analysis
- Neovim 0.11.3+ for integration testing
- Pre-commit hooks for code quality

Use `nix develop .` to enter the development shell (only if `IN_NIX_SHELL` is
not already set) or set up direnv with `echo 'use flake' > .envrc` for
automatic environment activation.

### Code Formatting

The project uses `stylua.toml` for Lua code formatting. Rust code follows
standard rustfmt conventions.

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
- **Enhanced reliability**: Robust LSP synchronization with notification tracking
- **Optimized timing**: Better test performance with improved setup and teardown
- **Notification testing**: Unit tests for notification tracking system
- **Code coverage**: LLVM-based code coverage using grcov with HTML, Cobertura,
  and Markdown report generation

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
- **`grcov`**: Code coverage reporting tool using LLVM instrumentation
- **Enhanced deserialization**: Support for both string and struct formats
  in CodeAction and WorkspaceEdit types
- **Lua tool testing** ⚠️ **(Experimental)**: Integration tests for custom tool registration
  and execution
