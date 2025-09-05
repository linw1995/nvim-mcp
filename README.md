# Neovim MCP Server

[![codecov](https://codecov.io/gh/linw1995/nvim-mcp/graph/badge.svg?token=OFWOKQQFSD)](https://codecov.io/gh/linw1995/nvim-mcp)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/linw1995/nvim-mcp/CI.yaml)

A Model Context Protocol (MCP) server that provides seamless integration with
Neovim instances, enabling AI assistants to interact with your editor through
connections and access diagnostic information via structured resources.
Supports both stdio and HTTP server transport modes for different integration
scenarios.

## Features

- **Multi-Connection Support**: Manage multiple concurrent Neovim instances
- **LSP Integration**: Complete LSP workflow with code actions, hover, and diagnostics
- **Universal Document Identifier**: Work with files by buffer ID, relative path,
  or absolute path
- **MCP Resources**: Structured diagnostic data via connection-aware URI schemes
- **Multi-Transport Support**: Both stdio and HTTP server transport modes
- **Dynamic Tool System** ⚠️ **(Experimental)**: User-extensible custom tools
- **Plugin Integration**: Automatic setup through Neovim plugin

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

## Demo

<!-- markdownlint-configure-file
{
  "no-inline-html": false
}
-->

### Diagnostic Analysis and Code Fixes

See how the nvim-mcp server helps fix workspace diagnostics
in real-time using AI assistance and LSP integration.
(From [#10](https://github.com/linw1995/nvim-mcp/discussions/10))

<video
  src="https://github.com/user-attachments/assets/6a9b0d84-db28-4896-a843-3798e70c8ba8"
  style="max-height:640px; min-height: 200px">
</video>

### LSP Hover Translation

Seamlessly translate LSP hover information into your native language
without breaking your development workflow.
(From [#85](https://github.com/linw1995/nvim-mcp/discussions/85))

<video
  src="https://github.com/user-attachments/assets/ad8b7e9e-b677-4606-b7c8-5cf6b4f0ab74"
  style="max-height:640px; min-height: 200px">
</video>

### Smart Context Retrieval

Efficiently retrieve related code context for symbols under the cursor
through LSP integration, eliminating the need for manual searching.
(From [#86](https://github.com/linw1995/nvim-mcp/discussions/86))

<video
  src="https://github.com/user-attachments/assets/4c991b37-5bda-43d4-b81b-bea2ae9daaf5"
  style="max-height:640px; min-height: 200px">
</video>

## Quick Start

### 1. Setup Neovim Integration

With a plugin manager like `lazy.nvim`:

```lua
return {
    "linw1995/nvim-mcp",
    build = "cargo install --path .",
    opts = {},
}
```

### 2. Configure `claude` or other MCP clients

```bash
# Auto-connect to current project Neovim instances (recommended)
claude mcp add -s local nvim -- nvim-mcp --log-file . \
  --log-level debug --connect auto

# Analyze diagnostics in current Neovim instance
claude "analyze @nvim:nvim-diagnostics://"
```

## Documentation

For detailed information, see:

- **[Usage Guide](docs/usage.md)**: Detailed usage workflows, CLI options,
  and transport modes
- **[Tools Reference](docs/tools.md)**: Complete reference for all 26 MCP tools
- **[Resources](docs/resources.md)**: MCP resources and URI schemes
- **[Development](docs/development.md)**: Development setup, testing,
  and contributing

## Development

Basic development setup:

```bash
# Enter development shell
nix develop .

# Run tests
cargo test -- --show-output

# Build and run
cargo run -- --connect auto
```

See [Development Guide](docs/development.md) for complete setup instructions,
testing procedures, and contribution guidelines.
