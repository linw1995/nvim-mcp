use rmcp::{
    ErrorData as McpError,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::*,
    schemars, tool, tool_router,
};
use tracing::instrument;

use super::core::{NeovimMcpServer, find_get_all_targets};
use crate::neovim::{
    CodeAction, DocumentIdentifier, NeovimClient, NeovimClientTrait, Position, Range, WorkspaceEdit,
};

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

/// Updated parameter struct for buffer operations
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct BufferRequest {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Neovim Buffer ID
    pub id: u64,
}

/// Lua execution request
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecuteLuaRequest {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Lua code to execute in Neovim
    pub code: String,
}

/// Workspace symbols parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WorkspaceSymbolsParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Lsp client name
    pub lsp_client_name: String,
    /// A query string to filter symbols by. Clients may send an empty string here to request all symbols.
    pub query: String,
}

/// Code Actions parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CodeActionsParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Universal document identifier
    pub document: DocumentIdentifier,
    /// Lsp client name
    pub lsp_client_name: String,
    /// Range start position, line number starts from 0
    pub start_line: u64,
    /// Range start position, character number starts from 0
    pub start_character: u64,
    /// Range end position, line number starts from 0
    pub end_line: u64,
    /// Range end position, character number starts from 0
    pub end_character: u64,
}

/// Hover parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct HoverParam {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Universal document identifier
    pub document: DocumentIdentifier,
    /// Lsp client name
    pub lsp_client_name: String,
    /// Symbol position, line number starts from 0
    pub line: u64,
    /// Symbol position, character number starts from 0
    pub character: u64,
}

/// Document symbols parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DocumentSymbolsParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Universal document identifier
    pub document: DocumentIdentifier,
    /// Lsp client name
    pub lsp_client_name: String,
}

/// References parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReferencesParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Universal document identifier
    pub document: DocumentIdentifier,
    /// Lsp client name
    pub lsp_client_name: String,
    /// Symbol position, line number starts from 0
    pub line: u64,
    /// Symbol position, character number starts from 0
    pub character: u64,
    /// Include the declaration of the current symbol in the results
    pub include_declaration: bool,
}

/// Code action resolve parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ResolveCodeActionParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Lsp client name
    pub lsp_client_name: String,
    /// Code action to resolve
    pub code_action: CodeAction,
}

/// Apply workspace edit parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ApplyWorkspaceEditParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Lsp client name
    pub lsp_client_name: String,
    /// Workspace edit to apply
    pub workspace_edit: WorkspaceEdit,
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
        Parameters(ExecuteLuaRequest {
            connection_id,
            code,
        }): Parameters<ExecuteLuaRequest>,
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
        Parameters(BufferRequest { connection_id, id }): Parameters<BufferRequest>,
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

    #[tool(description = "Search workspace symbols by query")]
    #[instrument(skip(self))]
    pub async fn lsp_workspace_symbols(
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

    #[tool(description = "Get LSP code actions")]
    #[instrument(skip(self))]
    pub async fn lsp_code_actions(
        &self,
        Parameters(CodeActionsParams {
            connection_id,
            document,
            lsp_client_name,
            start_line,
            start_character,
            end_line,
            end_character,
        }): Parameters<CodeActionsParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let start = Position {
            line: start_line,
            character: start_character,
        };
        let end = Position {
            line: end_line,
            character: end_character,
        };
        let range = Range { start, end };

        let code_actions = client
            .lsp_get_code_actions(&lsp_client_name, document, range)
            .await?;
        Ok(CallToolResult::success(vec![Content::json(code_actions)?]))
    }

    #[tool(description = "Get LSP hover information")]
    #[instrument(skip(self))]
    pub async fn lsp_hover(
        &self,
        Parameters(HoverParam {
            connection_id,
            document,
            lsp_client_name,
            line,
            character,
        }): Parameters<HoverParam>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let position = Position { line, character };
        let hover = client
            .lsp_hover(&lsp_client_name, document, position)
            .await?;
        Ok(CallToolResult::success(vec![Content::json(hover)?]))
    }

    #[tool(description = "Get document symbols")]
    #[instrument(skip(self))]
    pub async fn lsp_document_symbols(
        &self,
        Parameters(DocumentSymbolsParams {
            connection_id,
            document,
            lsp_client_name,
        }): Parameters<DocumentSymbolsParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let symbols = client
            .lsp_document_symbols(&lsp_client_name, document)
            .await?;
        Ok(CallToolResult::success(vec![Content::json(symbols)?]))
    }

    #[tool(description = "Get LSP references")]
    #[instrument(skip(self))]
    pub async fn lsp_references(
        &self,
        Parameters(ReferencesParams {
            connection_id,
            document,
            lsp_client_name,
            line,
            character,
            include_declaration,
        }): Parameters<ReferencesParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let position = Position { line, character };
        let references = client
            .lsp_references(&lsp_client_name, document, position, include_declaration)
            .await?;
        Ok(CallToolResult::success(vec![Content::json(references)?]))
    }

    #[tool(description = "Resolve a code action that may have incomplete data")]
    #[instrument(skip(self))]
    pub async fn lsp_resolve_code_action(
        &self,
        Parameters(ResolveCodeActionParams {
            connection_id,
            lsp_client_name,
            code_action,
        }): Parameters<ResolveCodeActionParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let resolved_action = client
            .lsp_resolve_code_action(&lsp_client_name, code_action)
            .await?;
        Ok(CallToolResult::success(vec![Content::json(
            resolved_action,
        )?]))
    }

    #[tool(description = "Apply a workspace edit using the LSP workspace/applyEdit method")]
    #[instrument(skip(self))]
    pub async fn lsp_apply_edit(
        &self,
        Parameters(ApplyWorkspaceEditParams {
            connection_id,
            lsp_client_name,
            workspace_edit,
        }): Parameters<ApplyWorkspaceEditParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let result = client
            .lsp_apply_workspace_edit(&lsp_client_name, workspace_edit)
            .await?;
        Ok(CallToolResult::success(vec![Content::json(result)?]))
    }
}

/// Build tool router for NeovimMcpServer
pub fn build_tool_router() -> ToolRouter<NeovimMcpServer> {
    NeovimMcpServer::tool_router()
}
