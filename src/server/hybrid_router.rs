use std::collections::HashSet;
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

/// Type alias for a single dynamic tool instance
pub type DynamicToolBox = Box<dyn DynamicTool>;

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

    /// Dynamic tools by tool name, then by connection ID (tool_name -> connection_id -> tool)
    dynamic_tools: DynamicToolsStorage,

    /// Connection-specific tool mapping: connection_id -> tool_names
    connection_tools: Arc<DashMap<String, HashSet<String>>>,
}

impl HybridToolRouter {
    /// Create a new HybridToolRouter with the given static router
    pub fn new(static_router: ToolRouter<NeovimMcpServer>) -> Self {
        Self {
            static_router,
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
        let static_tools = self.static_router.list_all();
        tools.extend(static_tools);

        // 2. Add dynamic tools with proper metadata
        // For each tool name, we want to show one entry (representing all connections that have this tool)
        for tool_name_entry in self.dynamic_tools.iter() {
            let tool_name = tool_name_entry.key();
            let connections_map = tool_name_entry.value();

            // Pick any tool from the connections to get metadata (they should all be the same)
            if let Some(first_tool_entry) = connections_map.iter().next() {
                let tool = first_tool_entry.value();
                tools.push(Tool {
                    name: tool_name.clone().into(),
                    description: Some(tool.description().to_owned().into()),
                    input_schema: Arc::new(
                        tool.input_schema()
                            .as_object()
                            .unwrap_or(&serde_json::Map::new())
                            .clone(),
                    ),
                    output_schema: None,
                    annotations: Some(ToolAnnotations {
                        title: Some(format!(
                            "Dynamic: {} (available on {} connections)",
                            tool.name(),
                            connections_map.len()
                        )),
                        read_only_hint: Some(false),
                        destructive_hint: Some(false),
                        idempotent_hint: Some(false),
                        open_world_hint: Some(false),
                    }),
                });
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
                    tools.push(Tool {
                        name: tool.name().to_owned().into(),
                        description: Some(tool.description().to_owned().into()),
                        input_schema: Arc::new(
                            tool.input_schema()
                                .as_object()
                                .unwrap_or(&serde_json::Map::new())
                                .clone(),
                        ),
                        output_schema: None,
                        annotations: Some(ToolAnnotations {
                            title: Some(format!("Connection: {}", connection_id)),
                            read_only_hint: Some(false),
                            destructive_hint: Some(false),
                            idempotent_hint: Some(false),
                            open_world_hint: Some(false),
                        }),
                    });
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
