use std::collections::HashMap;
use std::sync::Arc;

use futures::future::BoxFuture;
use rmcp::{
    ErrorData as McpError,
    model::{CallToolResult, Content},
};
use tracing::{debug, instrument};

use super::core::NeovimMcpServer;
use super::hybrid_router::DynamicTool;
use crate::neovim::{NeovimClientTrait, NeovimError};

// Type alias for the dynamic tool handler function
type LuaToolHandler = Arc<
    dyn Fn(
            &NeovimMcpServer,
            serde_json::Value,
        ) -> BoxFuture<'static, Result<CallToolResult, McpError>>
        + Send
        + Sync,
>;

// Core structures for Lua tool integration
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LuaToolConfig {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
pub struct LuaMcpResponse {
    pub content: Vec<LuaMcpContent>,
    #[serde(rename = "isError")]
    pub is_error: bool,
    #[serde(rename = "_meta")]
    pub meta: Option<LuaMcpMeta>,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
pub struct LuaMcpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
pub struct LuaMcpMeta {
    pub error: Option<LuaErrorInfo>,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
pub struct LuaErrorInfo {
    pub code: String,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

// PATTERN: Simple validation using jsonschema crate
#[allow(dead_code)]
pub struct LuaToolValidator {
    schema: serde_json::Value,
}

#[allow(dead_code)]
impl LuaToolValidator {
    pub fn new(schema: &serde_json::Value) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            schema: schema.clone(),
        })
    }

    pub fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        match jsonschema::is_valid(&self.schema, params) {
            true => Ok(()),
            false => Err("Validation failed".to_string()),
        }
    }

    pub fn is_valid(&self, params: &serde_json::Value) -> bool {
        jsonschema::is_valid(&self.schema, params)
    }
}

// CORE FUNCTION: Discover tools from Neovim instance
#[instrument(skip(client))]
pub async fn discover_lua_tools(
    client: &dyn NeovimClientTrait,
) -> Result<HashMap<String, LuaToolConfig>, NeovimError> {
    debug!("Discovering Lua tools from Neovim instance");

    let lua_code = r#"
        local nvim_mcp = require('nvim-mcp')
        return nvim_mcp.get_registered_tools()
    "#;

    let result = client.execute_lua(lua_code).await?;

    // Convert nvim_rs::Value to serde_json::Value
    let json_result = convert_nvim_value_to_json(result)?;
    let tools: HashMap<String, LuaToolConfig> = serde_json::from_value(json_result)
        .map_err(|e| NeovimError::Api(format!("Failed to parse tool configs: {}", e)))?;

    debug!("Discovered {} Lua tools", tools.len());
    Ok(tools)
}

// Helper function to convert nvim_rs::Value to serde_json::Value
fn convert_nvim_value_to_json(nvim_value: rmpv::Value) -> Result<serde_json::Value, NeovimError> {
    match nvim_value {
        rmpv::Value::Nil => Ok(serde_json::Value::Null),
        rmpv::Value::Boolean(b) => Ok(serde_json::Value::Bool(b)),
        rmpv::Value::Integer(i) => {
            if let Some(num) = i.as_i64() {
                Ok(serde_json::Value::Number(serde_json::Number::from(num)))
            } else if let Some(num) = i.as_u64() {
                Ok(serde_json::Value::Number(serde_json::Number::from(num)))
            } else {
                Err(NeovimError::Api("Integer value out of range".to_string()))
            }
        }
        rmpv::Value::F32(f) => {
            if let Some(num) = serde_json::Number::from_f64(f as f64) {
                Ok(serde_json::Value::Number(num))
            } else {
                Err(NeovimError::Api("Invalid float value".to_string()))
            }
        }
        rmpv::Value::F64(f) => {
            if let Some(num) = serde_json::Number::from_f64(f) {
                Ok(serde_json::Value::Number(num))
            } else {
                Err(NeovimError::Api("Invalid float value".to_string()))
            }
        }
        rmpv::Value::String(s) => {
            let utf8_str = s
                .into_str()
                .ok_or_else(|| NeovimError::Api("Invalid UTF-8 string".to_string()))?;
            Ok(serde_json::Value::String(utf8_str))
        }
        rmpv::Value::Binary(_) => Err(NeovimError::Api("Binary values not supported".to_string())),
        rmpv::Value::Array(arr) => {
            let mut json_arr = Vec::new();
            for item in arr {
                json_arr.push(convert_nvim_value_to_json(item)?);
            }
            Ok(serde_json::Value::Array(json_arr))
        }
        rmpv::Value::Map(map) => {
            let mut json_obj = serde_json::Map::new();
            for (key, value) in map {
                let key_str = match key {
                    rmpv::Value::String(s) => s
                        .into_str()
                        .ok_or_else(|| NeovimError::Api("Invalid UTF-8 key".to_string()))?,
                    _ => return Err(NeovimError::Api("Map keys must be strings".to_string())),
                };
                json_obj.insert(key_str, convert_nvim_value_to_json(value)?);
            }
            Ok(serde_json::Value::Object(json_obj))
        }
        rmpv::Value::Ext(_, _) => Err(NeovimError::Api(
            "Extension values not supported".to_string(),
        )),
    }
}

// CORE FUNCTION: Execute Lua tool with proper error handling
#[allow(dead_code)]
#[instrument(skip(server, arguments))]
pub async fn execute_lua_tool(
    server: &NeovimMcpServer,
    connection_id: &str,
    tool_name: &str,
    arguments: serde_json::Value,
) -> Result<CallToolResult, McpError> {
    let client = server.get_connection(connection_id)?;

    // CRITICAL: Use vim.json.encode for proper serialization
    let args_json = serde_json::to_string(&arguments).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize arguments: {}", e), None)
    })?;

    let lua_code = format!(
        "return require('nvim-mcp').execute_tool('{}', {})",
        tool_name, args_json
    );

    let result = client.execute_lua(&lua_code).await?;
    let json_result = convert_nvim_value_to_json(result)?;
    convert_lua_response_to_mcp(json_result)
}

// CONVERSION: Transform Lua MCP response to Rust CallToolResult
#[allow(dead_code)]
fn convert_lua_response_to_mcp(lua_result: serde_json::Value) -> Result<CallToolResult, McpError> {
    let lua_response: LuaMcpResponse = serde_json::from_value(lua_result).map_err(|e| {
        McpError::internal_error(format!("Failed to parse Lua response: {}", e), None)
    })?;

    if lua_response.is_error {
        if let Some(meta) = lua_response.meta
            && let Some(error) = meta.error
        {
            return Err(McpError::invalid_request(error.message, None));
        }
        return Err(McpError::internal_error("Lua tool execution failed", None));
    }

    // PATTERN: Convert to standard MCP Content format
    let content = lua_response
        .content
        .into_iter()
        .map(|c| Content::text(c.text))
        .collect();

    Ok(CallToolResult::success(content))
}

// Helper function to create a handler for a Lua tool
fn create_lua_tool_handler(tool_name: String, connection_id: String) -> LuaToolHandler {
    Arc::new(move |_server, _arguments| {
        let tool_name = tool_name.clone();
        let connection_id = connection_id.clone();

        // For now, return an error indicating this is not yet implemented
        Box::pin(async move {
            Err(McpError::internal_error(
                format!(
                    "Lua tool '{}' for connection '{}' not yet fully implemented",
                    tool_name, connection_id
                ),
                None,
            ))
        })
    })
}

// Helper function to register discovered Lua tools as dynamic tools
#[instrument(skip(server, client))]
pub async fn discover_and_register_lua_tools(
    server: &NeovimMcpServer,
    connection_id: &str,
    client: &dyn NeovimClientTrait,
) -> Result<(), McpError> {
    debug!(
        "Discovering and registering Lua tools for connection: {}",
        connection_id
    );

    let discovered_tools = discover_lua_tools(client).await?;

    for (tool_name, tool_config) in discovered_tools {
        let tool_name_for_handler = tool_name.clone();
        let connection_id_for_handler = connection_id.to_string();

        let dynamic_tool = DynamicTool {
            name: tool_name.clone(),
            description: tool_config.description.clone(),
            input_schema: tool_config.input_schema,
            handler: create_lua_tool_handler(tool_name_for_handler, connection_id_for_handler),
        };

        server.register_dynamic_tool(connection_id, dynamic_tool)?;
        debug!(
            "Registered Lua tool '{}' for connection '{}'",
            tool_name, connection_id
        );
    }

    debug!(
        "Completed Lua tool registration for connection: {}",
        connection_id
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_lua_tool_validator_creation() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "required": ["name"]
        });

        let validator = LuaToolValidator::new(&schema);
        assert!(validator.is_ok());
    }

    #[test]
    fn test_lua_tool_validator_validation() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "required": ["name"]
        });

        let validator = LuaToolValidator::new(&schema).unwrap();

        // Valid parameters
        let valid_params = json!({"name": "test"});
        assert!(validator.is_valid(&valid_params));
        assert!(validator.validate(&valid_params).is_ok());

        // Invalid parameters
        let invalid_params = json!({"age": 25});
        assert!(!validator.is_valid(&invalid_params));
        assert!(validator.validate(&invalid_params).is_err());
    }

    #[test]
    fn test_convert_lua_response_to_mcp_success() {
        let lua_response = json!({
            "content": [
                {"type": "text", "text": "success message"}
            ],
            "isError": false
        });

        let result = convert_lua_response_to_mcp(lua_response);
        assert!(result.is_ok());

        let mcp_result = result.unwrap();
        assert_eq!(mcp_result.content.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_convert_lua_response_to_mcp_error() {
        let lua_response = json!({
            "content": [
                {"type": "text", "text": "error message"}
            ],
            "isError": true,
            "_meta": {
                "error": {
                    "code": "TEST_ERROR",
                    "message": "Test error message"
                }
            }
        });

        let result = convert_lua_response_to_mcp(lua_response);
        assert!(result.is_err());
    }
}
