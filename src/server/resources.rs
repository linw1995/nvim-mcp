use regex::Regex;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    model::*,
    service::{RequestContext, RoleServer},
};
use serde_json::json;
use tracing::{debug, instrument};

use super::core::NeovimMcpServer;

// Manual ServerHandler implementation to override tool methods
impl ServerHandler for NeovimMcpServer {
    #[instrument(skip(self))]
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: None,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .enable_resources()
                .build(),
            ..Default::default()
        }
    }

    #[instrument(skip(self))]
    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        debug!("Listing available diagnostic resources");

        let mut resources = vec![
            Resource {
                raw: RawResource {
                    uri: "nvim-connections://".to_string(),
                    name: "Active Neovim Connections".to_string(),
                    description: Some("List of active Neovim connections".to_string()),
                    mime_type: Some("application/json".to_string()),
                    size: None,
                },
                annotations: None,
            },
            Resource {
                raw: RawResource {
                    uri: "nvim-tools://".to_string(),
                    name: "Tool Registration Overview".to_string(),
                    description: Some(
                        "Overview of all tools and their connection mappings".to_string(),
                    ),
                    mime_type: Some("application/json".to_string()),
                    size: None,
                },
                annotations: None,
            },
        ];

        // Add connection-specific resources
        for connection_entry in self.nvim_clients.iter() {
            let connection_id = connection_entry.key().clone();

            // Add diagnostic resource
            resources.push(Resource {
                raw: RawResource {
                    uri: format!("nvim-diagnostics://{connection_id}/workspace"),
                    name: format!("Workspace Diagnostics ({connection_id})"),
                    description: Some(format!(
                        "Diagnostic messages for connection {connection_id}"
                    )),
                    mime_type: Some("application/json".to_string()),
                    size: None,
                },
                annotations: None,
            });

            // Add connection-specific tools resource
            resources.push(Resource {
                raw: RawResource {
                    uri: format!("nvim-tools://{connection_id}"),
                    name: format!("Tools for Connection ({connection_id})"),
                    description: Some(format!(
                        "List of tools available for connection {connection_id}"
                    )),
                    mime_type: Some("application/json".to_string()),
                    size: None,
                },
                annotations: None,
            });
        }

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
        })
    }

    #[instrument(skip(self))]
    async fn read_resource(
        &self,
        ReadResourceRequestParam { uri }: ReadResourceRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        debug!("Reading resource: {}", uri);

        match uri.as_str() {
            "nvim-connections://" => {
                let connections: Vec<_> = self
                    .nvim_clients
                    .iter()
                    .map(|entry| {
                        json!({
                            "id": entry.key(),
                            "target": entry.value().target()
                                .unwrap_or_else(|| "Unknown".to_string())
                        })
                    })
                    .collect();

                Ok(ReadResourceResult {
                    contents: vec![ResourceContents::text(
                        serde_json::to_string_pretty(&connections).map_err(|e| {
                            McpError::internal_error(
                                "Failed to serialize connections",
                                Some(json!({"error": e.to_string()})),
                            )
                        })?,
                        uri,
                    )],
                })
            }
            "nvim-tools://" => {
                // Overview of all tools and their connection mappings
                let static_tools: Vec<_> = self
                    .hybrid_router
                    .static_router()
                    .list_all()
                    .into_iter()
                    .map(|tool| {
                        json!({
                            "name": tool.name,
                            "description": tool.description,
                            "type": "static",
                            "available_to": "all_connections"
                        })
                    })
                    .collect();

                let mut connection_tools = json!({});
                for connection_entry in self.nvim_clients.iter() {
                    let connection_id = connection_entry.key();
                    let tools_info = self.hybrid_router.get_connection_tools_info(connection_id);
                    let dynamic_tools: Vec<_> = tools_info
                        .into_iter()
                        .filter(|(_, _, is_static)| !is_static) // Only show dynamic tools
                        .map(|(name, description, _)| {
                            json!({
                                "name": name,
                                "description": description,
                                "type": "dynamic"
                            })
                        })
                        .collect();

                    connection_tools[connection_id] = json!(dynamic_tools);
                }

                let overview = json!({
                    "static_tools": static_tools,
                    "connection_specific_tools": connection_tools
                });

                Ok(ReadResourceResult {
                    contents: vec![ResourceContents::text(
                        serde_json::to_string_pretty(&overview).map_err(|e| {
                            McpError::internal_error(
                                "Failed to serialize tools overview",
                                Some(json!({"error": e.to_string()})),
                            )
                        })?,
                        uri,
                    )],
                })
            }
            uri if uri.starts_with("nvim-tools://") => {
                // Handle connection-specific tool resources like "nvim-tools://{connection_id}"
                let connection_id = uri.strip_prefix("nvim-tools://").unwrap();

                if connection_id.is_empty() {
                    return Err(McpError::invalid_params(
                        "Missing connection ID in tools URI",
                        None,
                    ));
                }

                // Verify connection exists
                let _client = self.get_connection(connection_id)?;

                // Get clean tools info for this connection
                let tools_info_data = self.hybrid_router.get_connection_tools_info(connection_id);
                let tools_info: Vec<_> = tools_info_data
                    .into_iter()
                    .map(|(name, description, is_static)| {
                        json!({
                            "name": name,
                            "description": description,
                            "type": if is_static { "static" } else { "dynamic" },
                            "connection_id": connection_id
                        })
                    })
                    .collect();

                let result = json!({
                    "connection_id": connection_id,
                    "tools": tools_info,
                    "total_count": tools_info.len(),
                    "dynamic_count": self.hybrid_router.get_connection_tool_count(connection_id)
                });

                Ok(ReadResourceResult {
                    contents: vec![ResourceContents::text(
                        serde_json::to_string_pretty(&result).map_err(|e| {
                            McpError::internal_error(
                                "Failed to serialize connection tools",
                                Some(json!({"error": e.to_string()})),
                            )
                        })?,
                        uri,
                    )],
                })
            }
            uri if uri.starts_with("nvim-diagnostics://") => {
                // Parse connection_id from URI pattern using regex
                let connection_diagnostics_regex = Regex::new(r"nvim-diagnostics://([^/]+)/(.+)")
                    .map_err(|e| {
                    McpError::internal_error(
                        "Failed to compile regex",
                        Some(json!({"error": e.to_string()})),
                    )
                })?;

                if let Some(captures) = connection_diagnostics_regex.captures(uri) {
                    let connection_id = captures.get(1).unwrap().as_str();
                    let resource_type = captures.get(2).unwrap().as_str();

                    let client = self.get_connection(connection_id)?;

                    match resource_type {
                        "workspace" => {
                            let diagnostics = client.get_workspace_diagnostics().await?;
                            Ok(ReadResourceResult {
                                contents: vec![ResourceContents::text(
                                    serde_json::to_string_pretty(&diagnostics).map_err(|e| {
                                        McpError::internal_error(
                                            "Failed to serialize workspace diagnostics",
                                            Some(json!({"error": e.to_string()})),
                                        )
                                    })?,
                                    uri,
                                )],
                            })
                        }
                        path if path.starts_with("buffer/") => {
                            let buffer_id = path
                                .strip_prefix("buffer/")
                                .and_then(|s| s.parse::<u64>().ok())
                                .ok_or_else(|| {
                                    McpError::invalid_params("Invalid buffer ID", None)
                                })?;

                            let diagnostics = client.get_buffer_diagnostics(buffer_id).await?;
                            Ok(ReadResourceResult {
                                contents: vec![ResourceContents::text(
                                    serde_json::to_string_pretty(&diagnostics).map_err(|e| {
                                        McpError::internal_error(
                                            "Failed to serialize buffer diagnostics",
                                            Some(json!({"error": e.to_string()})),
                                        )
                                    })?,
                                    uri,
                                )],
                            })
                        }
                        _ => Err(McpError::resource_not_found(
                            "resource_not_found",
                            Some(json!({"uri": uri})),
                        )),
                    }
                } else {
                    Err(McpError::resource_not_found(
                        "resource_not_found",
                        Some(json!({"uri": uri})),
                    ))
                }
            }
            _ => Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({"uri": uri})),
            )),
        }
    }

    // Override list_tools to use HybridToolRouter
    #[instrument(skip(self))]
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        debug!("Listing tools (static + dynamic) via HybridToolRouter");

        // Get tools from HybridToolRouter instead of static router
        let mut tools = self.hybrid_router.list_all_tools();

        for tool in &mut tools {
            if let Some(extra) = self.get_tool_extra_description(&tool.name) {
                if let Some(desc) = &mut tool.description {
                    let new_desc = format!("{}\n{}", desc, extra).trim().to_string();
                    *desc = new_desc.into();
                } else {
                    tool.description = Some(extra.into());
                }
            }
        }
        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
    }

    // Override call_tool to use HybridToolRouter
    #[instrument(skip(self))]
    async fn call_tool(
        &self,
        CallToolRequestParam { name, arguments }: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        debug!("Calling tool: {} via HybridToolRouter", name);

        // Convert arguments to serde_json::Value
        let args = arguments.unwrap_or_default();
        let args_value = serde_json::to_value(args).map_err(|e| {
            McpError::invalid_params(
                "Failed to serialize arguments",
                Some(json!({"error": e.to_string()})),
            )
        })?;

        // Use HybridToolRouter for dispatch
        self.hybrid_router
            .call_tool(self, &name, args_value, context)
            .await
    }
}
