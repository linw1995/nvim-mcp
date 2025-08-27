use std::collections::HashMap;

use rmcp::{
    ErrorData as McpError, RoleServer,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::*,
    schemars,
    service::RequestContext,
    tool, tool_router,
};
use tracing::instrument;

use super::core::{NeovimMcpServer, find_get_all_targets};
use super::lua_tools;
use crate::neovim::{
    CodeAction, DocumentIdentifier, FormattingOptions, NeovimClient, NeovimClientTrait, Position,
    PrepareRenameResult, Range, WorkspaceEdit, string_or_struct,
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

/// Wait for LSP client readiness parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WaitForLspReadyRequest {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Optional specific LSP client name to wait for (waits for any client if None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_name: Option<String>,
    /// Timeout in milliseconds (default 5000ms)
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    5000
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
    // Supports both string and struct deserialization.
    // Compatible with Claude Code when using subscription.
    #[serde(deserialize_with = "string_or_struct")]
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
    // Supports both string and struct deserialization.
    // Compatible with Claude Code when using subscription.
    #[serde(deserialize_with = "string_or_struct")]
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
    // Supports both string and struct deserialization.
    // Compatible with Claude Code when using subscription.
    #[serde(deserialize_with = "string_or_struct")]
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
    // Supports both string and struct deserialization.
    // Compatible with Claude Code when using subscription.
    #[serde(deserialize_with = "string_or_struct")]
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

/// Definition parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DefinitionParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Universal document identifier
    // Supports both string and struct deserialization.
    // Compatible with Claude Code when using subscription.
    #[serde(deserialize_with = "string_or_struct")]
    pub document: DocumentIdentifier,
    /// Lsp client name
    pub lsp_client_name: String,
    /// Symbol position, line number starts from 0
    pub line: u64,
    /// Symbol position, character number starts from 0
    pub character: u64,
}

/// Type definition parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TypeDefinitionParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Universal document identifier
    // Supports both string and struct deserialization.
    // Compatible with Claude Code when using subscription.
    #[serde(deserialize_with = "string_or_struct")]
    pub document: DocumentIdentifier,
    /// Lsp client name
    pub lsp_client_name: String,
    /// Symbol position, line number starts from 0
    pub line: u64,
    /// Symbol position, character number starts from 0
    pub character: u64,
}

/// Implementation parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ImplementationParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Universal document identifier
    // Supports both string and struct deserialization.
    // Compatible with Claude Code when using subscription.
    #[serde(deserialize_with = "string_or_struct")]
    pub document: DocumentIdentifier,
    /// Lsp client name
    pub lsp_client_name: String,
    /// Symbol position, line number starts from 0
    pub line: u64,
    /// Symbol position, character number starts from 0
    pub character: u64,
}

/// Declaration parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeclarationParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Universal document identifier
    // Supports both string and struct deserialization.
    // Compatible with Claude Code when using subscription.
    #[serde(deserialize_with = "string_or_struct")]
    pub document: DocumentIdentifier,
    /// Lsp client name
    pub lsp_client_name: String,
    /// Symbol position, line number starts from 0
    pub line: u64,
    /// Symbol position, character number starts from 0
    pub character: u64,
}

/// Code action resolve parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ResolveCodeActionParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Lsp client name
    pub lsp_client_name: String,
    /// Code action to resolve
    // Supports both string and struct deserialization.
    // Compatible with Claude Code when using subscription.
    #[serde(deserialize_with = "string_or_struct")]
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
    // Supports both string and struct deserialization.
    // Compatible with Claude Code when using subscription.
    #[serde(deserialize_with = "string_or_struct")]
    pub workspace_edit: WorkspaceEdit,
}

/// Rename parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RenameParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Universal document identifier
    // Supports both string and struct deserialization.
    // Compatible with Claude Code when using subscription.
    #[serde(deserialize_with = "string_or_struct")]
    pub document: DocumentIdentifier,
    /// Lsp client name
    pub lsp_client_name: String,
    /// Symbol position, line number starts from 0
    pub line: u64,
    /// Symbol position, character number starts from 0
    pub character: u64,
    /// The new name of the symbol
    pub new_name: String,
    /// Whether to run prepare rename first to validate the position (default: true)
    #[serde(default = "default_prepare_first")]
    pub prepare_first: bool,
}

fn default_prepare_first() -> bool {
    true
}

/// Document formatting parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DocumentFormattingParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Universal document identifier
    // Supports both string and struct deserialization.
    // Compatible with Claude Code when using subscription.
    #[serde(deserialize_with = "string_or_struct")]
    pub document: DocumentIdentifier,
    /// Lsp client name
    pub lsp_client_name: String,
    /// The formatting options
    pub options: FormattingOptions,
    /// Whether to apply the text edits automatically (default: false)
    #[serde(default)]
    pub apply_edits: bool,
}

/// Document range formatting parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DocumentRangeFormattingParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Universal document identifier
    // Supports both string and struct deserialization.
    // Compatible with Claude Code when using subscription.
    #[serde(deserialize_with = "string_or_struct")]
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
    /// The formatting options
    #[serde(deserialize_with = "string_or_struct")]
    pub options: FormattingOptions,
    /// Whether to apply the text edits automatically (default: false)
    #[serde(default)]
    pub apply_edits: bool,
}

/// Organize imports parameters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct LspOrganizeImportsParams {
    /// Unique identifier for the target Neovim instance
    pub connection_id: String,
    /// Universal document identifier
    // Supports both string and struct deserialization.
    // Compatible with Claude Code when using subscription.
    #[serde(deserialize_with = "string_or_struct")]
    pub document: DocumentIdentifier,
    /// Lsp client name
    pub lsp_client_name: String,
    /// Whether to apply the text edits automatically (default: true)
    #[serde(default = "default_true")]
    pub apply_edits: bool,
}

fn default_true() -> bool {
    true
}

macro_rules! include_files {
    ($($key:ident),* $(,)?) => {{
        let mut map = HashMap::new();
        $(
            map.insert(stringify!($key), include_str!(concat!("../../docs/tools/", stringify!($key), ".md")));
        )*
        map
    }};
}

impl NeovimMcpServer {
    pub fn tool_descriptions() -> HashMap<&'static str, &'static str> {
        include_files! {
            get_targets,
            connect,
        }
    }
}

#[tool_router]
impl NeovimMcpServer {
    #[tool]
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

    #[tool]
    #[instrument(skip(self))]
    pub async fn connect(
        &self,
        Parameters(ConnectNvimRequest { target: path }): Parameters<ConnectNvimRequest>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let connection_id = self.generate_shorter_connection_id(&path);

        // If connection already exists, disconnect the old one first (ignoring errors)
        if let Some(mut old_client) = self.nvim_clients.get_mut(&connection_id) {
            let _ = old_client.disconnect().await;
        }

        let mut client = NeovimClient::new();
        client.connect_path(&path).await?;
        client.setup_autocmd().await?;

        // Discover and register Lua tools for this connection
        if let Err(e) =
            lua_tools::discover_and_register_lua_tools(self, &connection_id, &client).await
        {
            tracing::warn!(
                "Failed to discover Lua tools for connection '{}': {}",
                connection_id,
                e
            );
        } else {
            ctx.peer
                .notify_tool_list_changed()
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        "Failed to notify tool list changed for connection '{}': {}",
                        connection_id,
                        e
                    );
                });
        }

        self.nvim_clients
            .insert(connection_id.clone(), Box::new(client));

        Ok(CallToolResult::success(vec![Content::json(
            serde_json::json!({
                "connection_id": connection_id,
                "target": path,
            }),
        )?]))
    }

    #[tool(description = "Connect via TCP address")]
    #[instrument(skip(self))]
    pub async fn connect_tcp(
        &self,
        Parameters(ConnectNvimRequest { target: address }): Parameters<ConnectNvimRequest>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let connection_id = self.generate_shorter_connection_id(&address);

        // If connection already exists, disconnect the old one first (ignoring errors)
        if let Some(mut old_client) = self.nvim_clients.get_mut(&connection_id) {
            let _ = old_client.disconnect().await;
        }

        let mut client = NeovimClient::new();
        client.connect_tcp(&address).await?;
        client.setup_autocmd().await?;

        // Discover and register Lua tools for this connection
        if let Err(e) =
            lua_tools::discover_and_register_lua_tools(self, &connection_id, &client).await
        {
            tracing::warn!(
                "Failed to discover Lua tools for connection '{}': {}",
                connection_id,
                e
            );
        } else {
            ctx.peer
                .notify_tool_list_changed()
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        "Failed to notify tool list changed for connection '{}': {}",
                        connection_id,
                        e
                    );
                });
        }

        self.nvim_clients
            .insert(connection_id.clone(), Box::new(client));

        Ok(CallToolResult::success(vec![Content::json(
            serde_json::json!({
                "connection_id": connection_id,
                "target": address,
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

    #[tool(description = "List all open buffers")]
    #[instrument(skip(self))]
    pub async fn list_buffers(
        &self,
        Parameters(ConnectionRequest { connection_id }): Parameters<ConnectionRequest>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let buffers = client.get_buffers().await?;
        Ok(CallToolResult::success(vec![Content::json(buffers)?]))
    }

    #[tool(description = "Execute Lua code")]
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

    #[tool(description = "Wait for LSP client to be ready and attached")]
    #[instrument(skip(self))]
    pub async fn wait_for_lsp_ready(
        &self,
        Parameters(WaitForLspReadyRequest {
            connection_id,
            client_name,
            timeout_ms,
        }): Parameters<WaitForLspReadyRequest>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        client
            .wait_for_lsp_ready(client_name.as_deref(), timeout_ms)
            .await?;
        Ok(CallToolResult::success(vec![Content::json(
            serde_json::json!({
                "message": "LSP client ready",
                "client_name": client_name.unwrap_or_else(|| "any".to_string()),
                "timeout_ms": timeout_ms
            }),
        )?]))
    }

    #[tool(description = "Get diagnostics for specific buffer")]
    #[instrument(skip(self))]
    pub async fn buffer_diagnostics(
        &self,
        Parameters(BufferRequest { connection_id, id }): Parameters<BufferRequest>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let diagnostics = client.get_buffer_diagnostics(id).await?;
        Ok(CallToolResult::success(vec![Content::json(diagnostics)?]))
    }

    #[tool(description = "Get workspace LSP clients")]
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

    #[tool(description = "Get LSP definition")]
    #[instrument(skip(self))]
    pub async fn lsp_definition(
        &self,
        Parameters(DefinitionParams {
            connection_id,
            document,
            lsp_client_name,
            line,
            character,
        }): Parameters<DefinitionParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let position = Position { line, character };
        let definition = client
            .lsp_definition(&lsp_client_name, document, position)
            .await?;
        Ok(CallToolResult::success(vec![Content::json(definition)?]))
    }

    #[tool(description = "Get LSP type definition")]
    #[instrument(skip(self))]
    pub async fn lsp_type_definition(
        &self,
        Parameters(TypeDefinitionParams {
            connection_id,
            document,
            lsp_client_name,
            line,
            character,
        }): Parameters<TypeDefinitionParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let position = Position { line, character };
        let type_definition = client
            .lsp_type_definition(&lsp_client_name, document, position)
            .await?;
        Ok(CallToolResult::success(vec![Content::json(
            type_definition,
        )?]))
    }

    #[tool(description = "Get LSP implementation")]
    #[instrument(skip(self))]
    pub async fn lsp_implementations(
        &self,
        Parameters(ImplementationParams {
            connection_id,
            document,
            lsp_client_name,
            line,
            character,
        }): Parameters<ImplementationParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let position = Position { line, character };
        let implementation = client
            .lsp_implementation(&lsp_client_name, document, position)
            .await?;
        Ok(CallToolResult::success(vec![Content::json(
            implementation,
        )?]))
    }

    #[tool(description = "Get LSP declaration")]
    #[instrument(skip(self))]
    pub async fn lsp_declaration(
        &self,
        Parameters(DeclarationParams {
            connection_id,
            document,
            lsp_client_name,
            line,
            character,
        }): Parameters<DeclarationParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let position = Position { line, character };
        let declaration = client
            .lsp_declaration(&lsp_client_name, document, position)
            .await?;
        Ok(CallToolResult::success(vec![Content::json(declaration)?]))
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

    #[tool(description = "Apply workspace edits using Neovim's LSP utility functions")]
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
        client
            .lsp_apply_workspace_edit(&lsp_client_name, workspace_edit)
            .await?;
        Ok(CallToolResult::success(vec![Content::text("success")]))
    }

    #[tool(description = "Rename symbol across workspace using LSP with optional validation")]
    #[instrument(skip(self))]
    pub async fn lsp_rename(
        &self,
        Parameters(RenameParams {
            connection_id,
            document,
            lsp_client_name,
            line,
            character,
            new_name,
            prepare_first,
        }): Parameters<RenameParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let position = Position { line, character };

        // Optionally run prepare rename first to validate the position
        if prepare_first {
            match client
                .lsp_prepare_rename(&lsp_client_name, document.clone(), position.clone())
                .await
            {
                Ok(Some(prepare_result)) => {
                    // Prepare rename was successful, we can proceed
                    let prepare_info = match prepare_result {
                        PrepareRenameResult::Range(range) => {
                            format!("Range: {:?}", range)
                        }
                        PrepareRenameResult::RangeWithPlaceholder { range, placeholder } => {
                            format!("Range: {:?}, Current name: '{}'", range, placeholder)
                        }
                        PrepareRenameResult::DefaultBehavior { .. } => {
                            "Default behavior enabled".to_string()
                        }
                    };
                    tracing::debug!("Prepare rename successful: {}", prepare_info);
                }
                Ok(None) => {
                    return Err(McpError::invalid_request(
                        "Position is not renameable according to prepare rename".to_string(),
                        None,
                    ));
                }
                Err(e) => {
                    return Err(McpError::invalid_request(
                        format!("Prepare rename failed: {}", e),
                        None,
                    ));
                }
            }
        }

        // Proceed with the actual rename
        let workspace_edit = client
            .lsp_rename(&lsp_client_name, document, position, &new_name)
            .await?;

        if let Some(edit) = workspace_edit {
            // Apply the workspace edit automatically
            client
                .lsp_apply_workspace_edit(&lsp_client_name, edit)
                .await?;
            Ok(CallToolResult::success(vec![Content::text(
                "Rename completed successfully",
            )]))
        } else {
            Err(McpError::invalid_request(
                "Rename operation is not valid at this position".to_string(),
                None,
            ))
        }
    }

    #[tool(description = "Format entire document using LSP with optional auto-apply")]
    #[instrument(skip(self))]
    pub async fn lsp_formatting(
        &self,
        Parameters(DocumentFormattingParams {
            connection_id,
            document,
            lsp_client_name,
            options,
            apply_edits,
        }): Parameters<DocumentFormattingParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;
        let text_edits = client
            .lsp_formatting(&lsp_client_name, document.clone(), options)
            .await?;

        if apply_edits {
            // Apply the text edits automatically
            client
                .lsp_apply_text_edits(&lsp_client_name, document, text_edits)
                .await?;
            Ok(CallToolResult::success(vec![Content::text(
                "Formatting applied successfully",
            )]))
        } else {
            // Return the text edits for inspection
            Ok(CallToolResult::success(vec![Content::json(text_edits)?]))
        }
    }

    #[tool(
        description = "Format a specific range in a document using LSP with optional auto-apply"
    )]
    #[instrument(skip(self))]
    pub async fn lsp_range_formatting(
        &self,
        Parameters(DocumentRangeFormattingParams {
            connection_id,
            document,
            lsp_client_name,
            start_line,
            start_character,
            end_line,
            end_character,
            options,
            apply_edits,
        }): Parameters<DocumentRangeFormattingParams>,
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

        let text_edits = client
            .lsp_range_formatting(&lsp_client_name, document.clone(), range, options)
            .await?;

        if apply_edits {
            // Apply the text edits automatically
            client
                .lsp_apply_text_edits(&lsp_client_name, document, text_edits)
                .await?;
            Ok(CallToolResult::success(vec![Content::text(
                "Range formatting applied successfully",
            )]))
        } else {
            // Return the text edits for inspection
            Ok(CallToolResult::success(vec![Content::json(text_edits)?]))
        }
    }

    #[tool(description = "Sort and organize imports")]
    #[instrument(skip(self))]
    pub async fn lsp_organize_imports(
        &self,
        Parameters(LspOrganizeImportsParams {
            connection_id,
            document,
            lsp_client_name,
            apply_edits,
        }): Parameters<LspOrganizeImportsParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_connection(&connection_id)?;

        // Get organize imports code actions for the entire document
        let code_actions = client
            .lsp_get_organize_imports_actions(&lsp_client_name, document)
            .await?;

        if code_actions.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "No organize imports actions available for this document",
            )]));
        }

        if !apply_edits {
            // Return the code actions for inspection
            return Ok(CallToolResult::success(vec![Content::json(code_actions)?]));
        }

        // Apply the first/preferred organize imports action
        let action = code_actions[0].clone();

        // Resolve the action if it needs resolution
        let resolved_action = if action.has_edit() {
            action
        } else {
            client
                .lsp_resolve_code_action(&lsp_client_name, action)
                .await?
        };

        // Apply the workspace edit
        if let Some(edit) = resolved_action.edit() {
            client
                .lsp_apply_workspace_edit(&lsp_client_name, edit.clone())
                .await?;
            Ok(CallToolResult::success(vec![Content::text(
                "Imports organized successfully",
            )]))
        } else {
            Err(McpError::invalid_request(
                "Organize imports action does not contain workspace edit".to_string(),
                None,
            ))
        }
    }
}

/// Build tool router for NeovimMcpServer
pub fn build_tool_router() -> ToolRouter<NeovimMcpServer> {
    NeovimMcpServer::tool_router()
}
