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

## Usage Cases Collection

<!-- markdownlint-configure-file
{
  "no-inline-html": false
}
-->

<video
  src="https://github.com/user-attachments/assets/6946ee1c-42ac-4313-ae10-ca92a4dd0ab7"
  style="max-height:640px; min-height: 200px">
</video>

- LSP Hover Translation (From [#85](https://github.com/linw1995/nvim-mcp/discussions/85))
- Diagnostic Analysis and Code Fixes (From [#10](https://github.com/linw1995/nvim-mcp/discussions/10))
- Smart Context Retrieval (From [#86](https://github.com/linw1995/nvim-mcp/discussions/86))
- And more in [Discussions](https://github.com/linw1995/nvim-mcp/discussions)

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
