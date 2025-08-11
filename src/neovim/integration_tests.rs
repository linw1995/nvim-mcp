use std::time::Duration;

use tokio::time::sleep;
use tracing::info;
use tracing_test::traced_test;

use crate::neovim::client::{DocumentIdentifier, Position, Range};
use crate::neovim::{NeovimClient, NeovimClientTrait};
use crate::test_utils::*;

#[tokio::test]
#[traced_test]
async fn test_tcp_connection_lifecycle() {
    let port = PORT_BASE;
    let address = format!("{HOST}:{port}");

    let child = {
        let _guard = NEOVIM_TEST_MUTEX.lock().unwrap();
        drop(_guard);
        setup_neovim_instance(port).await
    };
    let _guard = NeovimProcessGuard::new(child, address.clone());
    let mut client = NeovimClient::new();

    // Test connection
    let result = client.connect_tcp(&address).await;
    assert!(result.is_ok(), "Failed to connect: {result:?}");

    // Test that we can't connect again while already connected
    let result = client.connect_tcp(&address).await;
    assert!(result.is_err(), "Should not be able to connect twice");

    // Test disconnect
    let result = client.disconnect().await;
    assert!(result.is_ok(), "Failed to disconnect: {result:?}");

    // Test that disconnect fails when not connected
    let result = client.disconnect().await;
    assert!(
        result.is_err(),
        "Should not be able to disconnect when not connected"
    );

    // Guard automatically cleans up when it goes out of scope
}

#[tokio::test]
#[traced_test]
#[cfg(any(unix, windows))]
async fn test_buffer_operations() {
    let ipc_path = generate_random_ipc_path();

    let (client, _guard) = setup_connected_client_ipc(&ipc_path).await;

    // Test buffer listing
    let result = client.get_buffers().await;
    assert!(result.is_ok(), "Failed to get buffers: {result:?}");

    let buffer_info = result.unwrap();
    assert!(!buffer_info.is_empty());

    // Should have at least one buffer (the initial empty buffer)
    let first_buffer = &buffer_info[0];
    assert!(
        first_buffer.id > 0,
        "Buffer should have valid id: {first_buffer:?}"
    );
    // Line count should be reasonable (buffers typically have at least 1 line)
    assert!(
        first_buffer.line_count > 0,
        "Buffer should have at least one line: {first_buffer:?}"
    );

    // Guard automatically cleans up when it goes out of scope
}

#[tokio::test]
#[traced_test]
#[cfg(any(unix, windows))]
async fn test_lua_execution() {
    let ipc_path = generate_random_ipc_path();

    let (client, _guard) = setup_connected_client_ipc(&ipc_path).await;

    // Test successful Lua execution
    let result = client.execute_lua("return 42").await;
    assert!(result.is_ok(), "Failed to execute Lua: {result:?}");

    let lua_result = result.unwrap();
    assert!(
        format!("{lua_result:?}").contains("42"),
        "Lua result should contain 42: {lua_result:?}"
    );

    // Test Lua execution with string result
    let result = client.execute_lua("return 'hello world'").await;
    assert!(result.is_ok(), "Failed to execute Lua: {result:?}");

    // Test error handling for invalid Lua
    let result = client.execute_lua("invalid lua syntax !!!").await;
    assert!(result.is_err(), "Should fail for invalid Lua syntax");

    // Test error handling for empty code
    let result = client.execute_lua("").await;
    assert!(result.is_err(), "Should fail for empty Lua code");

    // Guard automatically cleans up when it goes out of scope
}

#[tokio::test]
#[traced_test]
#[cfg(any(unix, windows))]
async fn test_error_handling() {
    #[cfg(unix)]
    use tokio::net::UnixStream;
    #[cfg(windows)]
    use tokio::net::windows::named_pipe::NamedPipeClient;
    #[cfg(unix)]
    let client = NeovimClient::<UnixStream>::new();
    #[cfg(windows)]
    let client = NeovimClient::<NamedPipeClient>::new();

    // Test operations without connection
    let result = client.get_buffers().await;
    assert!(
        result.is_err(),
        "get_buffers should fail when not connected"
    );

    let result = client.execute_lua("return 1").await;
    assert!(
        result.is_err(),
        "execute_lua should fail when not connected"
    );

    let mut client_mut = client;
    let result = client_mut.disconnect().await;
    assert!(result.is_err(), "disconnect should fail when not connected");
}

#[tokio::test]
#[traced_test]
#[cfg(any(unix, windows))]
async fn test_connection_constraint() {
    let ipc_path = generate_random_ipc_path();

    let child = setup_neovim_instance_ipc(&ipc_path).await;
    let _guard = NeovimIpcGuard::new(child, ipc_path.clone());
    let mut client = NeovimClient::new();

    // Connect to instance
    let result = client.connect_path(&ipc_path).await;
    assert!(result.is_ok(), "Failed to connect to instance");

    // Try to connect again (should fail)
    let result = client.connect_path(&ipc_path).await;
    assert!(result.is_err(), "Should not be able to connect twice");

    // Disconnect and then connect again (should work)
    let result = client.disconnect().await;
    assert!(result.is_ok(), "Failed to disconnect from instance");

    let result = client.connect_path(&ipc_path).await;
    assert!(result.is_ok(), "Failed to reconnect after disconnect");

    // Guard automatically cleans up when it goes out of scope
}

#[tokio::test]
#[traced_test]
#[cfg(any(unix, windows))]
async fn test_get_vim_diagnostics() {
    let ipc_path = generate_random_ipc_path();

    let child = setup_neovim_instance_ipc_advance(
        &ipc_path,
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        get_testdata_path("diagnostic_problems.lua")
            .to_str()
            .unwrap(),
    )
    .await;
    let _guard = NeovimIpcGuard::new(child, ipc_path.clone());
    let mut client = NeovimClient::new();

    // Connect to instance
    let result = client.connect_path(&ipc_path).await;
    assert!(result.is_ok(), "Failed to connect to instance");

    // Set up diagnostics and get diagnostics for buffer 0
    let result = client.setup_diagnostics_changed_autocmd().await;
    assert!(
        result.is_ok(),
        "Failed to setup diagnostics autocmd: {result:?}"
    );

    sleep(Duration::from_secs(20)).await; // Allow time for LSP to initialize

    let result = client.get_buffer_diagnostics(0).await;
    assert!(result.is_ok(), "Failed to get diagnostics: {result:?}");

    // Guard automatically cleans up when it goes out of scope
}

#[tokio::test]
#[traced_test]
#[cfg(any(unix, windows))]
async fn test_code_action() {
    let ipc_path = generate_random_ipc_path();

    let child = setup_neovim_instance_ipc_advance(
        &ipc_path,
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        get_testdata_path("diagnostic_problems.lua")
            .to_str()
            .unwrap(),
    )
    .await;
    let _guard = NeovimIpcGuard::new(child, ipc_path.clone());
    let mut client = NeovimClient::new();

    // Connect to instance
    let result = client.connect_path(&ipc_path).await;
    assert!(result.is_ok(), "Failed to connect to instance");

    // Set up diagnostics and wait for LSP
    let result = client.setup_diagnostics_changed_autocmd().await;
    assert!(
        result.is_ok(),
        "Failed to setup diagnostics autocmd: {result:?}"
    );

    sleep(Duration::from_secs(20)).await; // Allow time for LSP to initialize

    let result = client.get_buffer_diagnostics(0).await;
    assert!(result.is_ok(), "Failed to get diagnostics: {result:?}");
    let result = result.unwrap();
    info!("Diagnostics: {:?}", result);

    let diagnostic = result.first().expect("Failed to get any diagnostics");
    let result = client
        .lsp_get_code_actions(
            "luals",
            DocumentIdentifier::from_buffer_id(0),
            Range {
                start: Position {
                    line: diagnostic.lnum,
                    character: diagnostic.col,
                },
                end: Position {
                    line: diagnostic.end_lnum,
                    character: diagnostic.end_col,
                },
            },
        )
        .await;
    assert!(result.is_ok(), "Failed to get code actions: {result:?}");
    info!("Code actions: {:?}", result);

    // Guard automatically cleans up when it goes out of scope
}
