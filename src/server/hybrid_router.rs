use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dashmap::DashMap;
use rmcp::{
    ErrorData as McpError,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext},
    model::{CallToolRequestParam, CallToolResult, Tool, ToolAnnotations},
    service::{RequestContext, RoleServer},
};
use tracing::{debug, instrument};

use crate::neovim::NeovimClientTrait;

use super::core::NeovimMcpServer;

/// Implementation of From<&dyn DynamicTool> for rmcp::model::Tool
///
/// This implementation automatically injects the `connection_id` parameter into the JSON schema
/// properties and required fields, ensuring that all dynamic tools are compatible with the
/// MCP protocol's requirement for connection-scoped operations.
///
/// The injected `connection_id` parameter follows the standard format:
/// - type: "string"
/// - description: "Unique identifier for the target Neovim instance"
/// - required: true (added to the required array if not already present)
impl From<&dyn DynamicTool> for Tool {
    fn from(val: &dyn DynamicTool) -> Self {
        let mut schema = val
            .input_schema()
            .as_object()
            .unwrap_or(&serde_json::Map::new())
            .clone();

        // Ensure there's a properties object
        let properties = schema
            .entry("properties".to_string())
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .expect("properties should be an object");

        // Add connection_id parameter if not already present
        properties
            .entry("connection_id".to_string())
            .or_insert_with(|| {
                serde_json::json!({
                    "type": "string",
                    "description": "Unique identifier for the target Neovim instance"
                })
            });

        // Add connection_id to required fields if not already present
        let required = schema
            .entry("required".to_string())
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .expect("required should be an array");

        if !required.iter().any(|v| v.as_str() == Some("connection_id")) {
            required.push(serde_json::json!("connection_id"));
        }

        Tool {
            name: val.name().to_owned().into(),
            description: Some(val.description().to_owned().into()),
            input_schema: Arc::new(schema),
            output_schema: None,
            annotations: Some(ToolAnnotations {
                title: Some(format!("Dynamic: {}", val.name())),
                read_only_hint: None,
                destructive_hint: None,
                idempotent_hint: None,
                open_world_hint: None,
            }),
        }
    }
}

/// Type alias for a single dynamic tool instance
pub type DynamicToolBox = Box<dyn DynamicTool>;

/// Implementation of Into<rmcp::model::Tool> for DynamicToolBox (Box<dyn DynamicTool>)
/// Delegates to the trait object implementation
impl From<&DynamicToolBox> for Tool {
    fn from(val: &DynamicToolBox) -> Self {
        val.as_ref().into()
    }
}

/// Type alias for connection-to-tool mapping for a specific tool name
pub type ConnectionToolMap = DashMap<String, DynamicToolBox>;

/// Type alias for the complete dynamic tools storage structure
pub type DynamicToolsStorage = Arc<DashMap<String, ConnectionToolMap>>;

/// Dynamic tool definition with async handler
#[async_trait::async_trait]
pub trait DynamicTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> &serde_json::Value;
    fn validate_input(&self, arguments: &serde_json::Value) -> Result<(), McpError>;

    async fn call(
        &self,
        client: dashmap::mapref::one::Ref<'_, String, Box<dyn NeovimClientTrait + Send>>,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, McpError>;
}

/// Hybrid router that combines static tools (from #[tool_router] macro) with dynamic tools
pub struct HybridToolRouter {
    /// Static tools from #[tool_router] macro
    static_router: ToolRouter<NeovimMcpServer>,

    /// Static tools description overwrite (if any)
    static_tool_descriptions: HashMap<&'static str, &'static str>,

    /// Dynamic tools by tool name, then by connection ID (tool_name -> connection_id -> tool)
    dynamic_tools: DynamicToolsStorage,

    /// Connection-specific tool mapping: connection_id -> tool_names
    connection_tools: Arc<DashMap<String, HashSet<String>>>,
}

impl HybridToolRouter {
    /// Create a new HybridToolRouter with the given static router
    pub fn new(
        static_router: ToolRouter<NeovimMcpServer>,
        static_tool_descriptions: HashMap<&'static str, &'static str>,
    ) -> Self {
        Self {
            static_router,
            static_tool_descriptions,
            dynamic_tools: Arc::new(DashMap::new()),
            connection_tools: Arc::new(DashMap::new()),
        }
    }

    /// Register a connection-specific tool with clean name (recommended approach)
    #[instrument(skip(self, tool))]
    pub fn register_dynamic_tool(
        &self,
        connection_id: &str,
        tool: DynamicToolBox,
    ) -> Result<(), McpError> {
        let tool_name = tool.name().to_owned();

        // Check if tool name conflicts with static tools
        if self.static_router.has_route(&tool_name) {
            return Err(McpError::invalid_params(
                format!("Tool name '{}' conflicts with static tool", tool_name),
                None,
            ));
        }

        debug!(
            "Registering connection tool '{}' for connection '{}'",
            tool_name, connection_id
        );

        // Get or create the tools map for this tool name
        let tools_for_name = self.dynamic_tools.entry(tool_name.clone()).or_default();

        // Store the tool for this connection
        tools_for_name.insert(connection_id.to_string(), tool);

        // Track which tools belong to this connection
        self.connection_tools
            .entry(connection_id.to_string())
            .or_default()
            .insert(tool_name);

        Ok(())
    }

    /// Remove all tools for a connection (called on disconnect)
    #[instrument(skip(self))]
    pub fn unregister_dynamic_tools(&self, connection_id: &str) {
        debug!("Unregistering all tools for connection '{}'", connection_id);

        if let Some((_, tool_names)) = self.connection_tools.remove(connection_id) {
            for tool_name in tool_names {
                if let Some(tools_for_name) = self.dynamic_tools.get(&tool_name) {
                    tools_for_name.remove(connection_id);
                    debug!(
                        "Removed dynamic tool '{}' for connection '{}'",
                        tool_name, connection_id
                    );

                    // Clean up empty tool name entries
                    if tools_for_name.is_empty() {
                        drop(tools_for_name); // Release the reference before removing
                        self.dynamic_tools.remove(&tool_name);
                    }
                }
            }
        }
    }

    /// Check if a tool exists (static or dynamic)
    pub fn has_tool(&self, tool_name: &str) -> bool {
        // Check dynamic tools first
        if let Some(tools_for_name) = self.dynamic_tools.get(tool_name)
            && !tools_for_name.is_empty()
        {
            return true;
        }

        // Check static tools
        self.static_router.has_route(tool_name)
    }

    /// List all available tools (static + dynamic) for MCP list_tools request
    #[instrument(skip(self))]
    pub fn list_all_tools(&self) -> Vec<Tool> {
        let mut tools = Vec::new();

        // 1. Get static tools from macro-generated router
        // Overwrite description for static tools if it has more comprehensive description
        tools.extend(self.static_router.list_all().into_iter().map(|mut tool| {
            if let Some(desc) = self.static_tool_descriptions.get(tool.name.as_ref()) {
                tool.description = Some(desc.to_owned().into());
            }
            tool
        }));

        // 2. Add dynamic tools with proper metadata
        // For each tool name, we want to show one entry (representing all connections that have this tool)
        for tool_name_entry in self.dynamic_tools.iter() {
            let _tool_name = tool_name_entry.key();
            let connections_map = tool_name_entry.value();

            // Pick any tool from the connections to get metadata (they should all be the same)
            if let Some(first_tool_entry) = connections_map.iter().next() {
                let tool = first_tool_entry.value();
                let mut mcp_tool: Tool = tool.as_ref().into();

                // Update the title to show availability on multiple connections
                if let Some(ref mut annotations) = mcp_tool.annotations {
                    annotations.title = Some(format!(
                        "Dynamic: {} (available on {} connections)",
                        tool.name(),
                        connections_map.len()
                    ));
                }

                tools.push(mcp_tool);
            }
        }

        // Sort tools by name for consistent ordering
        tools.sort_by(|a, b| a.name.cmp(&b.name));

        debug!(
            "Listed {} total tools ({} static + {} unique dynamic)",
            tools.len(),
            self.static_router.list_all().len(),
            self.dynamic_tools.len()
        );

        tools
    }

    /// List tools for a specific connection (useful for debugging)
    #[instrument(skip(self))]
    pub fn list_connection_tools(&self, connection_id: &str) -> Vec<Tool> {
        let mut tools = Vec::new();

        // Add static tools (always available)
        tools.extend(self.static_router.list_all());

        // Add connection-specific dynamic tools
        if let Some(tool_names) = self.connection_tools.get(connection_id) {
            for tool_name in tool_names.iter() {
                if let Some(tools_for_name) = self.dynamic_tools.get(tool_name)
                    && let Some(tool) = tools_for_name.get(connection_id)
                {
                    let mut mcp_tool: Tool = tool.as_ref().into();

                    // Update annotations for connection-specific context
                    if let Some(ref mut annotations) = mcp_tool.annotations {
                        annotations.title = Some(format!("Connection: {}", connection_id));
                    }

                    tools.push(mcp_tool);
                }
            }
        }

        tools
    }

    /// Main tool call dispatch method for ServerHandler integration
    #[instrument(skip(self, server, arguments, _context))]
    pub async fn call_tool(
        &self,
        server: &NeovimMcpServer,
        tool_name: &str,
        arguments: serde_json::Value,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        debug!("HybridToolRouter dispatching tool: {}", tool_name);

        // 1. Try dynamic tools first (higher priority)
        if let Some(tools_for_name) = self.dynamic_tools.get(tool_name) {
            debug!("Found dynamic tool variants for: {}", tool_name);

            // Extract connection_id from arguments to route to the correct tool instance
            let connection_id = arguments
                .get("connection_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    McpError::invalid_params(
                        format!(
                            "Dynamic tool '{}' requires connection_id parameter",
                            tool_name
                        ),
                        None,
                    )
                })?;

            let client = server.get_connection(connection_id)?;

            if let Some(dynamic_tool) = tools_for_name.get(connection_id) {
                debug!(
                    "Executing dynamic tool: {} for connection: {}",
                    tool_name, connection_id
                );

                // Validate input arguments before execution
                dynamic_tool.validate_input(&arguments)?;

                return dynamic_tool.call(client, arguments).await;
            } else {
                return Err(McpError::invalid_request(
                    format!(
                        "Dynamic tool '{}' not available for connection '{}'",
                        tool_name, connection_id
                    ),
                    None,
                ));
            }
        }

        // 2. Fallback to static tools
        debug!("Falling back to static tool: {}", tool_name);

        // Create ToolCallContext and delegate to static router
        let request_param = CallToolRequestParam {
            name: tool_name.to_string().into(),
            arguments: Some(
                arguments
                    .as_object()
                    .unwrap_or(&serde_json::Map::new())
                    .clone(),
            ),
        };
        let tool_context = ToolCallContext::new(server, request_param, _context);
        self.static_router.call(tool_context).await
    }

    /// Get count of dynamic tools for a connection
    pub fn get_connection_tool_count(&self, connection_id: &str) -> usize {
        self.connection_tools
            .get(connection_id)
            .map(|tools| tools.len())
            .unwrap_or(0)
    }

    /// Get total number of unique dynamic tool names
    pub fn get_dynamic_tool_count(&self) -> usize {
        self.dynamic_tools.len()
    }

    /// Get reference to static router (for compatibility)
    pub fn static_router(&self) -> &ToolRouter<NeovimMcpServer> {
        &self.static_router
    }

    /// Get connection-specific tools metadata for resource listing
    pub fn get_connection_tools_info(&self, connection_id: &str) -> Vec<(String, String, bool)> {
        let mut tools_info = Vec::new();

        // Add static tools (always available)
        for tool in self.static_router.list_all() {
            tools_info.push((
                tool.name.to_string(),
                tool.description.unwrap_or_default().to_string(),
                true,
            ));
        }

        // Add connection-specific dynamic tools
        if let Some(tool_names) = self.connection_tools.get(connection_id) {
            for tool_name in tool_names.iter() {
                if let Some(tools_for_name) = self.dynamic_tools.get(tool_name)
                    && let Some(tool) = tools_for_name.get(connection_id)
                {
                    tools_info.push((tool.name().to_owned(), tool.description().to_owned(), false));
                }
            }
        }

        tools_info
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Mock implementation of DynamicTool for testing
    struct MockDynamicTool {
        name: String,
        description: String,
        schema: serde_json::Value,
    }

    #[async_trait::async_trait]
    impl DynamicTool for MockDynamicTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn input_schema(&self) -> &serde_json::Value {
            &self.schema
        }

        fn validate_input(&self, _arguments: &serde_json::Value) -> Result<(), McpError> {
            Ok(())
        }

        async fn call(
            &self,
            _client: dashmap::mapref::one::Ref<'_, String, Box<dyn NeovimClientTrait + Send>>,
            _arguments: serde_json::Value,
        ) -> Result<CallToolResult, McpError> {
            Ok(CallToolResult::success(vec![]))
        }
    }

    #[test]
    fn test_dynamic_tool_into_mcp_tool() {
        let mock_tool = MockDynamicTool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            schema: json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "A test message"
                    }
                },
                "required": ["message"]
            }),
        };

        // Test the Into<Tool> implementation
        let mcp_tool: Tool = (&mock_tool as &dyn DynamicTool).into();

        // Verify basic properties
        assert_eq!(mcp_tool.name, "test_tool");
        assert_eq!(mcp_tool.description.unwrap(), "A test tool");

        // Verify the schema was properly modified
        let schema = mcp_tool.input_schema;
        let properties = schema.get("properties").unwrap().as_object().unwrap();

        // Check that connection_id was injected
        assert!(properties.contains_key("connection_id"));
        let connection_id_prop = properties.get("connection_id").unwrap();
        assert_eq!(connection_id_prop.get("type").unwrap(), "string");
        assert_eq!(
            connection_id_prop.get("description").unwrap(),
            "Unique identifier for the target Neovim instance"
        );

        // Check that original properties are preserved
        assert!(properties.contains_key("message"));
        let message_prop = properties.get("message").unwrap();
        assert_eq!(message_prop.get("type").unwrap(), "string");
        assert_eq!(message_prop.get("description").unwrap(), "A test message");

        // Check that connection_id was added to required fields
        let required = schema.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&json!("connection_id")));
        assert!(required.contains(&json!("message")));

        // Check annotations
        assert!(mcp_tool.annotations.is_some());
        let annotations = mcp_tool.annotations.unwrap();
        assert_eq!(annotations.title.unwrap(), "Dynamic: test_tool");
    }

    #[test]
    fn test_dynamic_tool_into_mcp_tool_preserves_existing_connection_id() {
        let mock_tool = MockDynamicTool {
            name: "existing_connection_id_tool".to_string(),
            description: "A tool that already has connection_id".to_string(),
            schema: json!({
                "type": "object",
                "properties": {
                    "connection_id": {
                        "type": "string",
                        "description": "Custom connection ID description"
                    },
                    "data": {
                        "type": "string"
                    }
                },
                "required": ["connection_id", "data"]
            }),
        };

        // Test the Into<Tool> implementation
        let mcp_tool: Tool = (&mock_tool as &dyn DynamicTool).into();

        // Verify the schema
        let schema = mcp_tool.input_schema;
        let properties = schema.get("properties").unwrap().as_object().unwrap();

        // Check that the existing connection_id property was preserved (not overwritten)
        assert!(properties.contains_key("connection_id"));
        let connection_id_prop = properties.get("connection_id").unwrap();
        assert_eq!(connection_id_prop.get("type").unwrap(), "string");
        assert_eq!(
            connection_id_prop.get("description").unwrap(),
            "Custom connection ID description" // Original description preserved
        );

        // Check that required array doesn't have duplicates
        let required = schema.get("required").unwrap().as_array().unwrap();
        let connection_id_count = required
            .iter()
            .filter(|v| v.as_str() == Some("connection_id"))
            .count();
        assert_eq!(
            connection_id_count, 1,
            "connection_id should appear only once in required array"
        );
    }

    #[test]
    fn test_dynamic_tool_box_into_mcp_tool() {
        let mock_tool: DynamicToolBox = Box::new(MockDynamicTool {
            name: "boxed_tool".to_string(),
            description: "A boxed test tool".to_string(),
            schema: json!({
                "type": "object",
                "properties": {
                    "value": {
                        "type": "integer"
                    }
                },
                "required": ["value"]
            }),
        });

        // Test the Into<Tool> implementation for DynamicToolBox
        let mcp_tool: Tool = (&mock_tool).into();

        // Verify basic properties
        assert_eq!(mcp_tool.name, "boxed_tool");
        assert_eq!(mcp_tool.description.unwrap(), "A boxed test tool");

        // Verify connection_id was injected
        let schema = mcp_tool.input_schema;
        let properties = schema.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("connection_id"));
        assert!(properties.contains_key("value"));

        let required = schema.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&json!("connection_id")));
        assert!(required.contains(&json!("value")));
    }
}
