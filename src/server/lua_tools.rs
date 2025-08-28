use std::collections::HashMap;

use rmcp::{
    ErrorData as McpError,
    model::{CallToolResult, Content},
};
use tracing::{debug, instrument};

use super::core::NeovimMcpServer;
use super::hybrid_router::DynamicTool;
use crate::neovim::{NeovimClientTrait, NeovimError};

// Core structures for Lua tool integration
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LuaToolConfig {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(skip)]
    validator: Option<LuaToolValidator>,
}

impl LuaToolConfig {
    /// Create a new LuaToolConfig with validator
    #[allow(dead_code)]
    pub fn new(
        name: String,
        description: String,
        input_schema: serde_json::Value,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let validator = LuaToolValidator::new(&input_schema)?;
        Ok(Self {
            name,
            description,
            input_schema,
            validator: Some(validator),
        })
    }

    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Initialize the validator if not already done
        if self.validator.is_none() {
            self.validator = Some(LuaToolValidator::new(&self.input_schema)?);
        }
        Ok(())
    }

    /// Get or create validator for this tool
    fn get_validator(&self) -> Result<&LuaToolValidator, Box<dyn std::error::Error>> {
        match &self.validator {
            Some(validator) => Ok(validator),
            None => Err("Validator not initialized".into()),
        }
    }
}

#[async_trait::async_trait]
impl DynamicTool for LuaToolConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> &serde_json::Value {
        &self.input_schema
    }

    /// Enhanced validation using LuaToolValidator
    fn validate_input(&self, arguments: &serde_json::Value) -> Result<(), rmcp::ErrorData> {
        match self.get_validator() {
            Ok(validator) => validator.validate(arguments).map_err(|err| {
                rmcp::ErrorData::invalid_params(
                    format!("Validation failed for Lua tool '{}': {}", self.name, err),
                    None,
                )
            }),
            Err(_) => {
                // Fallback to default validation if validator is not available
                if jsonschema::is_valid(&self.input_schema, arguments) {
                    Ok(())
                } else {
                    Err(rmcp::ErrorData::invalid_params(
                        format!(
                            "Invalid parameters for Lua tool '{}': arguments do not match the expected schema",
                            self.name
                        ),
                        None,
                    ))
                }
            }
        }
    }

    async fn call(
        &self,
        client: dashmap::mapref::one::Ref<'_, String, Box<dyn NeovimClientTrait + Send>>,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        let code = &format!(
            "return require('nvim-mcp').execute_tool('{}', vim.json.decode({:?}))",
            self.name,
            serde_json::to_string(&arguments).unwrap_or_default()
        );
        client
            .execute_lua(code)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    format!("Failed to execute Lua tool '{}': {}", self.name, e),
                    None,
                )
            })
            .and_then(|result| {
                // Convert nvim_rs::Value to serde_json::Value
                let json_result = convert_nvim_value_to_json(result)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                convert_lua_response_to_mcp(json_result)
            })
    }
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
#[allow(dead_code)]
pub struct LuaMcpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct LuaMcpMeta {
    pub error: Option<LuaErrorInfo>,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct LuaErrorInfo {
    pub code: String,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

// PATTERN: Simple validation using jsonschema crate
#[derive(Debug)]
pub struct LuaToolValidator {
    schema: serde_json::Value,
}

impl LuaToolValidator {
    pub fn new(schema: &serde_json::Value) -> Result<Self, Box<dyn std::error::Error>> {
        // Validate that the schema itself is valid
        if let Err(e) = jsonschema::Validator::new(schema) {
            return Err(format!("Invalid JSON schema: {}", e).into());
        }

        Ok(Self {
            schema: schema.clone(),
        })
    }

    pub fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        match jsonschema::is_valid(&self.schema, params) {
            true => Ok(()),
            false => Err("Validation failed: input does not match schema".to_string()),
        }
    }
}

// Helper function to check if nvim-mcp plugin is available
#[instrument(skip(client))]
async fn check_plugin_availability(client: &dyn NeovimClientTrait) -> Result<bool, NeovimError> {
    let lua_code = r#"
        local success, _ = pcall(require, 'nvim-mcp')
        return success
    "#;

    let result = client.execute_lua(lua_code).await?;

    match result {
        rmpv::Value::Boolean(available) => Ok(available),
        _ => Ok(false), // Default to false if we get unexpected response
    }
}

// CORE FUNCTION: Discover tools from Neovim instance
#[instrument(skip(client))]
pub async fn discover_lua_tools(
    client: &dyn NeovimClientTrait,
) -> Result<HashMap<String, LuaToolConfig>, NeovimError> {
    debug!("Discovering Lua tools from Neovim instance");

    // Check if nvim-mcp plugin is available before attempting discovery
    let plugin_available = check_plugin_availability(client).await?;

    if !plugin_available {
        debug!("nvim-mcp Lua plugin is not installed, skipping dynamic tool discovery");
        return Ok(HashMap::new());
    }

    let lua_code = r#"
        local nvim_mcp = require('nvim-mcp')
        return nvim_mcp.get_registered_tools()
    "#;

    let result = client.execute_lua(lua_code).await?;

    // Convert nvim_rs::Value to serde_json::Value
    let json_result = convert_nvim_value_to_json(result)?;

    // First deserialize into temporary struct without validator
    let temp_tools: Option<HashMap<String, LuaToolConfig>> = serde_json::from_value(json_result)
        .map_err(|e| NeovimError::Api(format!("Failed to parse tool configs: {}", e)))?;

    if let Some(mut tools) = temp_tools {
        debug!("Discovered {} Lua tools", tools.len());
        for tool in tools.values_mut() {
            if let Err(e) = tool.init() {
                let tool_name = tool.name();
                tracing::warn!("Failed to create validator for tool '{}': {}", tool_name, e);
                return Err(NeovimError::Api(format!(
                    "Failed to initialize tool '{}': {}",
                    tool_name, e
                )));
            }
        }
        Ok(tools)
    } else {
        debug!("No Lua tools discovered");
        return Ok(HashMap::new());
    }
}

// Helper function to convert nvim_rs::Value to serde_json::Value
pub fn convert_nvim_value_to_json(
    nvim_value: rmpv::Value,
) -> Result<serde_json::Value, NeovimError> {
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

// CONVERSION: Transform Lua MCP response to Rust CallToolResult
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
        server.register_dynamic_tool(connection_id, Box::new(tool_config))?;
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
        assert!(validator.validate(&valid_params).is_ok());

        // Invalid parameters
        let invalid_params = json!({"age": 25});
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
        assert_eq!(mcp_result.content.len(), 1);
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

    #[test]
    fn test_lua_tool_config_new_with_validator() {
        let schema = json!({
            "type": "object",
            "properties": {
                "connection_id": { "type": "string" },
                "message": { "type": "string" }
            },
            "required": ["connection_id", "message"]
        });

        let tool_config = LuaToolConfig::new(
            "test_tool".to_string(),
            "Test tool with validator".to_string(),
            schema,
        );

        assert!(tool_config.is_ok());
        let config = tool_config.unwrap();
        assert_eq!(config.name, "test_tool");
        assert!(config.validator.is_some());
    }

    #[test]
    fn test_lua_tool_config_validate_input_with_validator() {
        let schema = json!({
            "type": "object",
            "properties": {
                "connection_id": { "type": "string" },
                "count": { "type": "integer", "minimum": 0 }
            },
            "required": ["connection_id", "count"]
        });

        let tool_config =
            LuaToolConfig::new("test_tool".to_string(), "Test tool".to_string(), schema).unwrap();

        // Valid input
        let valid_input = json!({
            "connection_id": "test",
            "count": 5
        });
        assert!(tool_config.validate_input(&valid_input).is_ok());

        // Invalid input - missing required field
        let invalid_input = json!({
            "connection_id": "test"
        });
        let result = tool_config.validate_input(&invalid_input);
        assert!(result.is_err());

        // Check that error message contains tool name and validation details
        if let Err(error) = result {
            let error_str = error.to_string();
            assert!(error_str.contains("test_tool"));
            assert!(error_str.contains("Validation failed"));
        }

        // Invalid input - wrong type
        let wrong_type_input = json!({
            "connection_id": "test",
            "count": "five"
        });
        assert!(tool_config.validate_input(&wrong_type_input).is_err());

        // Invalid input - violates constraint (negative number)
        let constraint_violation = json!({
            "connection_id": "test",
            "count": -1
        });
        assert!(tool_config.validate_input(&constraint_violation).is_err());
    }

    #[test]
    fn test_lua_tool_config_fallback_validation() {
        // Create a tool config without validator
        let tool_config = LuaToolConfig {
            name: "fallback_tool".to_string(),
            description: "Tool without validator".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            }),
            validator: None,
        };

        // Valid input should work with fallback validation
        let valid_input = json!({
            "message": "hello"
        });
        assert!(tool_config.validate_input(&valid_input).is_ok());

        // Invalid input should fail with fallback validation
        let invalid_input = json!({
            "count": 123
        });
        let result = tool_config.validate_input(&invalid_input);
        assert!(result.is_err());

        if let Err(error) = result {
            let error_str = error.to_string();
            assert!(error_str.contains("fallback_tool"));
            assert!(error_str.contains("arguments do not match the expected schema"));
        }
    }

    #[test]
    fn test_lua_tool_validator_detailed_errors() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "minLength": 3 },
                "age": { "type": "integer", "minimum": 0 },
                "email": { "type": "string", "format": "email" }
            },
            "required": ["name", "age"]
        });

        let validator = LuaToolValidator::new(&schema).unwrap();

        // Test error messages for invalid input
        let invalid_input = json!({
            "name": "ab",  // too short
            "age": -5,     // negative
            "email": "not-an-email"  // invalid format
        });

        let result = validator.validate(&invalid_input);
        assert!(result.is_err());

        if let Err(error_message) = result {
            // Should contain basic validation error message
            assert!(error_message.contains("Validation failed"));
        }
    }

    #[test]
    fn test_lua_tool_validator_invalid_schema() {
        // Test with an invalid schema
        let invalid_schema = json!({
            "type": "unknown_type"  // Invalid type
        });

        let result = LuaToolValidator::new(&invalid_schema);
        assert!(result.is_err());

        if let Err(error) = result {
            assert!(error.to_string().contains("Invalid JSON schema"));
        }
    }
}
