use std::collections::HashSet;
use std::sync::Arc;

use dashmap::DashMap;
use futures::future::BoxFuture;
use rmcp::{
    ErrorData as McpError,
    handler::server::router::tool::ToolRouter,
    model::{CallToolResult, Tool, ToolAnnotations},
    service::{RequestContext, RoleServer},
};
use tracing::{debug, instrument};

use super::core::NeovimMcpServer;
/// Type alias for the dynamic tool handler function
type DynamicToolHandler = Arc<
    dyn Fn(
            &NeovimMcpServer,
            serde_json::Value,
        ) -> BoxFuture<'static, Result<CallToolResult, McpError>>
        + Send
        + Sync,
>;

/// Dynamic tool definition with async handler
pub struct DynamicTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub handler: DynamicToolHandler,
}

/// Hybrid router that combines static tools (from #[tool_router] macro) with dynamic tools
pub struct HybridToolRouter {
    /// Static tools from #[tool_router] macro
    static_router: ToolRouter<NeovimMcpServer>,

    /// Dynamic tools by name (includes connection-scoped tools)
    dynamic_tools: Arc<DashMap<String, DynamicTool>>,

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

    /// Register a dynamic tool for a specific connection
    #[instrument(skip(self, tool))]
    pub fn register_dynamic_tool(
        &self,
        connection_id: &str,
        tool: DynamicTool,
    ) -> Result<(), McpError> {
        // Create connection-scoped tool name to avoid conflicts
        let scoped_tool_name = format!("{}_{}", connection_id, tool.name);

        debug!(
            "Registering dynamic tool '{}' for connection '{}'",
            scoped_tool_name, connection_id
        );

        // Store the tool
        self.dynamic_tools.insert(scoped_tool_name.clone(), tool);

        // Track which tools belong to this connection
        self.connection_tools
            .entry(connection_id.to_string())
            .or_default()
            .insert(scoped_tool_name);

        Ok(())
    }

    /// Register a global dynamic tool (not connection-scoped)
    #[instrument(skip(self, tool))]
    pub fn register_global_dynamic_tool(&self, tool: DynamicTool) -> Result<(), McpError> {
        let tool_name = tool.name.clone();

        debug!("Registering global dynamic tool '{}'", tool_name);

        // Check if tool name conflicts with static tools
        if self.static_router.has_route(&tool_name) {
            return Err(McpError::invalid_params(
                format!("Tool name '{}' conflicts with static tool", tool_name),
                None,
            ));
        }

        self.dynamic_tools.insert(tool_name, tool);
        Ok(())
    }

    /// Remove all tools for a connection (called on disconnect)
    #[instrument(skip(self))]
    pub fn unregister_connection_tools(&self, connection_id: &str) {
        debug!("Unregistering all tools for connection '{}'", connection_id);

        if let Some((_, tool_names)) = self.connection_tools.remove(connection_id) {
            for tool_name in tool_names {
                self.dynamic_tools.remove(&tool_name);
                debug!("Removed dynamic tool '{}'", tool_name);
            }
        }
    }

    /// Check if a tool exists (static or dynamic)
    pub fn has_tool(&self, tool_name: &str) -> bool {
        // Check dynamic tools first
        if self.dynamic_tools.contains_key(tool_name) {
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
        for entry in self.dynamic_tools.iter() {
            let tool = entry.value();
            tools.push(Tool {
                name: entry.key().clone().into(),
                description: Some(tool.description.clone().into()),
                input_schema: Arc::new(
                    tool.input_schema
                        .as_object()
                        .unwrap_or(&serde_json::Map::new())
                        .clone(),
                ),
                output_schema: None,
                annotations: Some(ToolAnnotations {
                    title: Some(format!("Dynamic: {}", tool.name)),
                    read_only_hint: Some(false),
                    destructive_hint: Some(false),
                    idempotent_hint: Some(false),
                    open_world_hint: Some(false),
                }),
            });
        }

        // Sort tools by name for consistent ordering
        tools.sort_by(|a, b| a.name.cmp(&b.name));

        debug!(
            "Listed {} total tools ({} static + {} dynamic)",
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
                if let Some(tool) = self.dynamic_tools.get(tool_name) {
                    tools.push(Tool {
                        name: tool.name.clone().into(),
                        description: Some(tool.description.clone().into()),
                        input_schema: Arc::new(
                            tool.input_schema
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
        if let Some(dynamic_tool) = self.dynamic_tools.get(tool_name) {
            debug!("Executing dynamic tool: {}", tool_name);
            return (dynamic_tool.handler)(server, arguments).await;
        }

        // 2. Try connection-scoped tools (parse connection_id from tool name)
        if let Some((connection_id, base_tool_name)) = self.parse_scoped_tool_name(tool_name) {
            let scoped_name = format!("{}_{}", connection_id, base_tool_name);
            if let Some(dynamic_tool) = self.dynamic_tools.get(&scoped_name) {
                debug!(
                    "Executing connection-scoped tool: {} for connection: {}",
                    base_tool_name, connection_id
                );
                return (dynamic_tool.handler)(server, arguments).await;
            }
        }

        // 3. Fallback to static tools
        debug!("Falling back to static tool: {}", tool_name);

        // For static tools, we need to delegate to the actual tool methods
        // Since the #[tool_router] macro generates these methods on NeovimMcpServer,
        // we need to call them directly through the server instance
        match tool_name {
            "get_targets" => server.get_targets().await,
            "connect" => {
                // Extract target from arguments
                let target = arguments
                    .get("target")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("Missing 'target' parameter", None))?;

                use crate::server::tools::ConnectNvimRequest;
                use rmcp::handler::server::tool::Parameters;
                server
                    .connect(Parameters(ConnectNvimRequest {
                        target: target.to_string(),
                    }))
                    .await
            }
            "connect_tcp" => {
                let target = arguments
                    .get("target")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("Missing 'target' parameter", None))?;

                use crate::server::tools::ConnectNvimRequest;
                use rmcp::handler::server::tool::Parameters;
                server
                    .connect_tcp(Parameters(ConnectNvimRequest {
                        target: target.to_string(),
                    }))
                    .await
            }
            "disconnect" => {
                let connection_id = arguments
                    .get("connection_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::invalid_params("Missing 'connection_id' parameter", None)
                    })?;

                use crate::server::tools::ConnectionRequest;
                use rmcp::handler::server::tool::Parameters;
                server
                    .disconnect(Parameters(ConnectionRequest {
                        connection_id: connection_id.to_string(),
                    }))
                    .await
            }
            "list_buffers" => {
                let connection_id = arguments
                    .get("connection_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::invalid_params("Missing 'connection_id' parameter", None)
                    })?;

                use crate::server::tools::ConnectionRequest;
                use rmcp::handler::server::tool::Parameters;
                server
                    .list_buffers(Parameters(ConnectionRequest {
                        connection_id: connection_id.to_string(),
                    }))
                    .await
            }
            "exec_lua" => {
                let connection_id = arguments
                    .get("connection_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::invalid_params("Missing 'connection_id' parameter", None)
                    })?;
                let code = arguments
                    .get("code")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("Missing 'code' parameter", None))?;

                use crate::server::tools::ExecuteLuaRequest;
                use rmcp::handler::server::tool::Parameters;
                server
                    .exec_lua(Parameters(ExecuteLuaRequest {
                        connection_id: connection_id.to_string(),
                        code: code.to_string(),
                    }))
                    .await
            }
            "lsp_clients" => {
                let connection_id = arguments
                    .get("connection_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::invalid_params("Missing 'connection_id' parameter", None)
                    })?;

                use crate::server::tools::ConnectionRequest;
                use rmcp::handler::server::tool::Parameters;
                server
                    .lsp_clients(Parameters(ConnectionRequest {
                        connection_id: connection_id.to_string(),
                    }))
                    .await
            }
            // Add more static tool handlers as needed
            _ => Err(McpError::invalid_request(
                format!("Tool '{}' not found", tool_name),
                None,
            )),
        }
    }

    /// Parse connection-scoped tool names like "f303ec5_treesitter_query"
    fn parse_scoped_tool_name<'a>(&self, tool_name: &'a str) -> Option<(&'a str, &'a str)> {
        // Look for pattern: {connection_id}_{tool_name}
        if let Some(first_underscore) = tool_name.find('_') {
            let connection_id = &tool_name[..first_underscore];
            let base_tool_name = &tool_name[first_underscore + 1..];

            // Verify this connection exists
            if self.connection_tools.contains_key(connection_id) {
                return Some((connection_id, base_tool_name));
            }
        }
        None
    }

    /// Get count of dynamic tools for a connection
    pub fn get_connection_tool_count(&self, connection_id: &str) -> usize {
        self.connection_tools
            .get(connection_id)
            .map(|tools| tools.len())
            .unwrap_or(0)
    }

    /// Get total number of dynamic tools
    pub fn get_dynamic_tool_count(&self) -> usize {
        self.dynamic_tools.len()
    }

    /// Get reference to static router (for compatibility)
    pub fn static_router(&self) -> &ToolRouter<NeovimMcpServer> {
        &self.static_router
    }
}
