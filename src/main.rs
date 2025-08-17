use clap::Parser;
use rmcp::{ServiceExt, transport::stdio};
use std::{path::PathBuf, sync::OnceLock};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use nvim_mcp::{ConfigError, NeovimMcpServer, ServerConfig};

static LONG_VERSION: OnceLock<String> = OnceLock::new();

fn long_version() -> &'static str {
    LONG_VERSION
        .get_or_init(|| {
            // This closure is executed only once, on the first call to get_or_init
            let dirty = if env!("GIT_DIRTY") == "true" {
                "[dirty]"
            } else {
                ""
            };
            format!(
                "{} (sha:{:?}, build_time:{:?}){}",
                env!("CARGO_PKG_VERSION"),
                env!("GIT_COMMIT_SHA"),
                env!("BUILT_TIME_UTC"),
                dirty
            )
        })
        .as_str()
}

#[derive(Parser)]
#[command(version, long_version=long_version(), about, long_about = None)]
struct Cli {
    /// Path to the log file. If not specified, logs to stderr
    #[arg(long)]
    log_file: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Directory for socket files. Defaults to platform-specific location
    #[arg(long)]
    socket_path: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize tracing/logging
    let env_filter = EnvFilter::from_default_env().add_directive(cli.log_level.parse()?);

    let log_file_clone = cli.log_file.clone();
    let _guard = if let Some(log_file) = log_file_clone {
        // Log to file
        let file_appender = tracing_appender::rolling::never(
            log_file
                .parent()
                .unwrap_or_else(|| std::path::Path::new(".")),
            log_file
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("nvim-mcp.log")),
        );
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        tracing_subscriber::fmt()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_env_filter(env_filter)
            .init();

        // Note: _guard is a WorkerGuard which is returned by tracing_appender::non_blocking
        // to ensure buffered logs are flushed to their output
        // in the case of abrupt terminations of a process.
        Some(guard)
    } else {
        // Log to stderr (default behavior)
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_env_filter(env_filter)
            .init();

        None
    };

    // Create server configuration with lazy evaluation
    let config = ServerConfig::new(cli.socket_path, cli.log_file, cli.log_level)
        .map_err(|e: ConfigError| format!("Configuration error: {}", e))?;

    info!("Starting nvim-mcp Neovim server");
    let server = NeovimMcpServer::new(config.socket_path);
    let service = server.serve(stdio()).await.inspect_err(|e| {
        error!("Error starting Neovim server: {}", e);
    })?;
    info!("Neovim server started, waiting for connections...");
    service.waiting().await?;

    info!("Server shutdown complete");

    Ok(())
}
