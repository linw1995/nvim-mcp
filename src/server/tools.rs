use rmcp::{
    ErrorData as McpError,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::*,
    schemars, tool, tool_router,
};
use tracing::instrument;

use super::core::{NeovimMcpServer, find_get_all_targets};
use crate::neovim::{NeovimClient, NeovimClientTrait, Position, Range};

/// Connect to Neovim instance via unix socket or TCP
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ConnectNvimRequest {
    /// target can be a unix socket path or a TCP address
    pub target: String,
}

/// New parameter struct for connection-aware requests
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ConnectionRequest {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
}

/// Updated parameter struct for buffer operations with connection context
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct BufferConnectionRequest {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Neovim Buffer ID
    pub id: u64,
}

/// Lua execution request with connection context
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecuteLuaConnectionRequest {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Lua code to execute in Neovim
    pub code: String,
}

/// LSP parameters with connection context
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct BufferLSPConnectionParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Neovim Buffer ID
    pub id: u64,
    /// Lsp client name
    pub lsp_client_name: String,
    /// Cursor start position in the buffer, line number starts from 0
    pub line: u64,
    /// Cursor start position in the buffer, character number starts from 0
    pub character: u64,
    /// Cursor end position in the buffer, line number starts from 0
    pub end_line: u64,
    /// Cursor end position in the buffer, character number starts from 0
    pub end_character: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct HoverParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Neovim Buffer ID
    pub id: u64,
    /// Lsp client name
    pub lsp_client_name: String,
    /// Symbol position in the buffer, line number starts from 0
    pub line: u64,
    /// Symbol position in the buffer, character number starts from 0
    pub character: u64,
}

/// Document symbols parameters with connection context
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DocumentSymbolsParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Neovim Buffer ID
    pub id: u64,
    /// Lsp client name
    pub lsp_client_name: String,
}

/// Workspace symbols parameters with connection context
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WorkspaceSymbolsParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Lsp client name
    pub lsp_client_name: String,
    /// A query string to filter symbols by. Clients may send an empty string here to request all symbols.
    pub query: String,
}

#[tool_router]
impl NeovimMcpServer {
    #[tool(description = "Get available Neovim targets")]
    #[instrument(skip(self))]
    pub async fn get_targets(&self) -> Result<CallToolResult, McpError> {
        let targets = find_get_all_targets();
        if targets.is_empty() {
            return Err(McpError::invalid_request(
                "No Neovim targets found".to_string(),
                None,
            ));
        }

        Ok(CallToolResult::success(vec![Content::json(targets)?]))
    }

    #[tool(description = "Connect to Neovim instance via unix socket(pipe)")]
    #[instrument(skip(self))]
    pub async fn connect(
        &self,
        Parameters(ConnectNvimRequest { target: path }): Parameters<ConnectNvimRequest>,
    ) -> Result<CallToolResult, McpError> {
        let connection_id = self.generate_shorter_connection_id(&path);

        // If connection already exists, disconnect the old one first (ignoring errors)
        if let Some(mut old_client) = self.nvim_clients.get_mut(&connection_id) {
            let _ = old_client.disconnect().await;
        }

        let mut client = NeovimClient::new();
        client.connect_path(&path).await?;
        client.setup_diagnostics_changed_autocmd().await?;

        self.nvim_clients
            .insert(connection_id.clone(), Box::new(client));

        Ok(CallToolResult::success(vec![Content::json(
            serde_json::json!({
                "connection_id": connection_id,
                "target": path,
                "message": format!("Connected to Neovim at {path}")
            }),
        )?]))
    }

    #[tool(description = "Connect to Neovim instance via TCP")]
    #[instrument(skip(self))]
    pub async fn connect_tcp(
        &self,
        Parameters(ConnectNvimRequest { target: address }): Parameters<ConnectNvimRequest>,
    ) -> Result<CallToolResult, McpError> {
        let connection_id = self.generate_shorter_connection_id(&address);

        // If connection already exists, disconnect the old one first (ignoring errors)
        if let Some(mut old_client) = self.nvim_clients.get_mut(&connection_id) {
            let _ = old_client.disconnect().await;
        }

        let mut client = NeovimClient::new();
        client.connect_tcp(&address).await?;
        client.setup_diagnostics_changed_autocmd().await?;

        self.nvim_clients
            .insert(connection_id.clone(), Box::new(client));

        Ok(CallToolResult::success(vec![Content::json(
            serde_json::json!({
                "connection_id": connection_id,
                "target": address,
                "message": format!("Connected to Neovim at {address}")
            }),
        )?]))
    }

    #[tool(description = "Disconnect from Neovim instance")]
    #[instrument(skip(self))]
    pub async fn disconnect(
        &self,
        Parameters(ConnectionRequest { connection_id }): Parameters<ConnectionRequest>,
    ) -> Result<CallToolResult, McpError> {
        // Verify connection exists first
        let target = {
            let client = self.get_connection(&connection_id)?;
            client.target().unwrap_or_else(|| "Unknown".to_string())
        };

        // Remove the connection from the map
        if let Some((_, mut client)) = self.nvim_clients.remove(&connection_id) {
            if let Err(e) = client.disconnect().await {
                return Err(McpError::internal_error(
                    format!("Failed to disconnect: {e}"),
                    None,
                ));
            }
            Ok(CallToolResult::success(vec![Content::json(
                serde_json::json!({
                    "connection_id": connection_id,
                    "target": target,
                    "message": format!("Disconnected from Neovim at {target}")
                }),
            )?]))
        } else {
            Err(McpError::invalid_request(
                format!("No Neovim connection found for ID: {connection_id}"),
                None,
            ))
        }
    }

    #[tool(description = "List all open buffers in Neovim")]
    #[instrument(skip(self))]
    pub async fn list_buffers(
        &self,
        Parameters(ConnectionRequest { connection_id }): Parameters<ConnectionRequest>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let buffers = client.get_buffers().await?;
        Ok(CallToolResult::success(vec![Content::json(buffers)?]))
    }

    #[tool(description = "Execute Lua code in Neovim")]
    #[instrument(skip(self))]
    pub async fn exec_lua(
        &self,
        Parameters(ExecuteLuaConnectionRequest {
            connection_id,
            code,
        }): Parameters<ExecuteLuaConnectionRequest>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let result = client.execute_lua(&code).await?;
        Ok(CallToolResult::success(vec![Content::json(
            serde_json::json!({
                "result": format!("{:?}", result)
            }),
        )?]))
    }

    #[tool(description = "Get buffer's diagnostics")]
    #[instrument(skip(self))]
    pub async fn buffer_diagnostics(
        &self,
        Parameters(BufferConnectionRequest { connection_id, id }): Parameters<
            BufferConnectionRequest,
        >,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let diagnostics = client.get_buffer_diagnostics(id).await?;
        Ok(CallToolResult::success(vec![Content::json(diagnostics)?]))
    }

    #[tool(description = "Get workspace's lsp clients")]
    #[instrument(skip(self))]
    pub async fn lsp_clients(
        &self,
        Parameters(ConnectionRequest { connection_id }): Parameters<ConnectionRequest>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let lsp_clients = client.lsp_get_clients().await?;
        Ok(CallToolResult::success(vec![Content::json(lsp_clients)?]))
    }

    #[tool(description = "Get buffer's code actions")]
    #[instrument(skip(self))]
    pub async fn buffer_code_actions(
        &self,
        Parameters(BufferLSPConnectionParams {
            connection_id,
            id,
            lsp_client_name,
            line,
            character,
            end_line,
            end_character,
        }): Parameters<BufferLSPConnectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let start = Position { line, character };
        let end = Position {
            line: end_line,
            character: end_character,
        };
        let range = Range { start, end };

        let code_actions = client
            .lsp_get_code_actions(&lsp_client_name, id, range)
            .await?;
        Ok(CallToolResult::success(vec![Content::json(code_actions)?]))
    }

    #[tool(description = "Get Symbol's hover information")]
    #[instrument(skip(self))]
    pub async fn buffer_hover(
        &self,
        Parameters(HoverParams {
            connection_id,
            id,
            lsp_client_name,
            line,
            character,
        }): Parameters<HoverParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let position = Position { line, character };
        let hover = client.lsp_hover(&lsp_client_name, id, position).await?;
        Ok(CallToolResult::success(vec![Content::json(hover)?]))
    }

    #[tool(description = "Get document symbols for a buffer")]
    #[instrument(skip(self))]
    pub async fn document_symbols(
        &self,
        Parameters(DocumentSymbolsParams {
            connection_id,
            id,
            lsp_client_name,
        }): Parameters<DocumentSymbolsParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let symbols = client.lsp_document_symbols(&lsp_client_name, id).await?;
        Ok(CallToolResult::success(vec![Content::json(symbols)?]))
    }

    #[tool(description = "Search workspace symbols by query")]
    #[instrument(skip(self))]
    pub async fn workspace_symbols(
        &self,
        Parameters(WorkspaceSymbolsParams {
            connection_id,
            lsp_client_name,
            query,
        }): Parameters<WorkspaceSymbolsParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let symbols = client
            .lsp_workspace_symbols(&lsp_client_name, &query)
            .await?;
        Ok(CallToolResult::success(vec![Content::json(symbols)?]))
    }
}

/// Build tool router for NeovimMcpServer
pub fn build_tool_router() -> ToolRouter<NeovimMcpServer> {
    NeovimMcpServer::tool_router()
}
