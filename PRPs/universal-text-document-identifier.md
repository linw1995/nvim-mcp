+++
title: Universal Text Document Identifier System
description:
  Implement a universal text document identifier system that supports multiple ways
  of referencing documents: buffer IDs, project-relative paths, and absolute paths.
  This enhances LSP operations by allowing work with files that aren't necessarily
  open in Neovim buffers.
+++

## Goal

Implement a comprehensive universal text document identifier system that
extends the current buffer-only LSP operations to support project-relative
paths and absolute file paths. The system should maintain backward
compatibility while providing enhanced flexibility for LSP operations on
files that may not be open in Neovim buffers.

## Why

- **Enhanced Flexibility**: Enable LSP operations on files without
  requiring them to be open in Neovim buffers
- **Project-Wide Analysis**: Support analysis of entire codebases, not
  just currently open files
- **Backward Compatibility**: Preserve existing API surface while adding new capabilities
- **Developer Experience**: Provide multiple ways to reference documents
  based on context and need
- **Integration Ready**: Prepare foundation for future MCP tool enhancements

## What

Create a `DocumentIdentifier` enum that supports three types of document
references:

1. `BufferId(u64)` - For currently open files in Neovim
2. `ProjectRelativePath(PathBuf)` - For files relative to project root
3. `AbsolutePath(PathBuf)` - For files with absolute filesystem paths

Enhanced LSP methods will accept `DocumentIdentifier` instead of just
buffer IDs, with automatic resolution to LSP `TextDocumentIdentifier` format.

### Success Criteria

- [ ] `DocumentIdentifier` enum implemented with proper serde support for
      MCP compatibility
- [ ] Universal resolver converts any `DocumentIdentifier` to `TextDocumentIdentifier`
- [ ] All 4 target LSP methods support universal document identification:
  - `lsp_get_code_actions_universal`
  - `lsp_hover_universal`
  - `lsp_document_symbols_universal`
  - `lsp_references_universal`
- [ ] Backward compatibility maintained via wrapper methods
- [ ] MCP tools updated to accept `DocumentIdentifier` in addition to buffer IDs
- [ ] Comprehensive test coverage including integration tests
- [ ] All existing tests continue to pass

## All Needed Context

### Documentation & References

```yaml
# MUST READ - Include these in your context window
- url: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocumentIdentifier
  why: LSP TextDocumentIdentifier specification and URI format
       requirements

- file: src/neovim/client.rs
  why: Current LSP method implementations, lsp_make_text_document_params
       pattern, make_text_document_identifier_from_path helper

- file: src/server/tools.rs
  why: Current MCP tool parameter structures and method signatures to extend

- file: src/neovim/integration_tests.rs
  why: Test patterns for LSP methods and connection setup

- file: src/server/integration_tests.rs
  why: MCP server testing patterns and tool validation approaches

- file: /Users/linw1995/Documents/opensources/linw1995/nvim-mcp/CLAUDE.md
  why: Build commands, test commands, and project architecture patterns
```

### Current Codebase Structure (Relevant Files)

```bash
src/
├── neovim/
│   ├── client.rs              # Core LSP implementations, existing helper functions
│   ├── integration_tests.rs   # LSP method test patterns
│   └── mod.rs
├── server/
│   ├── tools.rs              # MCP tool implementations and parameter structs
│   ├── integration_tests.rs  # MCP server testing patterns
│   └── mod.rs
└── lib.rs
```

### Files to be Modified/Enhanced

```bash
src/neovim/client.rs:
  - ADD DocumentIdentifier enum
  - ADD resolve_text_document_identifier method
  - ADD get_project_root method
  - ADD *_universal LSP methods (4 methods)
  - PRESERVE existing buffer-based methods as wrappers

src/server/tools.rs:
  - ADD DocumentIdentifierParam struct with serde support
  - ADD universal MCP tool variants
  - PRESERVE existing tools for backward compatibility

src/neovim/integration_tests.rs:
  - ADD tests for DocumentIdentifier variants
  - ADD tests for universal LSP methods
  - ADD path-based operation tests

src/server/integration_tests.rs:
  - ADD MCP tool tests for universal document identification
  - VERIFY backward compatibility
```

### Known Gotchas & Library Quirks

```rust
// CRITICAL: LSP TextDocumentIdentifier requires file:// URI format
// Current make_text_document_identifier_from_path() at line 767-784
// handles this correctly

// CRITICAL: Path canonicalization can fail - handle errors gracefully
// Pattern from existing code:
//   path.canonicalize().map_err(|e| NeovimError::Api(
//     format!("Failed to resolve path: {}", e)))

// CRITICAL: Project root detection uses vim.fn.getcwd() - cache result
// to avoid repeated calls
// Pattern from existing code at lines 906-937 shows proper Lua execution
// error handling

// CRITICAL: MCP tools require serde::Deserialize and schemars::JsonSchema derives
// Pattern from existing parameter structs in tools.rs lines 44-113

// CRITICAL: For path-based identifiers, diagnostics may not be available
// Handle gracefully with Vec::new() fallback as shown in PRP examples

// CRITICAL: Maintain #[instrument(skip(self))] for tracing on all methods
// Pattern consistent throughout existing codebase
```

## Implementation Blueprint

### Data Models and Structure

Core enum and supporting types for universal document identification:

```rust
// In src/neovim/client.rs - add near existing TextDocumentIdentifier (line ~170)

/// Universal identifier for text documents supporting multiple reference types
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", content = "value")]
pub enum DocumentIdentifier {
    /// Reference by Neovim buffer ID (for currently open files)
    #[serde(rename = "buffer_id")]
    BufferId(u64),
    /// Reference by project-relative path
    #[serde(rename = "project_path")]
    ProjectRelativePath(PathBuf),
    /// Reference by absolute file path
    #[serde(rename = "absolute_path")]
    AbsolutePath(PathBuf),
}

impl DocumentIdentifier {
    pub fn from_buffer_id(buffer_id: u64) -> Self { ... }
    pub fn from_project_path<P: Into<PathBuf>>(path: P) -> Self { ... }
    pub fn from_absolute_path<P: Into<PathBuf>>(path: P) -> Self { ... }
}
```

### List of Tasks to be Completed

Tasks to fulfill the PRP in the order they should be completed:

```yaml
Task 1 - Core DocumentIdentifier Implementation:
MODIFY src/neovim/client.rs:
  - FIND: Line ~170 near TextDocumentIdentifier struct
  - ADD: DocumentIdentifier enum with full serde support
  - ADD: Constructor methods (from_buffer_id, from_project_path, from_absolute_path)
  - PATTERN: Follow existing struct patterns in the file

Task 2 - Universal Resolver Implementation:
MODIFY src/neovim/client.rs:
  - FIND: Line ~856 in NeovimClient<T> impl block
  - ADD: resolve_text_document_identifier method
  - ADD: get_project_root method with caching
  - PATTERN: Mirror lsp_make_text_document_params structure (lines 906-937)
  - USE: Existing make_text_document_identifier_from_path helper (line 767-784)

Task 3 - Universal LSP Methods:
MODIFY src/neovim/client.rs:
  - FIND: Line ~939 after existing lsp_get_code_actions method
  - ADD: lsp_get_code_actions_universal method
  - ADD: lsp_hover_universal method
  - ADD: lsp_document_symbols_universal method
  - ADD: lsp_references_universal method
  - PATTERN: Mirror existing method signatures but accept DocumentIdentifier
  - PRESERVE: Error handling patterns from existing methods

Task 4 - Backward Compatibility Wrappers:
MODIFY src/neovim/client.rs:
  - FIND: NeovimClientTrait impl block around line 1007
  - MODIFY: Existing trait methods to call universal variants
  - PATTERN: Simple wrapper that creates DocumentIdentifier::BufferId(buffer_id)
  - PRESERVE: Exact same method signatures and return types

Task 5 - MCP Parameter Structs:
MODIFY src/server/tools.rs:
  - FIND: Line ~44 near existing parameter structs
  - ADD: DocumentIdentifierParam struct with serde support
  - ADD: Universal parameter variants for each LSP tool
  - PATTERN: Follow BufferLSPConnectionParams structure
  - CRITICAL: Include serde::Deserialize and schemars::JsonSchema derives

Task 6 - Universal MCP Tools:
MODIFY src/server/tools.rs:
  - FIND: Line ~278 after existing buffer_code_actions
  - ADD: universal_code_actions MCP tool method
  - ADD: universal_hover MCP tool method
  - ADD: universal_document_symbols MCP tool method
  - ADD: universal_references MCP tool method
  - PATTERN: Mirror existing tool structure with #[tool] attribute
  - USE: #[instrument(skip(self))] for all methods

Task 7 - Unit Tests:
CREATE tests for DocumentIdentifier in src/neovim/client.rs:
  - ADD: test_document_identifier_creation()
  - ADD: test_text_document_identifier_resolution()
  - PATTERN: Follow existing test structure at end of file (lines 1404+)
  - USE: Existing test utilities from test_utils module

Task 8 - Integration Tests:
MODIFY src/neovim/integration_tests.rs:
  - FIND: Line ~200 after existing LSP tests
  - ADD: test_universal_lsp_methods_with_paths()
  - ADD: test_project_relative_path_resolution()
  - PATTERN: Follow test_get_vim_diagnostics structure (lines 172+)
  - USE: Existing setup_connected_client_ipc pattern

Task 9 - MCP Integration Tests:
MODIFY src/server/integration_tests.rs:
  - FIND: Line ~100 after list_tools test
  - ADD: test_universal_document_tools()
  - ADD: test_backward_compatibility_maintained()
  - PATTERN: Follow existing MCP tool testing approach
  - USE: extract_connection_id helper function
```

### Per Task Pseudocode

```rust
// Task 2 - Universal Resolver (CRITICAL implementation details)
#[instrument(skip(self))]
async fn resolve_text_document_identifier(
    &self,
    identifier: &DocumentIdentifier,
) -> Result<TextDocumentIdentifier, NeovimError> {
    match identifier {
        DocumentIdentifier::BufferId(buffer_id) => {
            // PATTERN: Use existing buffer-based approach
            self.lsp_make_text_document_params(*buffer_id).await
        }
        DocumentIdentifier::ProjectRelativePath(rel_path) => {
            // GOTCHA: Cache project root to avoid repeated Neovim queries
            let project_root = self.get_project_root().await?;
            let absolute_path = project_root.join(rel_path);
            // PATTERN: Use existing path helper function
            make_text_document_identifier_from_path(absolute_path)
        }
        DocumentIdentifier::AbsolutePath(abs_path) => {
            // PATTERN: Direct conversion using existing helper
            make_text_document_identifier_from_path(abs_path)
        }
    }
}

// Task 3 - Universal LSP Method (example pattern)
#[instrument(skip(self))]
pub async fn lsp_get_code_actions_universal(
    &self,
    client_name: &str,
    document: DocumentIdentifier,
    range: Range,
) -> Result<Vec<CodeAction>, NeovimError> {
    // PATTERN: Resolve document first
    let text_document = self.resolve_text_document_identifier(&document).await?;

    // GOTCHA: Diagnostics may not be available for path-based identifiers
    let diagnostics = match &document {
        DocumentIdentifier::BufferId(buffer_id) => {
            self.get_buffer_diagnostics(*buffer_id).await.unwrap_or_default()
        }
        _ => Vec::new() // PATTERN: Graceful degradation
    };

    // PATTERN: Use existing Lua execution approach
    // ... rest follows existing lsp_get_code_actions implementation
}
```

### Integration Points

```yaml
NEOVIM_CLIENT_TRAIT:
  - modify: All 4 LSP trait methods to delegate to universal variants
  - preserve: Exact same signatures for backward compatibility

MCP_TOOLS:
  - add: 4 new universal tool methods with #[tool] attribute
  - preserve: Existing tools unchanged for backward compatibility
  - pattern: "universal_" prefix for new tool names

SERDE_INTEGRATION:
  - add: Full serde support for DocumentIdentifier enum
  - add: JsonSchema derive for MCP compatibility
  - pattern: Tagged enum serialization with type/value structure

ERROR_HANDLING:
  - preserve: Existing NeovimError patterns
  - add: Path resolution error handling
  - pattern: Graceful degradation for unavailable diagnostics
```

## Validation Loop

### Level 1: Syntax & Style

```bash
# Run these FIRST - fix any errors before proceeding
cargo check                          # Basic compilation check
cargo clippy -- -D warnings   # Lint with warnings as errors
cargo fmt --check                   # Format verification

# Expected: No errors. If errors, READ the error and fix.
```

### Level 2: Unit Tests

```bash
# Test core functionality incrementally
cargo test -- --show-output \\
  client::tests::test_document_identifier_creation
cargo test -- --show-output \\
  client::tests::test_text_document_identifier_resolution
cargo test -- --show-output client::tests::test_make_text_document_identifier_from_path

# Expected: All new unit tests pass
# If failing: Read error, understand root cause, fix implementation, re-run
```

### Level 3: Integration Tests

```bash
# Test LSP integration with real Neovim instances
cargo test -- --show-output neovim::integration_tests::test_universal_lsp_methods_with_paths
cargo test -- --show-output neovim::integration_tests::test_project_relative_path_resolution

# Test MCP server integration
cargo test -- --show-output server::integration_tests::test_universal_document_tools
cargo test -- --show-output server::integration_tests::test_backward_compatibility_maintained

# Expected: All integration tests pass
# If error: Check logs and connection setup, verify Neovim LSP configuration
```

### Level 4: Backward Compatibility Verification

```bash
# CRITICAL: Ensure all existing tests still pass
cargo test -- --show-output neovim::integration_tests
cargo test -- --show-output server::integration_tests

# Expected: No regressions in existing functionality
# If failing: Fix universal method implementations to maintain compatibility
```

## Final Validation Checklist

- [ ] All unit tests pass: `cargo test -- --show-output client::tests`
- [ ] All integration tests pass:
      `cargo test -- --show-output neovim::integration_tests \\`
      `server::integration_tests`
- [ ] No compilation errors: `cargo check`
- [ ] No linting warnings: `cargo clippy -- -D warnings`
- [ ] Code formatted: `cargo fmt --check`
- [ ] Backward compatibility maintained: All existing tests pass
- [ ] MCP tool compatibility: Universal tools accept DocumentIdentifier JSON
- [ ] LSP operations work with paths: Can perform operations on non-open files
- [ ] Error handling graceful: Invalid paths return proper error messages
- [ ] Documentation updated: Method signatures and examples reflect new capabilities

---

## Anti-Patterns to Avoid

- ❌ Don't change existing method signatures - use wrapper pattern instead
- ❌ Don't skip path validation - canonicalize and handle errors properly
- ❌ Don't ignore diagnostics availability - provide graceful fallbacks
- ❌ Don't hardcode project root detection - use Neovim's getcwd()
- ❌ Don't break MCP protocol compatibility - maintain proper serde derives
- ❌ Don't skip backward compatibility tests - ensure no regressions
- ❌ Don't assume paths exist - validate and return meaningful error messages

---

## PRP Confidence Score: 9/10

This PRP provides comprehensive context from existing codebase patterns,
detailed implementation steps, and robust validation procedures. The
incremental approach with validation loops should enable successful one-pass
implementation with iterative refinement capability.
