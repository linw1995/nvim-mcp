use std::fs;
use std::time::Duration;

use tempfile::TempDir;
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

#[tokio::test]
#[traced_test]
#[cfg(any(unix, windows))]
async fn test_lsp_resolve_code_action() {
    // Create a temporary directory and file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_file_path = temp_dir.path().join("test_resolve.go");

    // Create a Go file with fmt.Println call that can be inlined
    let go_content = include_str!("testdata/main.go");

    fs::write(&temp_file_path, go_content).expect("Failed to write temp Go file");

    let ipc_path = generate_random_ipc_path();
    let child = setup_neovim_instance_ipc_advance(
        &ipc_path,
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        temp_file_path.to_str().unwrap(),
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

    // Position cursor inside fmt.Println call (line 6, character 6)
    let result = client
        .lsp_get_code_actions(
            "gopls",
            DocumentIdentifier::from_buffer_id(0),
            Range {
                start: Position {
                    line: 6,      // Inside fmt.Println call
                    character: 6, // After fmt.P
                },
                end: Position {
                    line: 6,
                    character: 6,
                },
            },
        )
        .await;
    assert!(result.is_ok(), "Failed to get code actions: {result:?}");
    let code_actions = result.unwrap();
    info!("Code actions: {:?}", code_actions);

    // Find the "Inline call to Println" action which requires resolution
    let inline_action = code_actions
        .iter()
        .find(|action| action.title().contains("Inline call to Println"));

    if let Some(action) = inline_action {
        info!("Found inline action: {:?}", action.title());

        // Verify this action needs resolution (no edit, has data)
        assert!(
            action.edit().is_none(),
            "Action should not have edit before resolution"
        );

        // Test resolving the code action
        let code_action_json = serde_json::to_string(action).unwrap();
        let code_action_copy: crate::neovim::CodeAction =
            serde_json::from_str(&code_action_json).unwrap();

        let result = client
            .lsp_resolve_code_action("gopls", code_action_copy)
            .await;
        assert!(result.is_ok(), "Failed to resolve code action: {result:?}");
        let resolved_action = result.unwrap();
        info!("Resolved code action: {:?}", resolved_action);

        // Verify the action was properly resolved
        assert!(
            resolved_action.edit().is_some(),
            "Resolved action should have edit field populated"
        );

        let resolved_edit = resolved_action.edit().unwrap();
        let edit_json = serde_json::to_string(resolved_edit).unwrap();
        info!("Resolved workspace edit: {}", edit_json);

        // Verify the edit contains expected transformations for inlining fmt.Println
        assert!(
            edit_json.contains("Fp"),
            "Resolved edit should contain Fp (Printf) transformation"
        );
        assert!(
            edit_json.contains("os.Stdout"),
            "Resolved edit should contain os.Stdout parameter"
        );
        assert!(
            edit_json.contains("\\t\\\"os\\\""),
            "Resolved edit should add os import"
        );

        info!("✅ Code action resolution validated successfully!");
    } else {
        // List available actions for debugging
        info!("Inline action not found, available actions:");
        for (i, action) in code_actions.iter().enumerate() {
            info!("  Action {}: {}", i, action.title());
        }
        panic!("Expected 'Inline call to Println' action not found");
    }

    // Temp directory and file automatically cleaned up when temp_dir is dropped
}

#[tokio::test]
#[traced_test]
#[cfg(any(unix, windows))]
async fn test_lsp_apply_workspace_edit() {
    // Create a temporary directory and file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_file_path = temp_dir.path().join("test_main.go");

    // Create a Go file with code that gopls will want to modernize
    let go_content = include_str!("testdata/main.go");
    fs::write(&temp_file_path, go_content).expect("Failed to write temp Go file");

    let ipc_path = generate_random_ipc_path();
    let child = setup_neovim_instance_ipc_advance(
        &ipc_path,
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        temp_file_path.to_str().unwrap(),
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

    // Get buffer diagnostics to find modernization opportunities
    let result = client.get_buffer_diagnostics(0).await;
    assert!(result.is_ok(), "Failed to get diagnostics: {result:?}");
    let diagnostics = result.unwrap();
    info!("Diagnostics: {:?}", diagnostics);

    if let Some(diagnostic) = diagnostics.first() {
        // Get code actions for the diagnostic range
        let result = client
            .lsp_get_code_actions(
                "gopls",
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
        let code_actions = result.unwrap();
        info!("Code actions: {:?}", code_actions);

        // Find the "Replace for loop with range" action that has a workspace edit
        let modernize_action = code_actions.iter().find(|action| {
            action.title().contains("Replace for loop with range") && action.has_edit()
        });

        if let Some(action) = modernize_action {
            info!("Found modernize action: {:?}", action.title());

            // Extract the workspace edit from the code action
            let workspace_edit = action.edit().unwrap().clone();
            info!("Workspace edit to apply: {:?}", workspace_edit);

            // Read original content
            let original_content =
                fs::read_to_string(&temp_file_path).expect("Failed to read original file");
            info!("Original content:\n{}", original_content);

            // Apply the workspace edit using the client
            let result = client
                .lsp_apply_workspace_edit("gopls", workspace_edit)
                .await;
            assert!(result.is_ok(), "Failed to apply workspace edit: {result:?}");

            // Save the buffer to persist changes to disk
            let result = client.execute_lua("vim.cmd('write')").await;
            assert!(result.is_ok(), "Failed to save buffer: {result:?}");

            // Give some time for the edit and save to be applied
            sleep(Duration::from_millis(1000)).await;

            // Read the modified content to verify the change
            let modified_content =
                fs::read_to_string(&temp_file_path).expect("Failed to read modified file");
            info!("Modified content:\n{}", modified_content);

            // Verify that the for loop was modernized
            assert!(
                modified_content.contains("for i := range 10"),
                "Expected modernized for loop with 'range 10', got: {}",
                modified_content
            );
            assert!(
                !modified_content.contains("for i := 0; i < 10; i++"),
                "Original for loop should be replaced, but still found in: {}",
                modified_content
            );

            info!("✅ Workspace edit successfully applied and verified!");
        } else {
            info!("No modernize action with workspace edit found, available actions:");
            for action in &code_actions {
                info!("  - {}: edit={}", action.title(), action.has_edit());
            }
            panic!("Expected 'Replace for loop with range' action with workspace edit not found");
        }
    } else {
        info!("No diagnostics found for modernization");
    }

    // Temp directory and file automatically cleaned up when temp_dir is dropped
}
