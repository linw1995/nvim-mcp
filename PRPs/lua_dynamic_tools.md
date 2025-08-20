# Lua Dynamic Tools Implementation PRP

## Goal

Extend the nvim-mcp server to support custom tool registration through Neovim
configuration, allowing users to define specialized MCP tools using Lua
functions. Users will configure custom tools via
`require("nvim-mcp").setup({ custom_tools = {...} })`, which are then
discovered and registered as dynamic tools by the MCP server, enabling
project-specific workflows and automation.

## Why

- **User Extensibility**: Allows users to create project-specific tools
  without modifying the core server
- **Workflow Automation**: Enables custom tools for save operations, buffer
  management, and project-specific tasks
- **Developer Experience**: Provides a simple Lua API for tool creation while
  leveraging the existing dynamic tool infrastructure
- **Connection Isolation**: Each Neovim instance can have its own set of
  custom tools, properly isolated and managed

## What

Implement a system that:

1. **Lua-side tool registry**: Store custom tools configured during `setup()`
   in the Neovim plugin
2. **Tool discovery API**: Allow the MCP server to query registered tools
   from Neovim
3. **Dynamic registration**: Automatically register discovered tools using the
   existing `HybridToolRouter`
4. **Tool execution**: Execute Lua tool handlers via `execute_lua` and convert
   responses to MCP format
5. **Parameter validation**: Add JSON Schema validation for robust parameter
   checking
6. **Connection lifecycle**: Automatically discover and clean up tools with
   connection management

### Success Criteria

- [ ] Users can define custom tools in their Neovim config using
      `setup({ custom_tools = {...} })`
- [ ] MCP server automatically discovers and registers tools on connection
- [ ] Tools execute successfully with proper parameter validation and error handling
- [ ] Tools are properly cleaned up when connections disconnect
- [ ] Full test coverage including integration tests with real Neovim instances
- [ ] Documentation and examples for common tool patterns

## All Needed Context

### Documentation & References

```yaml
# MUST READ - Include these in your context window
- url: https://modelcontextprotocol.io/specification/2025-06-18
  why: MCP protocol specification for tool registration and execution patterns

- url: https://docs.rs/jsonschema
  why: JSON Schema validation patterns and performance best practices

- url: https://neovim.io/doc/user/lua.html
  why: Neovim Lua API reference for plugin development

- file: src/server/hybrid_router.rs
  why: Existing dynamic tool registration infrastructure and patterns

- file: src/server/tools.rs
  why: Static tool implementation patterns using #[tool] macro

- file: src/server/core.rs
  why: Connection management and NeovimMcpServer architecture

- file: lua/nvim-mcp/init.lua
  why: Current plugin structure and setup patterns

- file: src/neovim/client.rs
  why: execute_lua method and Neovim communication patterns

- file: src/server/integration_tests.rs
  why: Testing patterns for MCP server functionality

- doc: https://github.com/nvim-neorocks/nvim-best-practices
  section: Plugin structure and module organization
  critical: Use proper setup() patterns and avoid startup impact
```

### Current Codebase Architecture

**Core Components:**

- `src/server/core.rs`: NeovimMcpServer with HybridToolRouter integration
- `src/server/hybrid_router.rs`: Dynamic tool registration system with
  connection-scoped tools
- `src/server/tools.rs`: Static tools implemented with #[tool] macro
- `lua/nvim-mcp/init.lua`: Simple plugin that starts RPC server
- `src/neovim/client.rs`: NeovimClientTrait with execute_lua capability

**Key Infrastructure:**

- Connection management via
  `Arc<DashMap<String, Box<dyn NeovimClientTrait + Send>>>`
- Dynamic tools stored as:
  `Arc<DashMap<String, DashMap<String, DynamicTool>>>`
- Automatic cleanup on disconnect via `unregister_dynamic_tools`
- JSON Schema support available via schemars crate

### Desired Codebase Changes

**Files to modify:**

```text
lua/nvim-mcp/init.lua - Add tool registry and MCP helper functions
src/server/tools.rs - Add tool discovery functionality
Cargo.toml - Add jsonschema dependency for validation
```

**Files to create:**

```text
src/server/lua_tools.rs - Lua tool discovery and execution logic
src/server/integration_tests/lua_tools_test.rs - Integration tests
docs/lua_tools.md - User documentation and examples
```

### Known Gotchas & Critical Patterns

```rust
// CRITICAL: HybridToolRouter expects DynamicTool struct
pub struct DynamicTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub handler: DynamicToolHandler,
}

// CRITICAL: Tool handlers must be connection-aware
type DynamicToolHandler = Arc<
    dyn Fn(&NeovimMcpServer, serde_json::Value)
        -> BoxFuture<'static, Result<CallToolResult, McpError>>
        + Send + Sync,
>;

// CRITICAL: Tools require connection_id parameter for routing
// Dynamic tools are routed via: arguments.get("connection_id")

// CRITICAL: Lua execute_lua returns serde_json::Value - need conversion
// Result must be converted from Lua MCP response to Rust CallToolResult

// GOTCHA: Tools registered during connection setup, cleaned up on disconnect
// Use server.register_dynamic_tool(&connection_id, tool) pattern

// GOTCHA: JSON Schema validation requires explicit dependency
// Add jsonschema = "0.18" to Cargo.toml

// GOTCHA: Lua vim.json.encode/decode for JSON serialization
// Use vim.json.encode(data) and vim.json.decode(string) in Lua

// GOTCHA: pcall() for safe execution in Lua tool handlers
// Always wrap potentially failing operations:
// local success, result = pcall(handler, params)
```

## Implementation Blueprint

### Data Models and Structures

```rust
// Core structures for Lua tool integration
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LuaToolConfig {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, serde::Deserialize)]
pub struct LuaMcpResponse {
    pub content: Vec<LuaMcpContent>,
    #[serde(rename = "isError")]
    pub is_error: bool,
    #[serde(rename = "_meta")]
    pub meta: Option<LuaMcpMeta>,
}

#[derive(Debug, serde::Deserialize)]
pub struct LuaMcpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct LuaMcpMeta {
    pub error: Option<LuaErrorInfo>,
}
```

### List of Tasks to Complete

```yaml
Task 1: Add jsonschema dependency
MODIFY Cargo.toml:
  - ADD jsonschema = "0.18" to [dependencies]
  - PRESERVE existing dependencies order

Task 2: Extend Lua plugin with tool registry
MODIFY lua/nvim-mcp/init.lua:
  - ADD global tool registry: M._tool_registry = {}
  - ADD MCP helper functions (MCP.success, MCP.error, MCP.text, MCP.json)
  - MODIFY setup() to store custom_tools in registry
  - ADD get_registered_tools() function for discovery
  - ADD execute_tool() function for tool execution
  - PRESERVE existing RPC server setup

Task 3: Create Lua tool integration module
CREATE src/server/lua_tools.rs:
  - MIRROR pattern from: src/server/tools.rs for error handling
  - IMPLEMENT discover_lua_tools() function
  - IMPLEMENT execute_lua_tool() function
  - IMPLEMENT convert_lua_response_to_mcp() function
  - ADD JSON Schema validator integration

Task 4: Add tool discovery to connection process
MODIFY src/server/tools.rs:
  - FIND connect() and connect_tcp() methods
  - INJECT tool discovery after successful connection
  - PRESERVE existing connection setup flow
  - ADD error handling for discovery failures

Task 5: Enhance core server for Lua tools
MODIFY src/server/core.rs:
  - ADD pub use for lua_tools module
  - PRESERVE existing HybridToolRouter integration

Task 6: Create comprehensive integration tests
CREATE src/server/integration_tests/lua_tools_test.rs:
  - MIRROR pattern from: src/server/integration_tests.rs
  - TEST tool discovery, registration, and execution
  - TEST error handling and parameter validation
  - TEST connection lifecycle and cleanup
```

### Task Implementation Details

#### Task 2 Pseudocode: Lua Plugin Extension

```lua
-- lua/nvim-mcp/init.lua
local M = {}
local has_setup = false

-- Global registry to store configured tools
M._tool_registry = {}

-- MCP helper functions for creating responses
M.MCP = {
  success = function(data)
    return {
      content = { { type = "text", text = vim.json.encode(data) } },
      isError = false,
    }
  end,

  error = function(code, message, data)
    return {
      content = { { type = "text", text = message } },
      isError = true,
      _meta = { error = { code = code, message = message, data = data } },
    }
  end
}

-- Enhanced setup function with tool registration
function M.setup(opts)
  opts = opts or {}

  -- Store custom tools in registry with validation
  if opts.custom_tools then
    for tool_name, tool_config in pairs(opts.custom_tools) do
      -- VALIDATION: Ensure required fields exist
      if not tool_config.description or not tool_config.handler then
        vim.notify("Invalid tool config for: " .. tool_name, vim.log.levels.ERROR)
      else
        M._tool_registry[tool_name] = {
          description = tool_config.description,
          parameters = tool_config.parameters or
          { type = "object", properties = {} },
          handler = tool_config.handler,
        }
      end
    end
  end

  -- PRESERVE: Existing RPC server setup
  local pipe_path = generate_pipe_path()
  vim.fn.serverstart(pipe_path)
end

-- Tool Discovery API for MCP Server
function M.get_registered_tools()
  local tools = {}
  for tool_name, tool_config in pairs(M._tool_registry) do
    tools[tool_name] = {
      name = tool_name,
      description = tool_config.description,
      input_schema = tool_config.parameters,
    }
  end
  return tools
end

-- Tool Execution API with error handling
function M.execute_tool(tool_name, params)
  local tool_config = M._tool_registry[tool_name]

  if not tool_config then
    return M.MCP.error("TOOL_NOT_FOUND", "Tool '" .. tool_name .. "' not registered")
  end

  -- SAFE EXECUTION: Use pcall for error handling
  local success, result = pcall(tool_config.handler, params)

  if success then
    return result
  else
    return M.MCP.error("EXECUTION_ERROR", "Tool execution failed: " .. tostring(result))
  end
end
```

#### Task 3 Pseudocode: Lua Tools Integration

```rust
// src/server/lua_tools.rs
use jsonschema::{Validator, ValidationError};
use std::sync::Arc;

// PATTERN: Similar to existing tool parameter validation
pub struct LuaToolValidator {
    validator: Arc<Validator>,
}

impl LuaToolValidator {
    pub fn new(
        schema: &serde_json::Value,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let validator = jsonschema::validator_for(schema)?;
        Ok(Self { validator: Arc::new(validator) })
    }

    pub fn validate(
        &self,
        params: &serde_json::Value,
    ) -> Result<(), ValidationError> {
        self.validator.validate(params)
    }
}

// CORE FUNCTION: Discover tools from Neovim instance
pub async fn discover_lua_tools(
    client: &dyn NeovimClientTrait,
) -> Result<HashMap<String, LuaToolConfig>, NeovimError> {
    debug!("Discovering Lua tools from Neovim instance");

    let lua_code = r#"
        local nvim_mcp = require('nvim-mcp')
        return nvim_mcp.get_registered_tools()
    "#;

    let result = client.execute_lua(lua_code).await?;
    let tools: HashMap<String, LuaToolConfig> = serde_json::from_value(result)?;

    debug!("Discovered {} Lua tools", tools.len());
    Ok(tools)
}

// CORE FUNCTION: Execute Lua tool with proper error handling
pub async fn execute_lua_tool(
    server: &NeovimMcpServer,
    connection_id: &str,
    tool_name: &str,
    arguments: serde_json::Value,
) -> Result<CallToolResult, McpError> {
    let client = server.get_connection(connection_id)?;

    // CRITICAL: Use vim.json.encode for proper serialization
    let lua_code = format!(
        "return require('nvim-mcp').execute_tool('{}', {})",
        tool_name,
        serde_json::to_string(&arguments)?
    );

    let result = client.execute_lua(&lua_code).await?;
    convert_lua_response_to_mcp(result)
}

// CONVERSION: Transform Lua MCP response to Rust CallToolResult
fn convert_lua_response_to_mcp(
    lua_result: serde_json::Value,
) -> Result<CallToolResult, McpError> {
    let lua_response: LuaMcpResponse = serde_json::from_value(lua_result)?;

    if lua_response.is_error {
        if let Some(meta) = lua_response.meta
            && let Some(error) = meta.error
        {
            return Err(McpError::invalid_request(error.message, None));
        }
        return Err(McpError::internal_error("Lua tool execution failed", None));
    }

    // PATTERN: Convert to standard MCP Content format
    let content = lua_response.content
        .into_iter()
        .map(|c| Content::text(c.text))
        .collect();

    Ok(CallToolResult::success(content))
}
```

### Integration Points

```yaml
CONNECTION_LIFECYCLE:
  - location: src/server/tools.rs connect() methods
  - pattern: "After client.setup_diagnostics_changed_autocmd().await?"
  - inject: "discover_and_register_lua_tools(&self, &connection_id, &client).await?"

DYNAMIC_TOOL_REGISTRATION:
  - location: src/server/hybrid_router.rs
  - existing: "register_dynamic_tool() method"
  - usage: "Register each discovered Lua tool with connection_id scope"

JSON_SCHEMA_VALIDATION:
  - location: src/server/lua_tools.rs
  - pattern: "Validate parameters before execution using jsonschema crate"
  - fallback: "Lua-side validation if schema validation fails"

ERROR_CONVERSION:
  - location: "All Lua tool functions"
  - pattern: "Convert NeovimError to McpError using existing From implementation"
```

## Validation Loop

### Level 1: Syntax & Style

```bash
# Run these FIRST - fix any errors before proceeding
cargo build                              # Ensure compilation succeeds
cargo clippy -- -D warnings             # Linting with zero warnings
cargo fmt                                # Code formatting

# Expected: No errors or warnings. If errors, READ carefully and fix.
```

### Level 2: Unit Tests

```rust
// CREATE src/server/lua_tools_test.rs with focused unit tests:
#[tokio::test]
async fn test_lua_tool_discovery() {
    // TEST: discover_lua_tools returns expected tool configs
}

#[tokio::test]
async fn test_lua_tool_execution_success() {
    // TEST: execute_lua_tool with valid parameters returns success
}

#[tokio::test]
async fn test_lua_tool_execution_error() {
    // TEST: execute_lua_tool with invalid tool name returns proper error
}

#[tokio::test]
async fn test_mcp_response_conversion() {
    // TEST: convert_lua_response_to_mcp handles success and error cases
}

#[tokio::test]
async fn test_json_schema_validation() {
    // TEST: LuaToolValidator correctly validates parameters
}
```

```bash
# Run focused unit tests first:
cargo test lua_tools_test -- --show-output
# If failing: Read error messages, understand root cause, fix code, re-run
```

### Level 3: Integration Tests

```rust
// CREATE src/server/integration_tests/lua_tools_integration_test.rs
#[tokio::test]
#[traced_test]
async fn test_end_to_end_lua_tool_workflow() {
    // 1. Start MCP server with child process
    // 2. Connect to Neovim with custom tools configured
    // 3. Verify tools are discovered and registered
    // 4. Execute custom tool via MCP protocol
    // 5. Verify response format and content
    // 6. Test error cases (invalid tool, bad parameters)
    // 7. Test connection cleanup removes tools
}

#[tokio::test]
#[traced_test]
async fn test_multiple_connections_tool_isolation() {
    // TEST: Different connections have independent tool registries
}
```

```bash
# Run comprehensive integration tests:
cargo test integration_tests::lua_tools_integration_test -- --show-output

# Expected: All tests pass with proper Neovim instance lifecycle
# If failing: Check logs for Neovim startup issues or MCP communication problems
```

### Level 4: Manual End-to-End Testing

```bash
# Start the MCP server
cargo run -- --log-level debug

# In separate terminal, test with real Neovim config:
# Create test config with custom tools and verify:
# 1. Tool discovery on connection
# 2. Tool execution via MCP client
# 3. Error handling for invalid inputs
# 4. Cleanup on disconnect
```

## Final Validation Checklist

- [ ] All unit tests pass: `cargo test lua_tools_test -- --show-output`
- [ ] All integration tests pass:
      `cargo test integration_tests -- --show-output`
- [ ] No compilation warnings: `cargo clippy -- -D warnings`
- [ ] Code properly formatted: `cargo fmt`
- [ ] JSON Schema validation works correctly
- [ ] Tool discovery and registration functional
- [ ] Tool execution with proper error handling
- [ ] Connection lifecycle properly manages tool cleanup
- [ ] Documentation updated with examples
- [ ] Manual testing confirms end-to-end workflow

---

## Anti-Patterns to Avoid

- ❌ Don't bypass existing HybridToolRouter infrastructure
- ❌ Don't ignore JSON Schema validation for user input
- ❌ Don't skip connection_id validation in tool routing
- ❌ Don't use direct Lua execution without proper error handling
- ❌ Don't forget to clean up tools on connection disconnect
- ❌ Don't hardcode tool configurations - make them user-configurable
- ❌ Don't break existing static tool functionality

## Confidence Score: 9/10

This PRP provides comprehensive context from the existing codebase, follows
established patterns, includes complete validation loops, and leverages the
robust infrastructure already in place. The implementation builds incrementally
with proper testing at each level, making success in one-pass implementation
highly likely.
