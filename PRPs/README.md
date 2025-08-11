# Project Requirements & Proposals (PRPs)

## Development Timeline

The nvim-mcp project evolved through four major phases, each documented as a PRP:

### Phase 1: Foundation (July 2024)

**[nvim-api-integration](./nvim-api-integration.md)** |
**[STARTER](./nvim-api-integration.STARTER.md)** _(July 26, 2024)_

- **Goal**: TCP client for Neovim API with basic MCP tools
- **Key Features**:
  - Single TCP connection to Neovim instances
  - Buffer operations (`list_buffers`)
  - Lua code execution (`exec_lua`)
  - Connection lifecycle management
- **Status**: ✅ **Implemented** - Forms the foundation of current architecture

### Phase 2: MCP Protocol Foundation (July 2024)

**[mcp-server-implementation](./mcp-server-implementation.md)** |
**[STARTER](./mcp-server-implementation.STARTER.md)** _(July 30, 2024)_

- **Goal**: Robust MCP server using rmcp crate with stdio transport
- **Key Features**:
  - MCP protocol compliance via stdio
  - Tool routing with `#[tool_router]` macro
  - Counter tool demonstration
  - Structured logging and error handling
- **Status**: ✅ **Implemented** - Core server infrastructure in place
- **Dependencies**: Builds on basic Rust project structure

### Phase 3: Diagnostics Integration (August 2024)

**[nvim-diagnostics-resources](./nvim-diagnostics-resources.md)** |
**[STARTER](./nvim-diagnostics-resources.STARTER.md)** _(August 5, 2024)_

- **Goal**: MCP Resources capability for diagnostic data access
- **Key Features**:
  - `nvim-diagnostics://` URI scheme
  - Workspace and buffer-scoped diagnostic resources
  - LSP integration for comprehensive diagnostic data
  - Structured JSON diagnostic information
- **Status**: ✅ **Implemented** - Available as MCP resources
- **Dependencies**: Requires Phase 1 (Neovim integration) and Phase 2 (MCP server)

### Phase 4: Multi-Connection Architecture (August 2024)

**[multi-connections](./multi-connections.md)** |
**[STARTER](./multi-connections.STARTER.md)** _(August 7, 2024)_

- **Goal**: Concurrent management of multiple Neovim instances
- **Key Features**:
  - DashMap-based connection storage for performance
  - Deterministic connection IDs using BLAKE3 hashing
  - Connection-scoped resources and operations
  - Independent session isolation
- **Status**: ✅ **Implemented** - Multi-connection architecture in place
- **Dependencies**: Requires all previous phases for comprehensive refactoring

### Phase 5: Universal Document Identification (August 2025)

- **[universal-text-document-identifier](./universal-text-document-identifier.md)**
- **[STARTER](./universal-text-document-identifier.STARTER.md)** _(August 11, 2025)_
- **Goal**: Universal text document identifier system for enhanced LSP operations
- **Key Features**:
  - Support for buffer IDs, project-relative paths, and absolute paths
  - Enhanced LSP operations on files not open in Neovim buffers
  - Backward compatibility with existing buffer-based methods
  - Universal MCP tools with `DocumentIdentifier` support
- **Status**: ✅ **Implemented** - Current production architecture
- **Dependencies**: Requires Phase 4 (multi-connection architecture) for proper operation
