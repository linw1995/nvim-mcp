use std::{path::PathBuf, sync::OnceLock};

use clap::Parser;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use rmcp::{
    ServiceExt,
    transport::{
        StreamableHttpServerConfig, StreamableHttpService, stdio,
        streamable_http_server::session::local::LocalSessionManager,
    },
};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use nvim_mcp::NeovimMcpServer;

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

    /// Enable HTTP server mode on the specified port
    #[arg(long)]
    http_port: Option<u16>,

    /// HTTP server bind address (default: 127.0.0.1)
    #[arg(long, default_value = "127.0.0.1")]
    http_host: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize tracing/logging
    let env_filter = EnvFilter::from_default_env().add_directive(cli.log_level.parse()?);

    let _guard = if let Some(log_file) = cli.log_file {
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

    info!("Starting nvim-mcp Neovim server");
    let server = NeovimMcpServer::new();

    if let Some(port) = cli.http_port {
        // HTTP server mode
        let addr = format!("{}:{}", cli.http_host, port);
        info!("Starting HTTP server on {}", addr);
        let service = TowerToHyperService::new(StreamableHttpService::new(
            || Ok(NeovimMcpServer::new()),
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig {
                stateful_mode: true,
                ..Default::default()
            },
        ));
        let listener = tokio::net::TcpListener::bind(addr).await?;
        loop {
            let io = tokio::select! {
                _ = tokio::signal::ctrl_c() => break,
                accept = listener.accept() => {
                    TokioIo::new(accept?.0)
                }
            };
            let service = service.clone();
            tokio::spawn(async move {
                let _result = Builder::new(TokioExecutor::default())
                    .serve_connection(io, service)
                    .await;
            });
        }
    } else {
        // Default stdio mode
        let service = server.serve(stdio()).await.inspect_err(|e| {
            error!("Error starting Neovim server: {}", e);
        })?;

        info!("Neovim server started, waiting for connections...");
        service.waiting().await?;
    };
    info!("Server shutdown complete");

    Ok(())
}
