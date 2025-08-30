# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

## Project Overview

This is a Rust-based MCP (Model Context Protocol) server that provides
seamless integration with Neovim instances. The server acts as a bridge between
AI assistants and Neovim editors, enabling operations like LSP integration,
diagnostic analysis, and file manipulation through structured MCP tools and
resources.

## Development Commands

More details in @./docs/development.md .

### Building and Running

```bash
# Development build and run
cargo build
cargo run -- --connect auto

# Connect to specific targets
cargo run -- --connect 127.0.0.1:6666
cargo run -- --connect /tmp/nvim.sock

# HTTP server mode with auto-connection
cargo run -- --http-port 8080 --connect auto

# Production build
cargo build --release
```

### Testing

```bash
# Run all tests with output
./scripts/run-test.sh -- --show-output

# Run specific test modules
./scripts/run-test.sh -- --show-output neovim::integration_tests

# Skip integration tests (which require Neovim)
./scripts/run-test.sh -- --skip=integration_tests --show-output

# Run tests with coverage
./scripts/run-cov.sh -- --show-output
```

### Linting and Formatting

```bash
pre-commit run --all-files
```

## Architecture Overview

### Core Components

- **MCP Server Core** (`src/server/core.rs`): Main server implementation with
  connection management using DashMap for thread-safe concurrent access
- **Neovim Client** (`src/neovim/client.rs`): Handles communication with
  Neovim instances via nvim-rs msgpack-rpc
- **Tool System** (`src/server/tools.rs`): 26+ MCP tools for LSP operations,
  file navigation, and diagnostics
- **Resource System** (`src/server/resources.rs`): Connection-scoped
  diagnostic resources with URI schemes
- **Transport Layer**: Supports both stdio and HTTP server transports via rmcp

## Testing Architecture

- **Integration Tests**: Full MCP client-server communication tests in
  `src/server/integration_tests.rs` and `src/neovim/integration_tests.rs`
- **LSP Testing**: Comprehensive Go/gopls integration with test data in `src/testdata/`
- **Global Test Mutex**: Prevents port conflicts during concurrent execution
- **Automated Setup**: Tests spawn and manage Neovim instances automatically
- **Code Coverage**: LLVM-based coverage using grcov with
  HTML/Cobertura/Markdown reports

## Key Dependencies

Read @./Cargo.toml

## Development Environment

Uses Nix flakes for reproducible development environments. Read @./flake.nix

Enter development shell: `nix develop .` (if not already in Nix shell)

## Adding New Tools

When adding connection-aware MCP tools:

1. Add parameter struct in `src/server/tools.rs` with `connection_id: String`
2. Implement tool method with `#[tool(description = "...")]` attribute
3. Use `self.get_connection(&connection_id)?` for connection validation
4. Return `Result<CallToolResult, McpError>`
5. Update integration tests
6. Tool is automatically registered via `#[tool_router]` macro

## Error Handling

- **Layered Errors**: `ServerError` (top-level) and `NeovimError` (Neovim-specific)
- **MCP Compliance**: Errors properly formatted for MCP protocol responses
- **Comprehensive Propagation**: I/O and nvim-rs errors properly converted and handled
