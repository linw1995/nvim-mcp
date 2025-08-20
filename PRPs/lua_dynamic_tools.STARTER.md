# Lua Dynamic Tools Blueprint

## Feature

Extend the nvim-mcp Lua plugin to support custom tool registration through
Neovim configuration. This allows users to define specialized MCP tools using
Lua functions, extending the server's built-in capabilities for project-specific
workflows and automation.

## Examples

### Basic Lua Plugin Setup with Custom Tools

```lua
-- In your Neovim configuration (init.lua or plugin config)
require("nvim-mcp").setup({
    custom_tools = {
        -- Save a single buffer by ID
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
                local buf_id = params.buffer_id

                -- Validate buffer
                if not vim.api.nvim_buf_is_valid(buf_id) then
                    return MCP.error("INVALID_PARAMS",
                        "Buffer " .. buf_id .. " is not valid")
                end

                local buf_name = vim.api.nvim_buf_get_name(buf_id)
                if buf_name == "" then
                    return MCP.error("INVALID_PARAMS",
                        "Buffer " .. buf_id .. " has no associated file")
                end

                -- Save the buffer
                local success, err = pcall(function()
                    vim.api.nvim_buf_call(buf_id, function()
                        vim.cmd("write")
                    end)
                end)

                if success then
                    return MCP.success({
                        buffer_id = buf_id,
                        filename = buf_name,
                        message = "Buffer saved successfully",
                    })
                else
                    return MCP.error("INTERNAL_ERROR",
                        "Failed to save buffer: " .. tostring(err))
                end
            end,
        },

        -- Save all modified buffers
        save_all = {
            description = "Save all modified buffers",
            parameters = {
                type = "object",
                properties = {},
            },
            handler = function(params)
                local results = {}
                local saved_count = 0
                local failed_count = 0

                -- Get all buffers
                local buffers = vim.api.nvim_list_bufs()

                for _, buf_id in ipairs(buffers) do
                    -- Only process valid, loaded, modified buffers with files
                    if
                        vim.api.nvim_buf_is_valid(buf_id)
                        and vim.api.nvim_buf_is_loaded(buf_id)
                        and vim.api.nvim_buf_get_option(buf_id, "modified")
                    then
                        local buf_name = vim.api.nvim_buf_get_name(buf_id)

                        if buf_name ~= "" then
                            local success, err = pcall(function()
                                vim.api.nvim_buf_call(buf_id, function()
                                    vim.cmd("write")
                                end)
                            end)

                            local result = {
                                buffer_id = buf_id,
                                filename = buf_name,
                                success = success,
                            }

                            if success then
                                result.message = "Saved successfully"
                                saved_count = saved_count + 1
                            else
                                result.error = tostring(err)
                                failed_count = failed_count + 1
                            end

                            table.insert(results, result)
                        end
                    end
                end

                return MCP.success({
                    saved_count = saved_count,
                    failed_count = failed_count,
                    total_processed = #results,
                    results = results,
                    message = string.format("Saved %d buffers, %d failed",
                        saved_count, failed_count),
                })
            end,
        },
    },
})
```

### MCP Helper Functions

The plugin provides helper functions that mirror rmcp's `CallToolResult` structure
for compatibility:

```lua
-- MCP helper functions for creating responses
local MCP = {}

-- Create successful tool response
function MCP.success(data)
    return {
        content = {
            {
                type = "text",
                text = vim.json.encode(data),
            },
        },
        isError = false,
    }
end

-- Create error response
function MCP.error(code, message, data)
    return {
        content = {
            {
                type = "text",
                text = message,
            },
        },
        isError = true,
        _meta = {
            error = {
                code = code,
                message = message,
                data = data,
            },
        },
    }
end

-- Create text response
function MCP.text(text)
    return {
        content = {
            {
                type = "text",
                text = text,
            },
        },
        isError = false,
    }
end

-- Create JSON response
function MCP.json(data)
    return {
        content = {
            {
                type = "text",
                text = vim.json.encode(data),
            },
        },
        isError = false,
    }
end
```

## Documentation

- [Neovim Lua API Reference](https://neovim.io/doc/user/lua.html) -
  Complete Lua API documentation for Neovim
- [MCP Protocol Specification](https://spec.modelcontextprotocol.io/) - Official
  Model Context Protocol documentation
- [JSON Schema Reference](https://json-schema.org/) - For parameter validation
  schemas

## Implementation Details

### Lua-Side Tool Registration Store

The Neovim plugin maintains a global tool registry that stores custom tools
configured during `setup()`:

```lua
-- In lua/nvim-mcp/init.lua
local M = {}

-- Global registry to store configured tools
M._tool_registry = {}

-- Enhanced setup function with tool registration
function M.setup(opts)
    opts = opts or {}

    -- Store custom tools in registry
    if opts.custom_tools then
        for tool_name, tool_config in pairs(opts.custom_tools) do
            M._tool_registry[tool_name] = {
                description = tool_config.description,
                parameters = tool_config.parameters,
                handler = tool_config.handler,
            }
        end
    end

    -- Start RPC server (existing functionality)
    -- ... socket setup code ...
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

-- Tool Execution API for MCP Server
function M.execute_tool(tool_name, params)
    local tool_config = M._tool_registry[tool_name]

    if not tool_config then
        return M.MCP.error("TOOL_NOT_FOUND", "Tool '" .. tool_name .. "' not registered")
    end

    -- Execute the tool handler with parameters
    local success, result = pcall(tool_config.handler, params)

    if success then
        return result
    else
        return M.MCP.error("EXECUTION_ERROR", "Tool execution failed: " .. tostring(result))
    end
end
```

### Registration Flow

#### 1. User Configuration Phase

When users call `require("nvim-mcp").setup()`, custom tools are stored in
the Lua-side registry:

```lua
-- User's init.lua
require("nvim-mcp").setup({
    custom_tools = {
        save_buffer = {
            description = "Save a specific buffer by ID",
            parameters = { /* JSON schema */ },
            handler = function(params)
                -- Tool implementation using MCP helpers
                return MCP.success({...})
            end,
        },
    },
})
```

#### 2. MCP Server Discovery Phase

When a connection is established, the MCP server discovers available tools:

```rust
// After successful connection in Rust
let tools_lua = r#"
    return require('nvim-mcp').get_registered_tools()
"#;

let tools_result = client.execute_lua(tools_lua).await?;
let tool_configs: HashMap<String, ToolConfig> = serde_json::from_value(tools_result)?;

// Register each discovered tool as a dynamic tool
for (tool_name, config) in tool_configs {
    let dynamic_tool = DynamicTool {
        name: tool_name.clone(),
        description: config.description,
        input_schema: config.input_schema,
        handler: Arc::new(move |server, arguments| {
            let tool_name = tool_name.clone();
            Box::pin(async move {
                execute_lua_tool(server, &connection_id, &tool_name, arguments).await
            })
        }),
    };

    server.register_dynamic_tool(&connection_id, dynamic_tool)?;
}
```

#### 3. Tool Execution Phase

When a dynamic tool is called, the MCP server executes the corresponding Lua function:

```rust
async fn execute_lua_tool(
    server: &NeovimMcpServer,
    connection_id: &str,
    tool_name: &str,
    arguments: serde_json::Value,
) -> Result<CallToolResult, McpError> {
    let client = server.get_connection(connection_id)?;

    let lua_code = format!(
        "return require('nvim-mcp').execute_tool('{}', {})",
        tool_name,
        serde_json::to_string(&arguments)?
    );

    let result = client.execute_lua(&lua_code).await?;

    // Convert Lua MCP response to Rust CallToolResult
    convert_lua_response_to_mcp(result)
}
```

### Rust-Side Parameter Validation

The MCP server implements JSON Schema validation using the `jsonschema` crate
for robust parameter checking:

```rust
use jsonschema::{Validator, ValidationError};
use serde_json::Value;
use std::sync::Arc;

pub struct DynamicToolValidator {
    validator: Arc<Validator>,
}

impl DynamicToolValidator {
    pub fn new(schema: &Value) -> Result<Self, Box<dyn std::error::Error>> {
        let validator = jsonschema::validator_for(schema)?;
        Ok(Self {
            validator: Arc::new(validator),
        })
    }

    pub fn validate(&self, params: &Value) -> Result<(), ValidationError> {
        self.validator.validate(params)
    }

    pub fn is_valid(&self, params: &Value) -> bool {
        self.validator.is_valid(params)
    }
}
```

### Dynamic Tool Registration

Custom tools are registered using the existing `HybridToolRouter`
infrastructure with runtime discovery:

```rust
pub struct DynamicTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub handler: DynamicToolHandler,
}

// Handler type for async execution
type DynamicToolHandler = Arc<
    dyn Fn(
            &NeovimMcpServer,
            serde_json::Value,
        ) -> BoxFuture<'static, Result<CallToolResult, McpError>>
        + Send
        + Sync,
>;

impl DynamicTool {
    pub fn from_lua_discovery(
        name: String,
        config: &LuaToolConfig,
        connection_id: String,
    ) -> Self {
        let tool_name_for_handler = name.clone();
        let connection_id_for_handler = connection_id.clone();

        Self {
            name,
            description: config.description.clone(),
            input_schema: config.input_schema.clone(),
            handler: Arc::new(move |server, arguments| {
                let tool_name = tool_name_for_handler.clone();
                let connection_id = connection_id_for_handler.clone();
                Box::pin(async move {
                    execute_lua_tool(server, &connection_id, &tool_name, arguments).await
                })
            }),
        }
    }
}
```

### Tool Execution Flow

1. **Tool Discovery**: MCP server queries Neovim for registered tools via
   `get_registered_tools()`
2. **Dynamic Registration**: Tools are registered in `HybridToolRouter` with
   connection-specific handlers
3. **Parameter Validation**: JSON Schema validation using `jsonschema` crate
   (future enhancement)
4. **Lua Execution**: Execute tool handler function in Neovim Lua context via `execute_tool()`
5. **Response Processing**: Convert Lua MCP response structure to Rust `CallToolResult`
6. **Error Handling**: Structured error reporting with Lua execution details

### Connection-Scoped Tool Management

Tools are automatically discovered, registered, and cleaned up with connection lifecycle:

```rust
// Tool discovery and registration on connection
let discovered_tools = discover_lua_tools(&client).await?;
for tool in discovered_tools {
    server.register_dynamic_tool(&connection_id, tool)?;
}

// Tools automatically cleaned up on disconnect
// Calls unregister_dynamic_tools internally
server.disconnect(&connection_id)?;
```

### Connection Isolation Benefits

- **Per-connection registries**: Each Neovim instance maintains its own tool registry
- **Independent configurations**: Different projects can have different custom tools
- **Clean separation**: Tools don't interfere across connections
- **Automatic cleanup**: Tool registry is cleared when connection closes
- **Runtime discovery**: Tools are discovered when connection is established,
  not at compile time

## Other Considerations

### Tool Design Principles

- **Parameter Validation**: JSON Schema enforced on Rust side for security
- **Error Handling**: Return structured errors using MCP helper functions
- **Response Compatibility**: Use MCP helpers to ensure rmcp compatibility
- **Namespace Isolation**: Custom tools cannot override built-in MCP server tools
- **Resource Management**: Tools execute within Neovim instance context
- **Connection Isolation**: Tools are scoped to specific connections

### Security and Safety

- Custom tools run with full Neovim API access - design carefully
- **Dual Validation**: Parameters validated both in Rust (JSON Schema) and Lua
- Use `pcall()` for safe execution of potentially failing operations
- Avoid exposing sensitive file system operations without proper validation
- Tool execution sandboxed within connection context

### Performance Considerations

- **Validator Caching**: JSON Schema validators compiled once and reused
- **Thread Safety**: Validators shared across threads using `Arc`
- Keep tool handlers lightweight and responsive
- Use async patterns where appropriate for long-running operations
- Consider caching results for expensive computations
- Implement proper cleanup for tools that manage resources

### Integration Patterns

- Tools can integrate with LSP clients for advanced code analysis
- Leverage existing Neovim plugins and ecosystem
- Use structured logging for debugging custom tool behavior
- Design tools to be composable and reusable across projects
- **Dynamic Registration**: Tools can be added/removed without server restart
