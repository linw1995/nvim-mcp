use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tokio::time::sleep;
use tracing::debug;

#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(windows)]
use tokio::net::windows::named_pipe::NamedPipeClient;

use crate::neovim::NeovimClient;
use crate::neovim::NeovimClientTrait;

// Constants
pub const HOST: &str = "127.0.0.1";
pub const PORT_BASE: u16 = 7777;

// Global mutex to prevent concurrent Neovim instances from using the same port
pub static NEOVIM_TEST_MUTEX: Mutex<()> = Mutex::new(());

/// Generate a random alphanumeric ID for test isolation
pub fn generate_random_id() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..16)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Generate a random Unix socket path for testing
#[cfg(unix)]
pub fn generate_random_socket_path() -> String {
    let random_id = generate_random_id();
    format!("/tmp/nvim-mcp-test-{random_id}.sock")
}

/// Generate a random Windows named pipe path for testing
#[cfg(windows)]
pub fn generate_random_pipe_path() -> String {
    let random_id = generate_random_id();
    format!("\\\\.\\pipe\\nvim-mcp-test-{random_id}")
}

/// Cross-platform IPC path generation
#[cfg(unix)]
pub fn generate_random_ipc_path() -> String {
    generate_random_socket_path()
}

/// Cross-platform IPC path generation
#[cfg(windows)]
pub fn generate_random_ipc_path() -> String {
    generate_random_pipe_path()
}

/// Get the path to nvim executable
pub fn nvim_path() -> &'static str {
    "nvim"
}

/// Get test data file path
pub fn get_testdata_path(filename: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("src/neovim/testdata");
    path.push(filename);
    path
}

/// RAII guard for TCP-based Neovim process cleanup
pub struct NeovimProcessGuard {
    child: Option<std::process::Child>,
    address: String,
}

impl NeovimProcessGuard {
    pub fn new(child: std::process::Child, address: String) -> Self {
        Self {
            child: Some(child),
            address,
        }
    }

    pub fn address(&self) -> &str {
        &self.address
    }
}

impl Drop for NeovimProcessGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if let Err(e) = child.kill() {
                tracing::warn!("Failed to kill Neovim process: {}", e);
            }
            if let Err(e) = child.wait() {
                tracing::warn!("Failed to wait for Neovim process: {}", e);
            }
            debug!("Cleaned up Neovim process at {}", self.address);
        }
    }
}

/// RAII guard for IPC-based Neovim process cleanup (Unix sockets or Windows named pipes)
pub struct NeovimIpcGuard {
    child: Option<std::process::Child>,
    ipc_path: String,
}

impl NeovimIpcGuard {
    pub fn new(child: std::process::Child, ipc_path: String) -> Self {
        Self {
            child: Some(child),
            ipc_path,
        }
    }

    pub fn ipc_path(&self) -> &str {
        &self.ipc_path
    }
}

impl Drop for NeovimIpcGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if let Err(e) = child.kill() {
                tracing::warn!("Failed to kill Neovim process: {}", e);
            }
            if let Err(e) = child.wait() {
                tracing::warn!("Failed to wait for Neovim process: {}", e);
            }
            debug!("Cleaned up Neovim process at {}", self.ipc_path);
        }

        // Clean up socket file on Unix (Windows named pipes are automatically cleaned up)
        #[cfg(unix)]
        {
            if std::path::Path::new(&self.ipc_path).exists() {
                if let Err(e) = std::fs::remove_file(&self.ipc_path) {
                    tracing::warn!("Failed to remove socket file {}: {}", self.ipc_path, e);
                } else {
                    debug!("Removed socket file: {}", self.ipc_path);
                }
            }
        }

        // On Windows, named pipes are automatically cleaned up by the OS
        #[cfg(windows)]
        {
            debug!("Named pipe {} cleaned up by OS", self.ipc_path);
        }
    }
}

/// Setup a Neovim instance with TCP listening
pub async fn setup_neovim_instance_advance(
    port: u16,
    cfg_path: &str,
    open_file: &str,
) -> std::process::Child {
    let listen = format!("{HOST}:{port}");

    let mut child = StdCommand::new(nvim_path())
        .args(["-n", "-u", cfg_path, "--headless", "--listen", &listen])
        .args(
            (!open_file.is_empty())
                .then_some(vec![open_file])
                .unwrap_or_default(),
        )
        .spawn()
        .expect("Failed to start Neovim - ensure nvim is installed and in PATH");

    // Wait for Neovim to start and create the TCP socket
    let start = Instant::now();
    loop {
        sleep(Duration::from_millis(100)).await;

        // Try to connect to see if Neovim is ready
        if tokio::net::TcpStream::connect(&listen).await.is_ok() {
            break;
        }

        if start.elapsed() >= Duration::from_secs(10) {
            let _ = child.kill();
            panic!("Neovim failed to start within 10 seconds at {listen}");
        }
    }

    child
}

/// Setup a basic Neovim instance with TCP listening
pub async fn setup_neovim_instance(port: u16) -> std::process::Child {
    setup_neovim_instance_advance(port, "NONE", "").await
}

/// Setup a Neovim instance with Unix socket listening
#[cfg(unix)]
pub async fn setup_neovim_instance_socket_advance(
    socket_path: &str,
    cfg_path: &str,
    open_file: &str,
) -> std::process::Child {
    let mut child = StdCommand::new(nvim_path())
        .args(["-n", "-u", cfg_path, "--headless", "--listen", socket_path])
        .args(
            (!open_file.is_empty())
                .then_some(vec![open_file])
                .unwrap_or_default(),
        )
        .spawn()
        .expect("Failed to start Neovim - ensure nvim is installed and in PATH");

    // Wait for Neovim to start and create the Unix socket
    let start = Instant::now();
    loop {
        sleep(Duration::from_millis(100)).await;

        // Try to connect to see if Neovim is ready
        if UnixStream::connect(socket_path).await.is_ok() {
            break;
        }

        if start.elapsed() >= Duration::from_secs(10) {
            let _ = child.kill();
            panic!("Neovim failed to start within 10 seconds at {socket_path}");
        }
    }

    child
}

/// Setup a basic Neovim instance with Unix socket listening
#[cfg(unix)]
pub async fn setup_neovim_instance_socket(socket_path: &str) -> std::process::Child {
    setup_neovim_instance_socket_advance(socket_path, "NONE", "").await
}

/// Setup a Neovim instance with Windows named pipe listening
#[cfg(windows)]
pub async fn setup_neovim_instance_pipe_advance(
    pipe_path: &str,
    cfg_path: &str,
    open_file: &str,
) -> std::process::Child {
    let mut child = StdCommand::new(nvim_path())
        .args(&["-u", cfg_path, "--headless", "--listen", pipe_path])
        .args(
            (!open_file.is_empty())
                .then_some(vec![open_file])
                .unwrap_or_default(),
        )
        .spawn()
        .expect("Failed to start Neovim - ensure nvim is installed and in PATH");

    // Wait for Neovim to start and create the named pipe
    let start = Instant::now();
    loop {
        sleep(Duration::from_millis(100)).await;

        // Try to connect to see if Neovim is ready
        if NamedPipeClient::connect(pipe_path).await.is_ok() {
            break;
        }

        if start.elapsed() >= Duration::from_secs(10) {
            let _ = child.kill();
            panic!("Neovim failed to start within 10 seconds at {pipe_path}");
        }
    }

    child
}

/// Setup a basic Neovim instance with Windows named pipe listening
#[cfg(windows)]
pub async fn setup_neovim_instance_pipe(pipe_path: &str) -> std::process::Child {
    setup_neovim_instance_pipe_advance(pipe_path, "NONE", "").await
}

/// Cross-platform IPC setup
#[cfg(unix)]
pub async fn setup_neovim_instance_ipc(ipc_path: &str) -> std::process::Child {
    setup_neovim_instance_socket(ipc_path).await
}

/// Cross-platform IPC setup
#[cfg(windows)]
pub async fn setup_neovim_instance_ipc(ipc_path: &str) -> std::process::Child {
    setup_neovim_instance_pipe(ipc_path).await
}

/// Cross-platform IPC setup with advanced configuration
#[cfg(unix)]
pub async fn setup_neovim_instance_ipc_advance(
    ipc_path: &str,
    cfg_path: &str,
    open_file: &str,
) -> std::process::Child {
    setup_neovim_instance_socket_advance(ipc_path, cfg_path, open_file).await
}

/// Cross-platform IPC setup with advanced configuration
#[cfg(windows)]
pub async fn setup_neovim_instance_ipc_advance(
    ipc_path: &str,
    cfg_path: &str,
    open_file: &str,
) -> std::process::Child {
    setup_neovim_instance_pipe_advance(ipc_path, cfg_path, open_file).await
}

/// Setup a test Neovim instance for MCP server testing
pub async fn setup_test_neovim_instance(
    ipc_path: &str,
) -> Result<NeovimIpcGuard, Box<dyn std::error::Error>> {
    let mut child = StdCommand::new(nvim_path())
        .args(["-u", "NONE", "--headless", "--listen", ipc_path])
        .spawn()
        .map_err(|e| {
            format!("Failed to start Neovim - ensure nvim is installed and in PATH: {e}")
        })?;

    // Wait for Neovim to start and create the socket/pipe
    let start = Instant::now();
    loop {
        sleep(Duration::from_millis(100)).await;

        // Try to connect to see if Neovim is ready
        #[cfg(unix)]
        let can_connect = UnixStream::connect(ipc_path).await.is_ok();
        #[cfg(windows)]
        let can_connect = NamedPipeClient::connect(ipc_path).await.is_ok();

        if can_connect {
            break;
        }

        if start.elapsed() >= Duration::from_secs(10) {
            let _ = child.kill();
            return Err(format!("Neovim failed to start within 10 seconds at {ipc_path}").into());
        }
    }

    debug!("Neovim instance started at {}", ipc_path);
    Ok(NeovimIpcGuard::new(child, ipc_path.to_string()))
}

/// Setup a connected client with TCP connection
pub async fn setup_connected_client(port: u16) -> (impl NeovimClientTrait, NeovimProcessGuard) {
    let child = setup_neovim_instance(port).await;
    let mut client = NeovimClient::new();
    let address = format!("{HOST}:{port}");

    let result = client.connect_tcp(&address).await;
    if result.is_err() {
        // Create guard temporarily to ensure cleanup on failure
        let _guard = NeovimProcessGuard::new(child, address.clone());
        panic!("Failed to connect to Neovim: {result:?}");
    }

    let guard = NeovimProcessGuard::new(child, address);
    (client, guard)
}

/// Setup a connected client with IPC connection
pub async fn setup_connected_client_ipc(
    ipc_path: &str,
) -> (impl NeovimClientTrait, NeovimIpcGuard) {
    let child = setup_neovim_instance_ipc(ipc_path).await;
    let mut client = NeovimClient::new();

    let result = client.connect_path(ipc_path).await;
    if result.is_err() {
        // Create guard temporarily to ensure cleanup on failure
        let _guard = NeovimIpcGuard::new(child, ipc_path.to_string());
        panic!("Failed to connect to Neovim: {result:?}");
    }

    let guard = NeovimIpcGuard::new(child, ipc_path.to_string());
    (client, guard)
}
