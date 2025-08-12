#![allow(rustdoc::invalid_codeblock_attributes)]

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use nvim_rs::{Handler, Neovim, create::tokio as create};
use rmpv::Value;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use tokio::{io::AsyncWrite, net::TcpStream};
use tracing::{debug, info, instrument};

use super::{connection::NeovimConnection, error::NeovimError};

/// Common trait for Neovim client operations
#[async_trait]
pub trait NeovimClientTrait: Sync {
    /// Get the target of the Neovim connection
    fn target(&self) -> Option<String>;

    /// Disconnect from the current Neovim instance
    async fn disconnect(&mut self) -> Result<String, NeovimError>;

    /// Get information about all buffers
    async fn get_buffers(&self) -> Result<Vec<BufferInfo>, NeovimError>;

    /// Execute Lua code in Neovim
    async fn execute_lua(&self, code: &str) -> Result<Value, NeovimError>;

    /// Set up diagnostics changed autocmd
    async fn setup_diagnostics_changed_autocmd(&self) -> Result<(), NeovimError>;

    /// Get diagnostics for a specific buffer
    async fn get_buffer_diagnostics(&self, buffer_id: u64) -> Result<Vec<Diagnostic>, NeovimError>;

    /// Get diagnostics for the entire workspace
    async fn get_workspace_diagnostics(&self) -> Result<Vec<Diagnostic>, NeovimError>;

    /// Get LSP clients
    async fn lsp_get_clients(&self) -> Result<Vec<LspClient>, NeovimError>;

    /// Get LSP code actions
    async fn lsp_get_code_actions(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
        range: Range,
    ) -> Result<Vec<CodeAction>, NeovimError>;

    /// Get LSP hover information for a specific position
    async fn lsp_hover(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
        position: Position,
    ) -> Result<HoverResult, NeovimError>;

    /// Get document symbols for a specific buffer or document
    async fn lsp_document_symbols(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
    ) -> Result<Option<DocumentSymbolResult>, NeovimError>;

    /// Search for workspace symbols by query
    async fn lsp_workspace_symbols(
        &self,
        client_name: &str,
        query: &str,
    ) -> Result<WorkspaceSymbolResult, NeovimError>;

    /// Get references for a symbol at a specific position
    async fn lsp_references(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
        position: Position,
        include_declaration: bool,
    ) -> Result<Vec<Location>, NeovimError>;
}

pub struct NeovimHandler<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T> NeovimHandler<T> {
    pub fn new() -> Self {
        NeovimHandler {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> Clone for NeovimHandler<T> {
    fn clone(&self) -> Self {
        NeovimHandler {
            _marker: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<T> Handler for NeovimHandler<T>
where
    T: futures::AsyncWrite + Send + Sync + Unpin + 'static,
{
    type Writer = T;

    async fn handle_notify(&self, name: String, args: Vec<Value>, _neovim: Neovim<T>) {
        info!("handling notification: {name:?}, {args:?}");
    }

    async fn handle_request(
        &self,
        name: String,
        args: Vec<Value>,
        _neovim: Neovim<T>,
    ) -> Result<Value, Value> {
        info!("handling request: {name:?}, {args:?}");
        match name.as_ref() {
            "ping" => Ok(Value::from("pong")),
            _ => Ok(Value::Nil),
        }
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Diagnostic {
    pub message: String,
    pub code: Option<String>,
    pub severity: u8,
    pub lnum: u64,
    pub col: u64,
    pub source: String,
    pub bufnr: u64,
    pub end_lnum: u64,
    pub end_col: u64,
    pub namespace: u64,
    pub user_data: Option<UserData>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct UserData {
    pub lsp: LSPDiagnostic,
    #[serde(flatten)]
    pub unknowns: HashMap<String, serde_json::Value>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LSPDiagnostic {
    pub code: Option<String>,
    pub message: String,
    pub range: Range,
    pub severity: u8,
    pub source: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LspClient {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BufferInfo {
    pub id: u64,
    pub name: String,
    pub line_count: u64,
}

/// Text documents are identified using a URI.
/// On the protocol level, URIs are passed as strings.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct TextDocumentIdentifier {
    /// The text document's URI.
    uri: String,
    /// The version number of this document. If an optional versioned text document
    /// identifier is sent from the server to the client and the file is not
    /// open in the editor (the server has not received an open notification
    /// before) the server can send `null` to indicate that the version is
    /// known and the content on disk is the master (as specified with document
    /// content ownership).
    ///
    /// The version number of a document will increase after each change,
    /// including undo/redo. The number doesn't need to be consecutive.
    version: Option<i32>,
}

/// Universal identifier for text documents supporting multiple reference types
#[derive(Debug, Clone, PartialEq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentIdentifier {
    /// Reference by Neovim buffer ID (for currently open files)
    BufferId(u64),
    /// Reference by project-relative path
    ProjectRelativePath(PathBuf),
    /// Reference by absolute file path
    AbsolutePath(PathBuf),
}

/// Supports both string and struct deserialization for DocumentIdentifier.
/// Compatible with Claude Code when using subscription.
struct DocumentIdentifierVisitor;

impl<'de> Visitor<'de> for DocumentIdentifierVisitor {
    type Value = DocumentIdentifier;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("string or object")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::from_str(value).map_err(de::Error::custom)
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "buffer_id" => {
                    let value: u64 = map.next_value()?;
                    return Ok(DocumentIdentifier::BufferId(value));
                }
                "project_relative_path" => {
                    let value: String = map.next_value()?;
                    return Ok(DocumentIdentifier::ProjectRelativePath(PathBuf::from(
                        value,
                    )));
                }
                "absolute_path" => {
                    let value: String = map.next_value()?;
                    return Ok(DocumentIdentifier::AbsolutePath(PathBuf::from(value)));
                }
                _ => {
                    // Skip unknown keys to be forward compatible
                    let _: serde_json::Value = map.next_value()?;
                }
            }
        }
        Err(de::Error::custom(
            "Expected one of: buffer_id, project_relative_path, or absolute_path",
        ))
    }
}

impl<'de> Deserialize<'de> for DocumentIdentifier {
    fn deserialize<D>(deserializer: D) -> Result<DocumentIdentifier, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DocumentIdentifierVisitor)
    }
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

/// Position in a text document expressed as zero-based line and zero-based character offset.
/// A position is between two characters like an 'insert' cursor in an editor.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Position {
    /// Line position in a document (zero-based).
    pub line: u64,
    /// Character offset on a line in a document (zero-based). The meaning of this
    /// offset is determined by the negotiated `PositionEncodingKind`.
    ///
    /// If the character value is greater than the line length it defaults back
    /// to the line length.
    pub character: u64,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Range {
    /// The range's start position.
    pub start: Position,
    /// The range's end position.
    pub end: Position,
}

/// The kind of a code action.
///
/// Kinds are a hierarchical list of identifiers separated by `.`,
/// e.g. `"refactor.extract.function"`.
///
/// The set of kinds is open and client needs to announce the kinds it supports
/// to the server during initialization.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CodeActionKind {
    /// Empty kind.
    #[serde(rename = "")]
    Empty,
    /// Base kind for quickfix actions: 'quickfix'.
    #[serde(rename = "quickfix")]
    Quickfix,
    /// Base kind for refactoring actions: 'refactor'.
    Refactor,
    /// Base kind for refactoring extraction actions: 'refactor.extract'.
    ///
    /// Example extract actions:
    ///
    /// - Extract method
    /// - Extract function
    /// - Extract variable
    /// - Extract interface from class
    /// - ...
    #[serde(rename = "refactor.extract")]
    RefactorExtract,
    /// Base kind for refactoring inline actions: 'refactor.inline'.
    ///
    /// Example inline actions:
    ///
    /// - Inline function
    /// - Inline variable
    /// - Inline constant
    /// - ...
    #[serde(rename = "refactor.inline")]
    RefactorInline,
    /// Base kind for refactoring rewrite actions: 'refactor.rewrite'.
    ///
    /// Example rewrite actions:
    ///
    /// - Convert JavaScript function to class
    /// - Add or remove parameter
    /// - Encapsulate field
    /// - Make method static
    /// - Move method to base class
    /// - ...
    #[serde(rename = "refactor.rewrite")]
    RefactorRewrite,
    /// Base kind for source actions: `source`.
    ///
    /// Source code actions apply to the entire file.
    Source,
    /// Base kind for an organize imports source action:
    /// `source.organizeImports`.
    #[serde(rename = "source.organizeImports")]
    SourceOrganizeImports,
    /// Base kind for a 'fix all' source action: `source.fixAll`.
    ///
    /// 'Fix all' actions automatically fix errors that have a clear fix that
    /// do not require user input. They should not suppress errors or perform
    /// unsafe fixes such as generating new types or classes.
    ///
    /// @since 3.17.0
    #[serde(rename = "source.fixAll")]
    SourceFixAll,
}

/// The reason why code actions were requested.
///
/// @since 3.17.0
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub enum CodeActionTriggerKind {
    /// Code actions were explicitly requested by the user or by an extension.
    Invoked = 1,
    /// Code actions were requested automatically.
    ///
    /// This typically happens when current selection in a file changes, but can
    /// also be triggered when file content changes.
    Automatic = 2,
}
/// Contains additional diagnostic information about the context in which
/// a code action is run.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeActionContext {
    /// Requested kind of actions to return.
    ///
    /// Actions not of this kind are filtered out by the client before being
    /// shown. So servers can omit computing them.
    only: Option<Vec<CodeActionKind>>,
    /// An array of diagnostics known on the client side overlapping the range
    /// provided to the `textDocument/codeAction` request. They are provided so
    /// that the server knows which errors are currently presented to the user
    /// for the given range. There is no guarantee that these accurately reflect
    /// the error state of the resource. The primary parameter
    /// to compute code actions is the provided range.
    diagnostics: Vec<LSPDiagnostic>,
    /// The reason why code actions were requested.
    ///
    /// @since 3.17.0
    trigger_kind: Option<CodeActionTriggerKind>,
}

/// Params for the CodeActionRequest
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeActionParams {
    /// The document in which the command was invoked.
    pub text_document: TextDocumentIdentifier,
    /// The range for which the command was invoked.
    pub range: Range,
    /// Context carrying additional information.
    pub context: CodeActionContext,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Disabled {
    /// Human readable description of why the code action is currently
    /// disabled.
    ///
    /// This is displayed in the code actions UI.
    reason: String,
}

/// A textual edit applicable to a text document.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEdit {
    /// The range of the text document to be manipulated. To insert
    /// text into a document create a range where start === end.
    range: Range,
    /// The string to be inserted. For delete operations use an
    /// empty string.
    new_text: String,
    /// The actual annotation identifier.
    annotation_id: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEdit {
    /// Holds changes to existing resources.
    changes: Option<std::collections::HashMap<String, Vec<TextEdit>>>,

    /// Depending on the client capability
    /// `workspace.workspaceEdit.resourceOperations` document changes are either
    /// an array of `TextDocumentEdit`s to express changes to n different text
    /// documents where each text document edit addresses a specific version of
    /// a text document. Or it can contain above `TextDocumentEdit`s mixed with
    /// create, rename and delete file / folder operations.
    ///
    /// Whether a client supports versioned document edits is expressed via
    /// `workspace.workspaceEdit.documentChanges` client capability.
    ///
    /// If a client neither supports `documentChanges` nor
    /// `workspace.workspaceEdit.resourceOperations` then only plain `TextEdit`s
    /// using the `changes` property are supported.
    document_changes: Option<Vec<serde_json::Value>>,
    /// A map of change annotations that can be referenced in
    /// `AnnotatedTextEdit`s or create, rename and delete file / folder
    /// operations.
    ///
    /// Whether clients honor this property depends on the client capability
    /// `workspace.changeAnnotationSupport`.
    ///
    /// @since 3.16.0
    change_annotations: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Command {
    /// Title of the command, like `save`.
    title: String,
    /// The identifier of the actual command handler.
    command: String,
    /// Arguments that the command handler should be
    /// invoked with.
    arguments: Vec<serde_json::Value>,
}

/// A code action represents a change that can be performed in code, e.g. to fix
/// a problem or to refactor code.
///
/// A CodeAction must set either `edit` and/or a `command`. If both are supplied,
/// the `edit` is applied first, then the `command` is executed.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeAction {
    /// A short, human-readable, title for this code action.
    title: String,

    /// The kind of the code action.
    ///
    /// Used to filter code actions.
    kind: Option<CodeActionKind>,

    /// The diagnostics that this code action resolves.
    diagnostics: Option<Vec<LSPDiagnostic>>,
    /// Marks this as a preferred action. Preferred actions are used by the
    /// `auto fix` command and can be targeted by keybindings.
    ///
    /// A quick fix should be marked preferred if it properly addresses the
    /// underlying error. A refactoring should be marked preferred if it is the
    /// most reasonable choice of actions to take.
    ///
    /// @since 3.15.0
    is_preferred: Option<bool>,
    /// Marks that the code action cannot currently be applied.
    ///
    /// Clients should follow the following guidelines regarding disabled code
    /// actions:
    ///
    /// - Disabled code actions are not shown in automatic lightbulbs code
    ///   action menus.
    ///
    /// - Disabled actions are shown as faded out in the code action menu when
    ///   the user request a more specific type of code action, such as
    ///   refactorings.
    ///
    /// - If the user has a keybinding that auto applies a code action and only
    ///   a disabled code actions are returned, the client should show the user
    ///   an error message with `reason` in the editor.
    ///
    /// @since 3.16.0
    disabled: Option<Disabled>,

    /// The workspace edit this code action performs.
    edit: Option<WorkspaceEdit>,

    /// A command this code action executes. If a code action
    /// provides an edit and a command, first the edit is
    /// executed and then the command.
    command: Option<Command>,

    /// A data entry field that is preserved on a code action between
    /// a `textDocument/codeAction` and a `codeAction/resolve` request.
    ///
    /// @since 3.16.0
    data: Option<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HoverParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    pub context: ReferenceContext,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceContext {
    /// Include the declaration of the current symbol.
    pub include_declaration: bool,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct HoverResult {
    /// The hover's content
    pub contents: HoverContents,
    /// An optional range is a range inside a text document
    /// that is used to visualize a hover, e.g. by changing the background color.
    pub range: Option<Range>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum HoverContents {
    String(MarkedString),
    Strings(Vec<MarkedString>),
    Content(MarkupContent),
}

/// MarkedString can be used to render human readable text. It is either a
/// markdown string or a code-block that provides a language and a code snippet.
/// The language identifier is semantically equal to the optional language
/// identifier in fenced code blocks in GitHub issues.
///
/// The pair of a language and a value is an equivalent to markdown:
/// ```${language}
/// ${value}
/// ```
///
/// Note that markdown strings will be sanitized - that means html will be
/// escaped.
///
/// @deprecated use MarkupContent instead.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub enum MarkedString {
    String(String),
    Markup { lang: String, value: String },
}

/// A `MarkupContent` literal represents a string value which content is
/// interpreted base on its kind flag. Currently the protocol supports
/// `plaintext` and `markdown` as markup kinds.
///
/// If the kind is `markdown` then the value can contain fenced code blocks like
/// in GitHub issues.
///
/// *Please Note* that clients might sanitize the return markdown. A client could
/// decide to remove HTML from the markdown to avoid script execution.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct MarkupContent {
    /// The type of the Markup
    pub kind: MarkupKind,
    /// The content itself
    pub value: String,
}

/// Describes the content type that a client supports in various
/// result literals like `Hover`, `ParameterInfo` or `CompletionItem`.
///
/// Please note that `MarkupKinds` must not start with a `$`. This kinds
/// are reserved for internal usage.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub enum MarkupKind {
    /// Plain text is supported as a content format
    #[serde(rename = "plaintext")]
    PlainText,
    /// Markdown is supported as a content format
    #[serde(rename = "markdown")]
    Markdown,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct CodeActionResult {
    #[serde(default)]
    pub result: Vec<CodeAction>,
}

/// A symbol kind.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(into = "u8", from = "u8")]
pub enum SymbolKind {
    File = 1,
    Module = 2,
    Namespace = 3,
    Package = 4,
    Class = 5,
    Method = 6,
    Property = 7,
    Field = 8,
    Constructor = 9,
    Enum = 10,
    Interface = 11,
    Function = 12,
    Variable = 13,
    Constant = 14,
    String = 15,
    Number = 16,
    Boolean = 17,
    Array = 18,
    Object = 19,
    Key = 20,
    Null = 21,
    EnumMember = 22,
    Struct = 23,
    Event = 24,
    Operator = 25,
    TypeParameter = 26,
}

impl From<SymbolKind> for u8 {
    fn from(kind: SymbolKind) -> u8 {
        kind as u8
    }
}

impl From<u8> for SymbolKind {
    fn from(value: u8) -> SymbolKind {
        match value {
            1 => SymbolKind::File,
            2 => SymbolKind::Module,
            3 => SymbolKind::Namespace,
            4 => SymbolKind::Package,
            5 => SymbolKind::Class,
            6 => SymbolKind::Method,
            7 => SymbolKind::Property,
            8 => SymbolKind::Field,
            9 => SymbolKind::Constructor,
            10 => SymbolKind::Enum,
            11 => SymbolKind::Interface,
            12 => SymbolKind::Function,
            13 => SymbolKind::Variable,
            14 => SymbolKind::Constant,
            15 => SymbolKind::String,
            16 => SymbolKind::Number,
            17 => SymbolKind::Boolean,
            18 => SymbolKind::Array,
            19 => SymbolKind::Object,
            20 => SymbolKind::Key,
            21 => SymbolKind::Null,
            22 => SymbolKind::EnumMember,
            23 => SymbolKind::Struct,
            24 => SymbolKind::Event,
            25 => SymbolKind::Operator,
            26 => SymbolKind::TypeParameter,
            _ => SymbolKind::Variable, // Default fallback
        }
    }
}

/// Symbol tags are extra annotations that tweak the rendering of a symbol.
///
/// @since 3.16
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(into = "u8", from = "u8")]
pub enum SymbolTag {
    /// Render a symbol as obsolete, usually using a strike-out.
    Deprecated = 1,
}

impl From<SymbolTag> for u8 {
    fn from(tag: SymbolTag) -> u8 {
        tag as u8
    }
}

impl From<u8> for SymbolTag {
    fn from(value: u8) -> SymbolTag {
        match value {
            1 => SymbolTag::Deprecated,
            _ => SymbolTag::Deprecated, // Default fallback
        }
    }
}

/// Represents a location inside a resource, such as a line inside a text file.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

/// Represents information about programming constructs like variables, classes, interfaces etc.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInformation {
    /// The name of this symbol.
    pub name: String,
    /// The kind of this symbol.
    pub kind: SymbolKind,
    /// Tags for this symbol.
    ///
    /// @since 3.16.0
    pub tags: Option<Vec<SymbolTag>>,
    /// Indicates if this symbol is deprecated.
    ///
    /// @deprecated Use tags instead
    pub deprecated: Option<bool>,
    /// The location of this symbol. The location's range is used by a tool
    /// to reveal the location in the editor. If the symbol is selected in the
    /// tool the range's start information is used to position the cursor. So
    /// the range usually spans more than the actual symbol's name and does
    /// normally include things like visibility modifiers.
    ///
    /// The range doesn't have to denote a node range in the sense of an abstract
    /// syntax tree. It can therefore not be used to re-construct a hierarchy of
    /// the symbols.
    pub location: Location,
    /// The name of the symbol containing this symbol. This information is for
    /// user interface purposes (e.g. to render a qualifier in the user interface
    /// if necessary). It can't be used to re-infer a hierarchy for the document
    /// symbols.
    pub container_name: Option<String>,
}

/// Represents programming constructs like variables, classes, interfaces etc. that appear in a document.
/// Document symbols can be hierarchical and they have two ranges: one that encloses its definition and
/// one that points to its most interesting range, e.g. the range of an identifier.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSymbol {
    /// The name of this symbol. Will be displayed in the user interface and therefore must not be
    /// an empty string or a string only consisting of white spaces.
    pub name: String,
    /// More detail for this symbol, e.g the signature of a function.
    pub detail: Option<String>,
    /// The kind of this symbol.
    pub kind: SymbolKind,
    /// Tags for this symbol.
    ///
    /// @since 3.16.0
    pub tags: Option<Vec<SymbolTag>>,
    /// Indicates if this symbol is deprecated.
    ///
    /// @deprecated Use tags instead
    pub deprecated: Option<bool>,
    /// The range enclosing this symbol not including leading/trailing whitespace but everything else
    /// like comments. This information is typically used to determine if the clients cursor is
    /// inside the symbol to reveal in the symbol in the UI.
    pub range: Range,
    /// The range that should be selected and revealed when this symbol is being picked, e.g the name of a function.
    /// Must be contained by the `range`.
    pub selection_range: Range,
    /// Children of this symbol, e.g. properties of a class.
    pub children: Option<Vec<DocumentSymbol>>,
}

/// Parameters for document symbol request
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSymbolParams {
    /// The text document.
    pub text_document: TextDocumentIdentifier,
}

/// Parameters for workspace symbol request
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct WorkspaceSymbolParams {
    /// A query string to filter symbols by. Clients may send an empty
    /// string here to request all symbols.
    pub query: String,
}

/// Result type for document symbols request
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum DocumentSymbolResult {
    Symbols(Vec<DocumentSymbol>),
    Information(Vec<SymbolInformation>),
}

/// Result type for workspace symbols request
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct WorkspaceSymbolResult {
    pub result: Option<DocumentSymbolResult>,
    #[serde(flatten)]
    pub unknowns: HashMap<String, serde_json::Value>,
}

pub struct NeovimClient<T>
where
    T: AsyncWrite + Send + 'static,
{
    connection: Option<NeovimConnection<T>>,
}

#[cfg(unix)]
type Connection = tokio::net::UnixStream;
#[cfg(windows)]
type Connection = tokio::net::windows::named_pipe::NamedPipeClient;

/// Creates a TextDocumentIdentifier from a file path
/// This utility function works independently of Neovim buffers
#[allow(dead_code)]
pub fn make_text_document_identifier_from_path<P: AsRef<Path>>(
    file_path: P,
) -> Result<TextDocumentIdentifier, NeovimError> {
    let path = file_path.as_ref();

    // Convert to absolute path and canonicalize
    let absolute_path = path.canonicalize().map_err(|e| {
        NeovimError::Api(format!("Failed to resolve path {}: {}", path.display(), e))
    })?;

    // Convert to file:// URI
    let uri = format!("file://{}", absolute_path.display());

    Ok(TextDocumentIdentifier {
        uri,
        version: None, // No version for path-based identifiers
    })
}

impl NeovimClient<Connection> {
    #[instrument(skip(self))]
    pub async fn connect_path(&mut self, path: &str) -> Result<(), NeovimError> {
        if self.connection.is_some() {
            return Err(NeovimError::Connection(format!(
                "Already connected to {}. Disconnect first.",
                self.connection.as_ref().unwrap().target()
            )));
        }

        debug!("Attempting to connect to Neovim at {}", path);
        let handler = NeovimHandler::new();
        match create::new_path(path, handler).await {
            Ok((nvim, io_handler)) => {
                let connection = NeovimConnection::new(
                    nvim,
                    tokio::spawn(async move {
                        let rv = io_handler.await;
                        info!("io_handler completed with result: {:?}", rv);
                        rv
                    }),
                    path.to_string(),
                );
                self.connection = Some(connection);
                debug!("Successfully connected to Neovim at {}", path);
                Ok(())
            }
            Err(e) => {
                debug!("Failed to connect to Neovim at {}: {}", path, e);
                Err(NeovimError::Connection(format!("Connection failed: {e}")))
            }
        }
    }
}

impl NeovimClient<TcpStream> {
    #[instrument(skip(self))]
    pub async fn connect_tcp(&mut self, address: &str) -> Result<(), NeovimError> {
        if self.connection.is_some() {
            return Err(NeovimError::Connection(format!(
                "Already connected to {}. Disconnect first.",
                self.connection.as_ref().unwrap().target()
            )));
        }

        debug!("Attempting to connect to Neovim at {}", address);
        let handler = NeovimHandler::new();
        match create::new_tcp(address, handler).await {
            Ok((nvim, io_handler)) => {
                let connection = NeovimConnection::new(
                    nvim,
                    tokio::spawn(async move {
                        let rv = io_handler.await;
                        info!("io_handler completed with result: {:?}", rv);
                        rv
                    }),
                    address.to_string(),
                );
                self.connection = Some(connection);
                debug!("Successfully connected to Neovim at {}", address);
                Ok(())
            }
            Err(e) => {
                debug!("Failed to connect to Neovim at {}: {}", address, e);
                Err(NeovimError::Connection(format!("Connection failed: {e}")))
            }
        }
    }
}

impl<T> NeovimClient<T>
where
    T: AsyncWrite + Send + 'static,
{
    pub fn new() -> Self {
        Self { connection: None }
    }

    #[instrument(skip(self))]
    async fn get_diagnostics(
        &self,
        buffer_id: Option<u64>,
    ) -> Result<Vec<Diagnostic>, NeovimError> {
        let conn = self.connection.as_ref().ok_or_else(|| {
            NeovimError::Connection("Not connected to any Neovim instance".to_string())
        })?;

        let args = if let Some(id) = buffer_id {
            vec![Value::from(id)]
        } else {
            vec![]
        };

        match conn
            .nvim
            .execute_lua("return vim.json.encode(vim.diagnostic.get(...))", args)
            .await
        {
            Ok(diagnostics) => {
                let diagnostics: Vec<Diagnostic> =
                    match serde_json::from_str(diagnostics.as_str().unwrap()) {
                        Ok(d) => d,
                        Err(e) => {
                            debug!("Failed to parse diagnostics: {}", e);
                            return Err(NeovimError::Api(format!(
                                "Failed to parse diagnostics: {e}"
                            )));
                        }
                    };
                debug!("Found {} diagnostics", diagnostics.len());
                Ok(diagnostics)
            }
            Err(e) => {
                debug!("Failed to get diagnostics: {}", e);
                Err(NeovimError::Api(format!("Failed to get diagnostics: {e}")))
            }
        }
    }

    #[instrument(skip(self))]
    async fn lsp_make_text_document_params(
        &self,
        buffer_id: u64,
    ) -> Result<TextDocumentIdentifier, NeovimError> {
        let conn = self.connection.as_ref().ok_or_else(|| {
            NeovimError::Connection("Not connected to any Neovim instance".to_string())
        })?;

        match conn
            .nvim
            .execute_lua(
                include_str!("lua/lsp_make_text_document_params.lua"),
                vec![Value::from(buffer_id)],
            )
            .await
        {
            Ok(raw) => {
                let doc = serde_json::from_str::<TextDocumentIdentifier>(raw.as_str().unwrap())
                    .map_err(|e| {
                        NeovimError::Api(format!("Failed to parse text document params: {e}"))
                    })?;
                info!("Created text document params {doc:?} for buffer {buffer_id}");
                Ok(doc)
            }
            Err(e) => {
                debug!("Failed to make text document params: {}", e);
                Err(NeovimError::Api(format!(
                    "Failed to make text document params: {e}"
                )))
            }
        }
    }

    /// Get project root directory from Neovim
    #[instrument(skip(self))]
    async fn get_project_root(&self) -> Result<PathBuf, NeovimError> {
        let conn = self.connection.as_ref().ok_or_else(|| {
            NeovimError::Connection("Not connected to any Neovim instance".to_string())
        })?;

        match conn
            .nvim
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
                // Use the existing path-based helper function
                make_text_document_identifier_from_path(abs_path)
            }
        }
    }

    /// Enhanced code actions method supporting universal document identification
    #[instrument(skip(self))]
    pub async fn lsp_get_code_actions(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
        range: Range,
    ) -> Result<Vec<CodeAction>, NeovimError> {
        let text_document = self.resolve_text_document_identifier(&document).await?;

        let diagnostics = match &document {
            DocumentIdentifier::BufferId(buffer_id) => self
                .get_buffer_diagnostics(*buffer_id)
                .await
                .map_err(|e| NeovimError::Api(format!("Failed to get diagnostics: {e}")))?,
            _ => {
                // For path-based identifiers, diagnostics might not be available
                Vec::new()
            }
        };

        let conn = self.connection.as_ref().ok_or_else(|| {
            NeovimError::Connection("Not connected to any Neovim instance".to_string())
        })?;

        // Get buffer ID for Lua execution (needed for some LSP operations)
        let buffer_id = match &document {
            DocumentIdentifier::BufferId(id) => *id,
            _ => 0, // Use buffer 0 as fallback for path-based operations
        };

        match conn
            .nvim
            .execute_lua(
                include_str!("lua/lsp_client_get_code_actions.lua"),
                vec![
                    Value::from(client_name), // client_name
                    Value::from(
                        serde_json::to_string(&CodeActionParams {
                            text_document,
                            range,
                            context: CodeActionContext {
                                diagnostics: diagnostics
                                    .into_iter()
                                    .filter_map(|d| d.user_data.map(|u| u.lsp))
                                    .collect(),
                                only: None,
                                trigger_kind: None,
                            },
                        })
                        .unwrap(),
                    ), // params
                    Value::from(1000),        // timeout_ms
                    Value::from(buffer_id),   // bufnr
                ],
            )
            .await
        {
            Ok(actions) => {
                let actions = serde_json::from_str::<CodeActionResult>(actions.as_str().unwrap())
                    .map_err(|e| {
                    NeovimError::Api(format!("Failed to parse code actions: {e}"))
                })?;
                debug!("Found {} code actions", actions.result.len());
                Ok(actions.result)
            }
            Err(e) => {
                debug!("Failed to get LSP code actions: {}", e);
                Err(NeovimError::Api(format!(
                    "Failed to get LSP code actions: {e}"
                )))
            }
        }
    }

    /// Enhanced hover method supporting universal document identification
    #[instrument(skip(self))]
    pub async fn lsp_hover(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
        position: Position,
    ) -> Result<HoverResult, NeovimError> {
        let text_document = self.resolve_text_document_identifier(&document).await?;

        let conn = self.connection.as_ref().ok_or_else(|| {
            NeovimError::Connection("Not connected to any Neovim instance".to_string())
        })?;

        // Get buffer ID for Lua execution (needed for some LSP operations)
        let buffer_id = match &document {
            DocumentIdentifier::BufferId(id) => *id,
            _ => 0, // Use buffer 0 as fallback for path-based operations
        };

        match conn
            .nvim
            .execute_lua(
                include_str!("lua/lsp_hover.lua"),
                vec![
                    Value::from(client_name), // client_name
                    Value::from(
                        serde_json::to_string(&HoverParams {
                            text_document,
                            position,
                        })
                        .unwrap(),
                    ), // params
                    Value::from(1000),        // timeout_ms
                    Value::from(buffer_id),   // bufnr
                ],
            )
            .await
        {
            Ok(result) => {
                debug!("LSP Hover retrieved successfully");
                #[derive(Debug, serde::Deserialize)]
                struct Result {
                    result: HoverResult,
                }
                let result: Result = match serde_json::from_str(result.as_str().unwrap()) {
                    Ok(d) => d,
                    Err(e) => {
                        debug!("Failed to parse hover result: {}", e);
                        return Err(NeovimError::Api(format!(
                            "Failed to parse hover result: {e}"
                        )));
                    }
                };
                Ok(result.result)
            }
            Err(e) => {
                debug!("Failed to get LSP hover: {}", e);
                Err(NeovimError::Api(format!("Failed to get LSP hover: {e}")))
            }
        }
    }

    /// Enhanced document symbols method supporting universal document identification
    #[instrument(skip(self))]
    pub async fn lsp_document_symbols(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
    ) -> Result<Option<DocumentSymbolResult>, NeovimError> {
        let text_document = self.resolve_text_document_identifier(&document).await?;

        let conn = self.connection.as_ref().ok_or_else(|| {
            NeovimError::Connection("Not connected to any Neovim instance".to_string())
        })?;

        // Get buffer ID for Lua execution (needed for some LSP operations)
        let buffer_id = match &document {
            DocumentIdentifier::BufferId(id) => *id,
            _ => 0, // Use buffer 0 as fallback for path-based operations
        };

        match conn
            .nvim
            .execute_lua(
                include_str!("lua/lsp_document_symbols.lua"),
                vec![
                    Value::from(client_name), // client_name
                    Value::from(
                        serde_json::to_string(&DocumentSymbolParams { text_document }).unwrap(),
                    ), // params
                    Value::from(1000),        // timeout_ms
                    Value::from(buffer_id),   // bufnr
                ],
            )
            .await
        {
            Ok(result) => {
                debug!("LSP Document symbols retrieved successfully");
                #[derive(Debug, serde::Deserialize)]
                struct Result {
                    result: Option<DocumentSymbolResult>,
                }
                let result: Result = match serde_json::from_str(result.as_str().unwrap()) {
                    Ok(d) => d,
                    Err(e) => {
                        debug!("Failed to parse document symbols result: {}", e);
                        return Err(NeovimError::Api(format!(
                            "Failed to parse document symbols result: {e}"
                        )));
                    }
                };
                Ok(result.result)
            }
            Err(e) => {
                debug!("Failed to get document symbols: {}", e);
                Err(NeovimError::Api(format!(
                    "Failed to get document symbols: {e}"
                )))
            }
        }
    }

    /// Enhanced references method supporting universal document identification
    #[instrument(skip(self))]
    pub async fn lsp_references(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
        position: Position,
        include_declaration: bool,
    ) -> Result<Vec<Location>, NeovimError> {
        let text_document = self.resolve_text_document_identifier(&document).await?;

        let conn = self.connection.as_ref().ok_or_else(|| {
            NeovimError::Connection("Not connected to any Neovim instance".to_string())
        })?;

        // Get buffer ID for Lua execution (needed for some LSP operations)
        let buffer_id = match &document {
            DocumentIdentifier::BufferId(id) => *id,
            _ => 0, // Use buffer 0 as fallback for path-based operations
        };

        match conn
            .nvim
            .execute_lua(
                include_str!("lua/lsp_references.lua"),
                vec![
                    Value::from(client_name), // client_name
                    Value::from(
                        serde_json::to_string(&ReferenceParams {
                            text_document,
                            position,
                            context: ReferenceContext {
                                include_declaration,
                            },
                        })
                        .unwrap(),
                    ), // params
                    Value::from(1000),        // timeout_ms
                    Value::from(buffer_id),   // bufnr
                ],
            )
            .await
        {
            Ok(result) => {
                debug!("LSP References retrieved successfully");
                #[derive(Debug, serde::Deserialize)]
                struct Result {
                    result: Option<Vec<Location>>,
                }
                let result: Result = match serde_json::from_str(result.as_str().unwrap()) {
                    Ok(d) => d,
                    Err(e) => {
                        debug!("Failed to parse references result: {}", e);
                        return Err(NeovimError::Api(format!(
                            "Failed to parse references result: {e}"
                        )));
                    }
                };
                Ok(result.result.unwrap_or_default())
            }
            Err(e) => {
                debug!("Failed to get LSP references: {}", e);
                Err(NeovimError::Api(format!(
                    "Failed to get LSP references: {e}"
                )))
            }
        }
    }
}

#[async_trait]
impl<T> NeovimClientTrait for NeovimClient<T>
where
    T: AsyncWrite + Send + 'static,
{
    fn target(&self) -> Option<String> {
        self.connection.as_ref().map(|c| c.target().to_string())
    }

    #[instrument(skip(self))]
    async fn disconnect(&mut self) -> Result<String, NeovimError> {
        debug!("Attempting to disconnect from Neovim");

        if let Some(connection) = self.connection.take() {
            let target = connection.target().to_string();
            connection.io_handler.abort();
            debug!("Successfully disconnected from Neovim at {}", target);
            Ok(target)
        } else {
            Err(NeovimError::Connection(
                "Not connected to any Neovim instance".to_string(),
            ))
        }
    }

    #[instrument(skip(self))]
    async fn get_buffers(&self) -> Result<Vec<BufferInfo>, NeovimError> {
        debug!("Getting buffer information");

        let lua_code = include_str!("lua/lsp_get_buffers.lua");

        match self.execute_lua(lua_code).await {
            Ok(buffers) => {
                debug!("Get buffers retrieved successfully");
                let buffers: Vec<BufferInfo> = match serde_json::from_str(buffers.as_str().unwrap())
                {
                    Ok(d) => d,
                    Err(e) => {
                        debug!("Failed to parse buffers: {}", e);
                        return Err(NeovimError::Api(format!("Failed to parse buffers: {e}")));
                    }
                };
                debug!("Found {} buffers", buffers.len());
                Ok(buffers)
            }
            Err(e) => {
                debug!("Failed to get buffer info: {}", e);
                Err(NeovimError::Api(format!("Failed to get buffer info: {e}")))
            }
        }
    }

    #[instrument(skip(self))]
    async fn execute_lua(&self, code: &str) -> Result<Value, NeovimError> {
        debug!("Executing Lua code: {}", code);

        if code.trim().is_empty() {
            return Err(NeovimError::Api("Lua code cannot be empty".to_string()));
        }

        let conn = self.connection.as_ref().ok_or_else(|| {
            NeovimError::Connection("Not connected to any Neovim instance".to_string())
        })?;

        let lua_args = Vec::<Value>::new();
        match conn.nvim.exec_lua(code, lua_args).await {
            Ok(result) => {
                debug!("Lua execution successful, result: {:?}", result);
                Ok(result)
            }
            Err(e) => {
                debug!("Lua execution failed: {e}");
                Err(NeovimError::Api(format!("Lua execution failed: {e}")))
            }
        }
    }

    #[instrument(skip(self))]
    async fn setup_diagnostics_changed_autocmd(&self) -> Result<(), NeovimError> {
        debug!("Setting up diagnostics changed autocmd");

        let conn = self.connection.as_ref().ok_or_else(|| {
            NeovimError::Connection("Not connected to any Neovim instance".to_string())
        })?;

        match conn
            .nvim
            .exec_lua(include_str!("lua/diagnostics_autocmd.lua"), vec![])
            .await
        {
            Ok(_) => {
                debug!("Autocmd for diagnostics changed set up successfully");
                Ok(())
            }
            Err(e) => {
                debug!("Failed to set up diagnostics changed autocmd: {}", e);
                Err(NeovimError::Api(format!(
                    "Failed to set up diagnostics changed autocmd: {e}"
                )))
            }
        }
    }

    #[instrument(skip(self))]
    async fn get_buffer_diagnostics(&self, buffer_id: u64) -> Result<Vec<Diagnostic>, NeovimError> {
        self.get_diagnostics(Some(buffer_id)).await
    }

    #[instrument(skip(self))]
    async fn get_workspace_diagnostics(&self) -> Result<Vec<Diagnostic>, NeovimError> {
        self.get_diagnostics(None).await
    }

    #[instrument(skip(self))]
    async fn lsp_get_clients(&self) -> Result<Vec<LspClient>, NeovimError> {
        debug!("Getting LSP clients");

        let conn = self.connection.as_ref().ok_or_else(|| {
            NeovimError::Connection("Not connected to any Neovim instance".to_string())
        })?;

        match conn
            .nvim
            .execute_lua(include_str!("lua/lsp_get_clients.lua"), vec![])
            .await
        {
            Ok(clients) => {
                debug!("LSP clients retrieved successfully");
                let clients: Vec<LspClient> = match serde_json::from_str(clients.as_str().unwrap())
                {
                    Ok(d) => d,
                    Err(e) => {
                        debug!("Failed to parse clients: {}", e);
                        return Err(NeovimError::Api(format!("Failed to parse clients: {e}")));
                    }
                };
                debug!("Found {} clients", clients.len());
                Ok(clients)
            }
            Err(e) => {
                debug!("Failed to get LSP clients: {}", e);
                Err(NeovimError::Api(format!("Failed to get LSP clients: {e}")))
            }
        }
    }

    #[instrument(skip(self))]
    async fn lsp_get_code_actions(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
        range: Range,
    ) -> Result<Vec<CodeAction>, NeovimError> {
        self.lsp_get_code_actions(client_name, document, range)
            .await
    }

    #[instrument(skip(self))]
    async fn lsp_hover(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
        position: Position,
    ) -> Result<HoverResult, NeovimError> {
        self.lsp_hover(client_name, document, position).await
    }

    #[instrument(skip(self))]
    async fn lsp_document_symbols(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
    ) -> Result<Option<DocumentSymbolResult>, NeovimError> {
        self.lsp_document_symbols(client_name, document).await
    }

    #[instrument(skip(self))]
    async fn lsp_workspace_symbols(
        &self,
        client_name: &str,
        query: &str,
    ) -> Result<WorkspaceSymbolResult, NeovimError> {
        let conn = self.connection.as_ref().ok_or_else(|| {
            NeovimError::Connection("Not connected to any Neovim instance".to_string())
        })?;

        match conn
            .nvim
            .execute_lua(
                include_str!("lua/lsp_workspace_symbols.lua"),
                vec![
                    Value::from(client_name), // client_name
                    Value::from(
                        serde_json::to_string(&WorkspaceSymbolParams {
                            query: query.to_string(),
                        })
                        .unwrap(),
                    ), // params
                    Value::from(1000),        // timeout_ms
                ],
            )
            .await
        {
            Ok(result) => {
                debug!("LSP Workspace symbols retrieved successfully");
                let result: WorkspaceSymbolResult =
                    match serde_json::from_str(result.as_str().unwrap()) {
                        Ok(d) => d,
                        Err(e) => {
                            debug!("Failed to parse workspace symbols result: {}", e);
                            return Err(NeovimError::Api(format!(
                                "Failed to parse workspace symbols result: {e}"
                            )));
                        }
                    };
                Ok(result)
            }
            Err(e) => {
                debug!("Failed to get workspace symbols: {}", e);
                Err(NeovimError::Api(format!(
                    "Failed to get workspace symbols: {e}"
                )))
            }
        }
    }

    #[instrument(skip(self))]
    async fn lsp_references(
        &self,
        client_name: &str,
        document: DocumentIdentifier,
        position: Position,
        include_declaration: bool,
    ) -> Result<Vec<Location>, NeovimError> {
        self.lsp_references(client_name, document, position, include_declaration)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_symbol_kind_serialization() {
        assert_eq!(serde_json::to_value(SymbolKind::Function).unwrap(), 12);
        assert_eq!(serde_json::to_value(SymbolKind::Variable).unwrap(), 13);
        assert_eq!(serde_json::to_value(SymbolKind::Class).unwrap(), 5);
    }

    #[test]
    fn test_symbol_information_serialization() {
        let symbol = SymbolInformation {
            name: "test_function".to_string(),
            kind: SymbolKind::Function,
            tags: None,
            deprecated: None,
            location: Location {
                uri: "file:///test.rs".to_string(),
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 13,
                    },
                },
            },
            container_name: None,
        };

        let json = serde_json::to_string(&symbol).unwrap();
        assert!(json.contains("test_function"));
        assert!(json.contains("file:///test.rs"));
    }

    #[test]
    fn test_document_symbol_serialization() {
        let symbol = DocumentSymbol {
            name: "TestClass".to_string(),
            detail: Some("class TestClass".to_string()),
            kind: SymbolKind::Class,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 10,
                    character: 0,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 0,
                    character: 6,
                },
                end: Position {
                    line: 0,
                    character: 15,
                },
            },
            children: None,
        };

        let json = serde_json::to_string(&symbol).unwrap();
        assert!(json.contains("TestClass"));
        assert!(json.contains("class TestClass"));
    }

    #[test]
    fn test_workspace_symbol_params_serialization() {
        let params = WorkspaceSymbolParams {
            query: "function".to_string(),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("function"));
    }

    #[test]
    fn test_reference_params_serialization() {
        let params = ReferenceParams {
            text_document: TextDocumentIdentifier {
                uri: "file:///test.rs".to_string(),
                version: Some(1),
            },
            position: Position {
                line: 10,
                character: 5,
            },
            context: ReferenceContext {
                include_declaration: true,
            },
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("textDocument"));
        assert!(json.contains("position"));
        assert!(json.contains("context"));
        assert!(json.contains("includeDeclaration"));
        assert!(json.contains("true"));
    }

    #[test]
    fn test_make_text_document_identifier_from_path() {
        // Test with current file (this source file should exist)
        let current_file = file!();
        let result = make_text_document_identifier_from_path(current_file);

        assert!(result.is_ok());
        let text_doc = result.unwrap();
        assert!(text_doc.uri.starts_with("file://"));
        assert!(text_doc.uri.ends_with("client.rs"));
        assert_eq!(text_doc.version, None);
    }

    #[test]
    fn test_make_text_document_identifier_from_path_invalid() {
        // Test with non-existent path
        let result = make_text_document_identifier_from_path("/nonexistent/path/file.rs");
        assert!(result.is_err());

        if let Err(NeovimError::Api(msg)) = result {
            assert!(msg.contains("Failed to resolve path"));
        } else {
            panic!("Expected NeovimError::Api");
        }
    }

    #[test]
    fn test_document_identifier_creation() {
        let buffer_id = DocumentIdentifier::from_buffer_id(42);
        assert_eq!(buffer_id, DocumentIdentifier::BufferId(42));

        let rel_path = DocumentIdentifier::from_project_path("src/lib.rs");
        assert_eq!(
            rel_path,
            DocumentIdentifier::ProjectRelativePath(PathBuf::from("src/lib.rs"))
        );

        let abs_path = DocumentIdentifier::from_absolute_path("/usr/src/lib.rs");
        assert_eq!(
            abs_path,
            DocumentIdentifier::AbsolutePath(PathBuf::from("/usr/src/lib.rs"))
        );
    }

    #[test]
    fn test_document_identifier_serde() {
        // Test BufferId serialization
        let buffer_id = DocumentIdentifier::from_buffer_id(123);
        let json = serde_json::to_string(&buffer_id).unwrap();
        assert!(json.contains("buffer_id"));
        assert!(json.contains("123"));

        let deserialized: DocumentIdentifier = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, buffer_id);

        // Test ProjectRelativePath serialization
        let rel_path = DocumentIdentifier::from_project_path("src/main.rs");
        let json = serde_json::to_string(&rel_path).unwrap();
        assert!(json.contains("project_relative_path"));
        assert!(json.contains("src/main.rs"));

        let deserialized: DocumentIdentifier = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, rel_path);

        // Test AbsolutePath serialization
        let abs_path = DocumentIdentifier::from_absolute_path("/tmp/test.rs");
        let json = serde_json::to_string(&abs_path).unwrap();
        assert!(json.contains("absolute_path"));
        assert!(json.contains("/tmp/test.rs"));

        let deserialized: DocumentIdentifier = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, abs_path);
    }

    #[test]
    fn test_document_identifier_json_schema() {
        use schemars::schema_for;

        let schema = schema_for!(DocumentIdentifier);
        let schema_json = serde_json::to_string(&schema).unwrap();

        // Verify the schema contains the expected enum variants
        assert!(schema_json.contains("buffer_id"));
        assert!(schema_json.contains("project_relative_path"));
        assert!(schema_json.contains("absolute_path"));
    }

    #[test]
    fn test_document_identifier_string_deserializer_buffer_id() {
        // Test deserializing buffer_id from JSON string
        let json_str = r#"{"buffer_id": 42}"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(json_str);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), DocumentIdentifier::BufferId(42));

        // Test large buffer ID
        let json_str = r#"{"buffer_id": 18446744073709551615}"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(json_str);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            DocumentIdentifier::BufferId(18446744073709551615)
        );
    }

    #[test]
    fn test_document_identifier_string_deserializer_project_path() {
        // Test deserializing project_relative_path from JSON string
        let json_str = r#"{"project_relative_path": "src/main.rs"}"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(json_str);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            DocumentIdentifier::ProjectRelativePath(PathBuf::from("src/main.rs"))
        );

        // Test with nested path
        let json_str = r#"{"project_relative_path": "src/server/tools.rs"}"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(json_str);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            DocumentIdentifier::ProjectRelativePath(PathBuf::from("src/server/tools.rs"))
        );

        // Test with empty path
        let json_str = r#"{"project_relative_path": ""}"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(json_str);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            DocumentIdentifier::ProjectRelativePath(PathBuf::from(""))
        );
    }

    #[test]
    fn test_document_identifier_string_deserializer_absolute_path() {
        // Test deserializing absolute_path from JSON string
        let json_str = r#"{"absolute_path": "/usr/local/src/main.rs"}"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(json_str);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            DocumentIdentifier::AbsolutePath(PathBuf::from("/usr/local/src/main.rs"))
        );

        // Test Windows-style absolute path
        let json_str = r#"{"absolute_path": "C:\\Users\\test\\Documents\\file.rs"}"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(json_str);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            DocumentIdentifier::AbsolutePath(PathBuf::from("C:\\Users\\test\\Documents\\file.rs"))
        );
    }

    #[test]
    fn test_document_identifier_string_deserializer_via_visitor() {
        // Test the custom deserializer's visit_str method by passing JSON as a string value
        // This simulates Claude Code's usage pattern where JSON is embedded as string
        let json_as_string = r#""{\"buffer_id\": 123}""#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(json_as_string);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), DocumentIdentifier::BufferId(123));

        let json_as_string = r#""{\"project_relative_path\": \"src/lib.rs\"}""#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(json_as_string);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            DocumentIdentifier::ProjectRelativePath(PathBuf::from("src/lib.rs"))
        );

        let json_as_string = r#""{\"absolute_path\": \"/home/user/file.rs\"}""#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(json_as_string);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            DocumentIdentifier::AbsolutePath(PathBuf::from("/home/user/file.rs"))
        );
    }

    #[test]
    fn test_document_identifier_string_deserializer_error_cases() {
        // Test invalid JSON string format
        let invalid_json = r#"{"invalid_field": 42}"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());

        // Test empty JSON object
        let empty_json = r#"{}"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(empty_json);
        assert!(result.is_err());

        // Test malformed JSON
        let malformed_json = r#"{"buffer_id": invalid}"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(malformed_json);
        assert!(result.is_err());

        // Test negative buffer_id (should fail for u64)
        let negative_json = r#"{"buffer_id": -1}"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(negative_json);
        assert!(result.is_err());

        // Test string with malformed embedded JSON
        let malformed_embedded = r#""{\"buffer_id\": }"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(malformed_embedded);
        assert!(result.is_err());
    }

    #[test]
    fn test_document_identifier_string_deserializer_mixed_cases() {
        // Test with extra whitespace in JSON
        let json_str = r#"{ "buffer_id" : 999 }"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(json_str);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), DocumentIdentifier::BufferId(999));

        // Test with Unicode in paths
        let json_str = r#"{"project_relative_path": "src/测试.rs"}"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(json_str);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            DocumentIdentifier::ProjectRelativePath(PathBuf::from("src/测试.rs"))
        );

        // Test with special characters in paths
        let json_str = r#"{"absolute_path": "/tmp/test with spaces & symbols!.rs"}"#;
        let result: Result<DocumentIdentifier, _> = serde_json::from_str(json_str);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            DocumentIdentifier::AbsolutePath(PathBuf::from("/tmp/test with spaces & symbols!.rs"))
        );
    }

    #[test]
    fn test_document_identifier_round_trip_serialization() {
        // Test round-trip serialization/deserialization for all variants
        let test_cases = vec![
            DocumentIdentifier::BufferId(42),
            DocumentIdentifier::ProjectRelativePath(PathBuf::from("src/main.rs")),
            DocumentIdentifier::AbsolutePath(PathBuf::from("/usr/src/main.rs")),
        ];

        for original in test_cases {
            // Serialize to JSON
            let json = serde_json::to_string(&original).unwrap();

            // Deserialize back from JSON
            let deserialized: DocumentIdentifier = serde_json::from_str(&json).unwrap();

            // Verify round-trip equality
            assert_eq!(original, deserialized);

            // Test string-embedded JSON deserialization (Claude Code use case)
            let json_string = serde_json::to_string(&json).unwrap();
            let from_string: DocumentIdentifier = serde_json::from_str(&json_string).unwrap();
            assert_eq!(original, from_string);
        }
    }
}
