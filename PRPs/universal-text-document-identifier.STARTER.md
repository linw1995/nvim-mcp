# Universal Text Document Identifier System

## Feature

Create a universal text document identifier system that supports multiple ways
of referencing documents: buffer IDs (for open files), project-relative
paths, and absolute paths. This enhances the flexibility of LSP operations
by allowing work with files that aren't necessarily open in Neovim buffers.

Currently, 4 LSP methods depend on `lsp_make_text_document_params` which only
works with buffer IDs:

- `lsp_get_code_actions` - Get LSP code actions for buffer range
- `lsp_hover` - Get hover information for symbol at position
- `lsp_document_symbols` - Get document symbols for buffer
- `lsp_references` - Get references for symbol at position

## Examples

### Universal Document Identifier Enum

```rust
use std::path::PathBuf;

/// Universal identifier for text documents supporting multiple reference types
#[derive(Debug, Clone, PartialEq)]
pub enum DocumentIdentifier {
    /// Reference by Neovim buffer ID (for currently open files)
    BufferId(u64),
    /// Reference by project-relative path
    ProjectRelativePath(PathBuf),
    /// Reference by absolute file path
    AbsolutePath(PathBuf),
}

impl DocumentIdentifier {
    /// Create from buffer ID
    pub fn from_buffer_id(buffer_id: u64) -> Self {
        Self::BufferId(buffer_id)
    }

    /// Create from project-relative path
    pub fn from_project_path<P: Into<PathBuf>>(path: P) -> Self {
        Self::ProjectRelativePath(path.into())
    }

    /// Create from absolute path
    pub fn from_absolute_path<P: Into<PathBuf>>(path: P) -> Self {
        Self::AbsolutePath(path.into())
    }
}
```

### Universal Text Document Identifier Resolver

```rust
impl<T> NeovimClient<T>
where
    T: AsyncWrite + Send + 'static,
{
    /// Universal resolver for converting any DocumentIdentifier to TextDocumentIdentifier
    #[instrument(skip(self))]
    async fn resolve_text_document_identifier(
        &self,
        identifier: &DocumentIdentifier,
    ) -> Result<TextDocumentIdentifier, NeovimError> {
        match identifier {
            DocumentIdentifier::BufferId(buffer_id) => {
                // Use existing buffer-based approach
                self.lsp_make_text_document_params(*buffer_id).await
            }
            DocumentIdentifier::ProjectRelativePath(rel_path) => {
                // Get project root from Neovim
                let project_root = self.get_project_root().await?;
                let absolute_path = project_root.join(rel_path);
                make_text_document_identifier_from_path(absolute_path)
            }
            DocumentIdentifier::AbsolutePath(abs_path) => {
                // Use the new path-based helper function
                make_text_document_identifier_from_path(abs_path)
            }
        }
    }

    /// Get project root directory from Neovim
    async fn get_project_root(&self) -> Result<PathBuf, NeovimError> {
        let conn = self.connection.as_ref().ok_or_else(|| {
            NeovimError::Connection("Not connected to any Neovim instance".to_string())
        })?;

        match conn.nvim
            .execute_lua("return vim.fn.getcwd()", vec![])
            .await
        {
            Ok(value) => {
                let cwd = value.as_str().ok_or_else(|| {
                    NeovimError::Api("Invalid working directory format".to_string())
                })?;
                Ok(PathBuf::from(cwd))
            }
            Err(e) => Err(NeovimError::Api(format!(
                "Failed to get working directory: {e}"
            ))),
        }
    }
}
```

### Updated Method Signatures

```rust
impl<T> NeovimClient<T>
where
    T: AsyncWrite + Send + 'static,
{
    /// Enhanced code actions method supporting universal document identification
    pub async fn lsp_get_code_actions_universal(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
        range: Range,
    ) -> Result<Vec<CodeAction>, NeovimError> {
        let text_document = self.resolve_text_document_identifier(&document).await?;

        let diagnostics = match &document {
            DocumentIdentifier::BufferId(buffer_id) => {
                self.get_buffer_diagnostics(*buffer_id)
                    .await
                    .map_err(|e| {
                        NeovimError::Api(format!(
                            "Failed to get diagnostics: {e}"
                        ))
                    })?
            }
            _ => {
                // For path-based identifiers, diagnostics might not be available
                Vec::new()
            }
        };

        // ... rest of implementation using text_document
    }

    /// Enhanced hover method supporting universal document identification
    pub async fn lsp_hover_universal(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
        position: Position,
    ) -> Result<HoverResult, NeovimError> {
        let text_document = self.resolve_text_document_identifier(&document).await?;

        // ... rest of implementation using text_document
    }

    /// Enhanced document symbols method supporting universal document identification
    pub async fn lsp_document_symbols_universal(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
    ) -> Result<Option<DocumentSymbolResult>, NeovimError> {
        let text_document = self.resolve_text_document_identifier(&document).await?;

        // ... rest of implementation using text_document
    }

    /// Enhanced references method supporting universal document identification
    pub async fn lsp_references_universal(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
        position: Position,
        include_declaration: bool,
    ) -> Result<Vec<Location>, NeovimError> {
        let text_document = self.resolve_text_document_identifier(&document).await?;

        // ... rest of implementation using text_document
    }
}
```

### Backward Compatibility Wrappers

```rust
impl<T> NeovimClient<T>
where
    T: AsyncWrite + Send + 'static,
{
    /// Existing buffer-based code actions method (preserved for backward compatibility)
    pub async fn lsp_get_code_actions(
        &self,
        client_name: &str,
        buffer_id: u64,
        range: Range,
    ) -> Result<Vec<CodeAction>, NeovimError> {
        self.lsp_get_code_actions_universal(
            client_name,
            DocumentIdentifier::from_buffer_id(buffer_id),
            range,
        ).await
    }

    // Similar wrappers for other methods...
}
```

### Usage Examples

```rust
// Using buffer ID (existing pattern)
let actions = client.lsp_get_code_actions_universal(
    "rust-analyzer",
    DocumentIdentifier::from_buffer_id(42),
    range,
).await?;

// Using project-relative path
let symbols = client.lsp_document_symbols_universal(
    "rust-analyzer",
    DocumentIdentifier::from_project_path("src/main.rs"),
).await?;

// Using absolute path
let hover = client.lsp_hover_universal(
    "rust-analyzer",
    DocumentIdentifier::from_absolute_path("/path/to/file.rs"),
    position,
).await?;

// Backward compatibility - existing code continues to work
let actions = client.lsp_get_code_actions("rust-analyzer", 42, range).await?;
```

### Testing the Universal System

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_identifier_creation() {
        let buffer_id = DocumentIdentifier::from_buffer_id(42);
        assert_eq!(buffer_id, DocumentIdentifier::BufferId(42));

        let rel_path = DocumentIdentifier::from_project_path("src/lib.rs");
        assert_eq!(rel_path, DocumentIdentifier::ProjectRelativePath("src/lib.rs".into()));

        let abs_path = DocumentIdentifier::from_absolute_path("/usr/src/lib.rs");
        assert_eq!(abs_path, DocumentIdentifier::AbsolutePath("/usr/src/lib.rs".into()));
    }

    #[tokio::test]
    async fn test_text_document_identifier_resolution() {
        // Test would require a mock Neovim connection
        // Verify that each DocumentIdentifier type resolves to correct TextDocumentIdentifier
    }
}
```

## Documentation

- [LSP TextDocumentIdentifier Specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocumentIdentifier)
  Official LSP protocol documentation for document identifiers
- [Neovim LSP Client Documentation](https://neovim.io/doc/user/lsp.html)
  Neovim's built-in LSP client reference
- Current implementation exists path-based helper function
  [`make_text_document_identifier_from_path`](../src/neovim/client.rs)

## Other Considerations

### Performance Optimizations

- **Caching**: Cache resolved TextDocumentIdentifiers to avoid repeated path
  resolution
- **Path canonicalization**: Only canonicalize paths when necessary to reduce
  I/O overhead
- **Project root detection**: Cache project root to avoid repeated Neovim
  queries

### Error Handling Strategy

- **Graceful degradation**: When diagnostics aren't available for path-based
  identifiers, continue with empty diagnostics
- **Path validation**: Validate paths exist and are readable before
  attempting LSP operations
- **Clear error messages**: Provide specific error messages for each
  identifier type failure

### Backward Compatibility

- **Method preservation**: Keep all existing buffer-based methods as thin
  wrappers
- **API versioning**: Consider this a non-breaking addition to the existing
  API
- **Migration path**: Provide clear migration examples for adopting the new
  system

### Future Extensions

- **URI support**: Add `DocumentIdentifier::Uri(String)` for remote files or
  special schemes
- **Git references**: Add `DocumentIdentifier::GitRef { repo: PathBuf, ref:
String, path: PathBuf }`
- **Virtual documents**: Add support for in-memory or generated documents
- **Workspace awareness**: Enhanced project-relative path resolution with
  multi-root workspace support

### Integration Considerations

- **MCP tool parameters**: Update MCP tool parameter structs to accept
  `DocumentIdentifier` instead of just `buffer_id`
- **JSON serialization**: Implement proper serde traits for
  `DocumentIdentifier` for MCP protocol compatibility
- **Tool documentation**: Update MCP server instructions to document the new
  identifier system
