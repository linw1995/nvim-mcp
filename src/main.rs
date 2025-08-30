use std::{path::PathBuf, str::FromStr, sync::OnceLock};

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
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use nvim_mcp::{NeovimMcpServer, auto_connect_current_project_targets, auto_connect_single_target};

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

#[derive(Clone, Debug)]
enum ConnectBehavior {
    Manual,
    Auto,
    SpecificTarget(String),
}

impl std::fmt::Display for ConnectBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectBehavior::Manual => write!(f, "manual"),
            ConnectBehavior::Auto => write!(f, "auto"),
            ConnectBehavior::SpecificTarget(target) => write!(f, "{}", target),
        }
    }
}

impl FromStr for ConnectBehavior {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "manual" => Ok(ConnectBehavior::Manual),
            "auto" => Ok(ConnectBehavior::Auto),
            target => {
                // Validate TCP address format
                if target.parse::<std::net::SocketAddr>().is_ok() {
                    return Ok(ConnectBehavior::SpecificTarget(target.to_string()));
                }

                // Validate file path (socket/pipe)
                let path = std::path::Path::new(target);
                if path.is_absolute()
                    && (path.exists() || path.parent().is_some_and(|p| p.exists()))
                {
                    return Ok(ConnectBehavior::SpecificTarget(target.to_string()));
                }

                Err(format!(
                    "Invalid target: '{}'. Must be 'manual', 'auto', TCP address (e.g., '127.0.0.1:6666'), or absolute socket path",
                    target
                ))
            }
        }
    }
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

    /// Connection mode: 'manual', 'auto', or specific target (TCP address/socket path)
    #[arg(long, default_value = "manual")]
    connect: ConnectBehavior,
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
    let connect_mode = cli.connect.to_string();
    let server = NeovimMcpServer::with_connect_mode(Some(connect_mode.clone()));

    // Handle connection mode
    let connection_ids = match cli.connect {
        ConnectBehavior::Auto => {
            match auto_connect_current_project_targets(&server).await {
                Ok(connections) => {
                    if connections.is_empty() {
                        info!("No Neovim instances found for current project");
                    } else {
                        info!("Auto-connected to {} project instances", connections.len());
                    }
                    connections
                }
                Err(failures) => {
                    warn!("Auto-connection failed for all {} targets", failures.len());
                    for (target, error) in &failures {
                        warn!("  {target}: {error}");
                    }
                    // Continue serving - manual connections still possible
                    vec![]
                }
            }
        }
        ConnectBehavior::SpecificTarget(target) => {
            match auto_connect_single_target(&server, &target).await {
                Ok(id) => {
                    info!("Connected to specific target {} with ID {}", target, id);
                    vec![id]
                }
                Err(e) => return Err(format!("Failed to connect to {}: {}", target, e).into()),
            }
        }
        ConnectBehavior::Manual => {
            info!("Manual connection mode - use get_targets and connect tools");
            vec![]
        }
    };

    if !connection_ids.is_empty() {
        server
            .discover_and_register_lua_tools()
            .await
            .inspect_err(|e| {
                error!("Error setting up Lua tools: {}", e);
            })?;
    }

    if let Some(port) = cli.http_port {
        // HTTP server mode
        let addr = format!("{}:{}", cli.http_host, port);
        info!("Starting HTTP server on {}", addr);
        let service = TowerToHyperService::new(StreamableHttpService::new(
            move || {
                Ok(NeovimMcpServer::with_connect_mode(Some(
                    connect_mode.clone(),
                )))
            },
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
                if let Err(e) = Builder::new(TokioExecutor::default())
                    .serve_connection(io, service)
                    .await
                {
                    error!("Error serving HTTP connection: {e}");
                }
            });
        }
    } else {
        // Default stdio mode
        info!("Starting Neovim server on stdio");
        let service = server.serve(stdio()).await.inspect_err(|e| {
            error!("Error starting Neovim server: {}", e);
        })?;

        info!("Neovim server started, waiting for connections...");
        service.waiting().await?;
    };
    info!("Server shutdown complete");

    Ok(())
}
