#[derive(Debug, thiserror::Error)]
pub enum NeovimError {
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("LSP error: {code} {message}")]
    Lsp { message: String, code: i32 },
}

impl From<std::io::Error> for NeovimError {
    fn from(err: std::io::Error) -> Self {
        NeovimError::Connection(err.to_string())
    }
}

impl From<nvim_rs::error::CallError> for NeovimError {
    fn from(err: nvim_rs::error::CallError) -> Self {
        NeovimError::Api(err.to_string())
    }
}
