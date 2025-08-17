use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Environment variable error: {0}")]
    Environment(String),
    #[error("Filesystem error: {0}")]
    Filesystem(String),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

/// Configuration for the Neovim MCP server
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub socket_path: PathBuf,
    pub log_file: Option<PathBuf>,
    pub log_level: String,
}

impl ServerConfig {
    /// Create a new server configuration with resolved socket path
    pub fn new(
        socket_path: Option<String>,
        log_file: Option<PathBuf>,
        log_level: String,
    ) -> Result<Self, ConfigError> {
        let socket_path = Self::resolve_socket_path(socket_path)?;

        Ok(Self {
            socket_path,
            log_file,
            log_level,
        })
    }

    /// Resolve socket path from optional user input or platform defaults
    pub fn resolve_socket_path(provided: Option<String>) -> Result<PathBuf, ConfigError> {
        match provided {
            Some(path) => {
                let path_buf = PathBuf::from(path);
                std::fs::create_dir_all(&path_buf).map_err(|e| {
                    ConfigError::Filesystem(format!(
                        "Cannot create directory {}: {}",
                        path_buf.display(),
                        e
                    ))
                })?;
                Ok(path_buf)
            }
            None => Self::default_socket_path(),
        }
    }

    /// Get platform-specific default socket directory
    fn default_socket_path() -> Result<PathBuf, ConfigError> {
        let socket_dir =
            if cfg!(target_os = "windows") {
                PathBuf::from(std::env::var("TEMP").map_err(|e| {
                    ConfigError::Environment(format!("TEMP variable not set: {}", e))
                })?)
            } else {
                PathBuf::from("/tmp")
            };

        // Ensure directory exists
        std::fs::create_dir_all(&socket_dir).map_err(|e| {
            ConfigError::Filesystem(format!(
                "Cannot create socket directory {}: {}",
                socket_dir.display(),
                e
            ))
        })?;

        Ok(socket_dir)
    }
}
