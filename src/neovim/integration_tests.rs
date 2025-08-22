use std::fs;

use tempfile::TempDir;
use tracing::info;
use tracing_test::traced_test;

use crate::neovim::client::{DocumentIdentifier, Position, Range};
use crate::neovim::{NeovimClient, NeovimClientTrait, NeovimError};
use crate::test_utils::*;

// Test helper functions to reduce boilerplate

/// Helper function to wait for LSP analysis to complete (LSP ready + diagnostics)
async fn wait_for_lsp_analysis_complete(
    client: &mut NeovimClient<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>,
    timeout_ms: u64,
) -> Result<(), NeovimError> {
    client.wait_for_lsp_ready(None, timeout_ms).await?;
    client.wait_for_diagnostics(None, timeout_ms).await?;
    Ok(())
}

/// Helper for complete LSP setup including autocmd setup and waiting for analysis
async fn setup_lsp_with_analysis(
    client: &mut NeovimClient<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>,
) -> Result<(), NeovimError> {
    client.setup_autocmd().await?;
    wait_for_lsp_analysis_complete(client, 5000).await?;
    Ok(())
}

/// Helper for LSP setup that only waits for LSP readiness (no diagnostics)
async fn setup_lsp_ready_only(
    client: &mut NeovimClient<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>,
) -> Result<(), NeovimError> {
    client.setup_autocmd().await?;
    client.wait_for_lsp_ready(None, 5000).await?;
    Ok(())
}

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

    // Set up LSP and wait for analysis to complete
    setup_lsp_with_analysis(&mut client)
        .await
        .expect("LSP setup and analysis should complete");

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

    // Set up LSP and wait for analysis to complete
    setup_lsp_with_analysis(&mut client)
        .await
        .expect("LSP setup and analysis should complete");

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
    let go_content = get_testdata_content("main.go");

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

    // Set up LSP and wait for readiness
    setup_lsp_ready_only(&mut client)
        .await
        .expect("LSP setup should complete");

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
    let go_content = get_testdata_content("main.go");
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

    // Set up LSP and wait for analysis to complete
    setup_lsp_with_analysis(&mut client)
        .await
        .expect("LSP setup and analysis should complete");

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

            // File operations should be synchronous in Neovim

            // Read the modified content to verify the change
            let modified_content =
                fs::read_to_string(&temp_file_path).expect("Failed to read modified file");
            info!("Modified content:\n{}", modified_content);

            // Verify that the for loop was modernized
            assert!(
                modified_content.contains("for i := range 10"),
                "Expected modernized for loop with 'range 10', got: {modified_content}"
            );
            assert!(
                !modified_content.contains("for i := 0; i < 10; i++"),
                "Original for loop should be replaced, but still found in: {modified_content}"
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

#[tokio::test]
#[traced_test]
async fn test_lsp_definition() {
    // Create a temporary directory and file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_file_path = temp_dir.path().join("test_definition.go");

    // Create a Go file with a function definition and call
    let go_content = r#"package main

import "fmt"

func sayHello(name string) string {
    return "Hello, " + name
}

func main() {
    message := sayHello("World")
    fmt.Println(message)
}
"#;

    fs::write(&temp_file_path, go_content).expect("Failed to write Go file");

    // Setup Neovim with gopls
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

    // Set up LSP and wait for readiness
    setup_lsp_ready_only(&mut client)
        .await
        .expect("LSP setup should complete");

    // Get LSP clients
    let lsp_clients = client.lsp_get_clients().await.unwrap();
    info!("LSP clients: {:?}", lsp_clients);
    assert!(!lsp_clients.is_empty(), "No LSP clients found");

    // Test definition lookup for sayHello function call on line 9 (0-indexed)
    // Position cursor on "sayHello" in the function call
    let result = client
        .lsp_definition(
            "gopls",
            DocumentIdentifier::from_buffer_id(1), // First opened file
            Position {
                line: 9,       // Line with sayHello call
                character: 17, // Position on "sayHello"
            },
        )
        .await;

    assert!(result.is_ok(), "Failed to get definition: {result:?}");
    let definition_result = result.unwrap();
    info!("Definition result found: {:?}", definition_result);
    assert!(
        definition_result.is_some(),
        "Definition result should not be empty"
    );
    let definition_result = definition_result.unwrap();

    // Extract the first location from the definition result
    let first_location = match &definition_result {
        crate::neovim::client::LocateResult::Single(loc) => loc,
        crate::neovim::client::LocateResult::Locations(locs) => {
            assert!(!locs.is_empty(), "No definitions found");
            &locs[0]
        }
        crate::neovim::client::LocateResult::LocationLinks(links) => {
            assert!(!links.is_empty(), "No definitions found");
            // For LocationLinks, we create a Location from the target info
            let link = &links[0];
            assert!(
                link.target_uri.contains("test_definition.go"),
                "Definition should point to the same file"
            );
            // The definition should point to line 4 (0-indexed) where the function is defined
            assert_eq!(
                link.target_range.start.line, 4,
                "Definition should point to line 4 where sayHello function is defined"
            );
            return; // Early return for LocationLinks case
        }
    };

    // For Location cases
    assert!(
        first_location.uri.contains("test_definition.go"),
        "Definition should point to the same file"
    );

    // The definition should point to line 4 (0-indexed) where the function is defined
    assert_eq!(
        first_location.range.start.line, 4,
        "Definition should point to line 4 where sayHello function is defined"
    );

    info!("✅ LSP definition lookup successful!");

    // Temp directory and file automatically cleaned up when temp_dir is dropped
}

#[tokio::test]
#[traced_test]
async fn test_lsp_declaration() {
    // Create a temporary directory and file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_file_path = temp_dir.path().join("test_declaration.zig");

    // Create a Zig file with a function declaration and call
    let zig_content = r#"const std = @import("std");

fn sayHello(allocator: std.mem.Allocator, name: []const u8) ![]u8 {
    return std.fmt.allocPrint(allocator, "Hello, {s}!", .{name});
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const message = try sayHello(allocator, "World");
    defer allocator.free(message);

    std.debug.print("{s}\n", .{message});
}
"#;
    fs::write(&temp_file_path, zig_content).expect("Failed to write Zig file");

    // Setup Neovim with zls (Zig Language Server)
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

    // Set up LSP and wait for analysis to complete
    setup_lsp_with_analysis(&mut client)
        .await
        .expect("LSP setup and analysis should complete");

    // Get LSP clients
    let lsp_clients = client.lsp_get_clients().await.unwrap();
    info!("LSP clients: {:?}", lsp_clients);
    assert!(!lsp_clients.is_empty(), "No LSP clients found");

    // Test declaration lookup for sayHello function call on line 11 (0-indexed)
    // Position cursor on "sayHello" in the function call
    let result = client
        .lsp_declaration(
            "zls",
            DocumentIdentifier::from_buffer_id(1), // First opened file
            Position {
                line: 11,      // Line with sayHello call (updated for Zig file format)
                character: 26, // Position on "sayHello"
            },
        )
        .await;

    assert!(result.is_ok(), "Failed to get declaration: {result:?}");
    let declaration_result = result.unwrap();
    info!("Declaration result found: {:?}", declaration_result);
    assert!(
        declaration_result.is_some(),
        "Declaration result should not be empty"
    );

    let declaration_result = declaration_result.unwrap();
    // Extract the first location from the declaration result
    let first_location = match &declaration_result {
        crate::neovim::client::LocateResult::Single(loc) => loc,
        crate::neovim::client::LocateResult::Locations(locs) => {
            assert!(!locs.is_empty(), "No declarations found");
            &locs[0]
        }
        crate::neovim::client::LocateResult::LocationLinks(links) => {
            assert!(!links.is_empty(), "No declarations found");
            // For LocationLinks, we create a Location from the target info
            let link = &links[0];
            assert!(
                link.target_uri.contains("test_declaration.zig"),
                "Declaration should point to the same file"
            );
            // The declaration should point to line 2 (0-indexed) where the function is declared
            assert_eq!(
                link.target_range.start.line, 2,
                "Declaration should point to line 2 where sayHello function is declared"
            );
            info!("✅ LSP declaration lookup successful!");
            // Temp directory and file automatically cleaned up when temp_dir is dropped
            return;
        }
    };

    // For regular Locations, verify the declaration points to the function declaration
    assert!(
        first_location.uri.contains("test_declaration.zig"),
        "Declaration should point to the same file"
    );
    assert_eq!(
        first_location.range.start.line, 2,
        "Declaration should point to line 2 where sayHello function is declared"
    );

    info!("✅ LSP declaration lookup successful!");
    // Temp directory and file automatically cleaned up when temp_dir is dropped
}

#[tokio::test]
#[traced_test]
async fn test_lsp_type_definition() {
    // Create a temporary directory and file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_file_path = temp_dir.path().join("test_type_definition.go");

    // Create a Go file with a custom type and variable using that type
    let go_content = r#"package main

import "fmt"

type Person struct {
    Name string
    Age  int
}

func main() {
    var user Person
    user.Name = "Alice"
    user.Age = 30
    fmt.Printf("User: %+v\n", user)
}
"#;

    fs::write(&temp_file_path, go_content).expect("Failed to write Go file");

    // Setup Neovim with gopls
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

    // Set up LSP and wait for readiness
    setup_lsp_ready_only(&mut client)
        .await
        .expect("LSP setup should complete");

    // Get LSP clients
    let lsp_clients = client.lsp_get_clients().await.unwrap();
    info!("LSP clients: {:?}", lsp_clients);
    assert!(!lsp_clients.is_empty(), "No LSP clients found");

    // Test type definition lookup for variable "user" on line 10 (0-indexed)
    // Position cursor on "user" variable declaration
    let result = client
        .lsp_type_definition(
            "gopls",
            DocumentIdentifier::from_buffer_id(1), // First opened file
            Position {
                line: 10,     // Line with var user Person
                character: 8, // Position on "user"
            },
        )
        .await;

    assert!(result.is_ok(), "Failed to get type definition: {result:?}");
    let type_definition_result = result.unwrap();
    info!("Type definition result found: {:?}", type_definition_result);
    assert!(
        type_definition_result.is_some(),
        "Type definition result should not be empty"
    );
    let type_definition_result = type_definition_result.unwrap();

    // Extract the first location from the type definition result
    let first_location = match &type_definition_result {
        crate::neovim::client::LocateResult::Single(loc) => loc,
        crate::neovim::client::LocateResult::Locations(locs) => {
            assert!(!locs.is_empty(), "No type definitions found");
            &locs[0]
        }
        crate::neovim::client::LocateResult::LocationLinks(links) => {
            assert!(!links.is_empty(), "No type definitions found");
            // For LocationLinks, we create a Location from the target info
            let link = &links[0];
            assert!(
                link.target_uri.contains("test_type_definition.go"),
                "Type definition should point to the same file"
            );
            // The type definition should point to line 4 (0-indexed) where the Person type is defined
            assert_eq!(
                link.target_range.start.line, 4,
                "Type definition should point to line 4 where Person type is defined"
            );
            return; // Early return for LocationLinks case
        }
    };

    // For Location cases
    assert!(
        first_location.uri.contains("test_type_definition.go"),
        "Type definition should point to the same file"
    );

    // The type definition should point to line 4 (0-indexed) where the Person type is defined
    assert_eq!(
        first_location.range.start.line, 4,
        "Type definition should point to line 4 where Person type is defined"
    );

    info!("✅ LSP type definition lookup successful!");

    // Temp directory and file automatically cleaned up when temp_dir is dropped
}

#[tokio::test]
#[traced_test]
async fn test_lsp_implementation() {
    // Create a temporary directory and file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_file_path = temp_dir.path().join("test_implementation.go");

    // Create a Go file with an interface and its implementation
    let go_content = r#"package main

import "fmt"

type Greeter interface {
    Greet(name string) string
}

type Person struct {
    Title string
}

func (p Person) Greet(name string) string {
    return fmt.Sprintf("Hello %s, I'm %s", name, p.Title)
}

func main() {
    var g Greeter = Person{Title: "Developer"}
    fmt.Println(g.Greet("World"))
}
"#;

    fs::write(&temp_file_path, go_content).expect("Failed to write Go file");

    // Setup Neovim with gopls
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

    // Set up LSP and wait for readiness
    setup_lsp_ready_only(&mut client)
        .await
        .expect("LSP setup should complete");

    // Get LSP clients
    let lsp_clients = client.lsp_get_clients().await.unwrap();
    info!("LSP clients: {:?}", lsp_clients);
    assert!(!lsp_clients.is_empty(), "No LSP clients found");

    // Test implementation lookup for "Greet" method in Greeter interface at line 5 (0-indexed)
    // Position cursor on "Greet" method declaration
    let result = client
        .lsp_implementation(
            "gopls",
            DocumentIdentifier::from_buffer_id(1), // First opened file
            Position {
                line: 5,      // Line with Greet method declaration
                character: 4, // Position on "Greet"
            },
        )
        .await;

    assert!(result.is_ok(), "Failed to get implementation: {result:?}");
    let implementation_result = result.unwrap();
    info!("Implementation result found: {:?}", implementation_result);

    // Implementation results might be empty for interface methods without implementations,
    // or contain the concrete implementations
    if let Some(implementation_result) = implementation_result {
        // Extract the first location from the implementation result
        let first_location = match &implementation_result {
            crate::neovim::client::LocateResult::Single(loc) => loc,
            crate::neovim::client::LocateResult::Locations(locs) => {
                assert!(!locs.is_empty(), "No implementations found");
                &locs[0]
            }
            crate::neovim::client::LocateResult::LocationLinks(links) => {
                assert!(!links.is_empty(), "No implementations found");
                // For LocationLinks, we create a Location from the target info
                let link = &links[0];
                assert!(
                    link.target_uri.contains("test_implementation.go"),
                    "Implementation should point to the same file"
                );
                // The implementation should point to line 12 (0-indexed) where the method is implemented
                assert_eq!(
                    link.target_range.start.line, 12,
                    "Implementation should point to line 12 where Greet method is implemented"
                );
                return; // Early return for LocationLinks case
            }
        };

        // For Location cases
        assert!(
            first_location.uri.contains("test_implementation.go"),
            "Implementation should point to the same file"
        );

        // The implementation should point to line 12 (0-indexed) where the method is implemented
        assert_eq!(
            first_location.range.start.line, 12,
            "Implementation should point to line 12 where Greet method is implemented"
        );
    }

    info!("✅ LSP implementation lookup successful!");

    // Temp directory and file automatically cleaned up when temp_dir is dropped
}

#[tokio::test]
#[traced_test]
#[cfg(any(unix, windows))]
async fn test_lsp_rename_with_prepare() {
    // Create a temporary directory and file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_file_path = temp_dir.path().join("test_main.go");

    // Create a Go file with code that gopls can analyze
    let go_content = get_testdata_content("main.go");
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

    // Set up LSP and wait for readiness
    setup_lsp_ready_only(&mut client)
        .await
        .expect("LSP setup should complete");

    // Get LSP clients
    let lsp_clients = client.lsp_get_clients().await.unwrap();
    let gopls_client = lsp_clients
        .iter()
        .find(|c| c.name == "gopls")
        .expect("gopls client should be available");

    info!("Found gopls client: {:?}", gopls_client);

    // Try to rename the Greet function to GreetUser (line 6, character 5)
    let document = DocumentIdentifier::AbsolutePath(temp_file_path.clone());
    let position = Position {
        line: 6,      // Greet function definition line (0-indexed)
        character: 5, // Position of "Greet" function name
    };

    info!("Testing rename of Greet function to GreetUser...");
    let rename_result = client
        .lsp_rename("gopls", document, position, "GreetUser")
        .await;

    info!("Rename result: {:?}", rename_result);

    if let Ok(Some(workspace_edit)) = rename_result {
        info!("✅ LSP rename successful!");
        info!("Workspace edit: {:?}", workspace_edit);

        // Apply the workspace edit to test the functionality
        info!("Applying workspace edit...");
        let apply_result = client
            .lsp_apply_workspace_edit("gopls", workspace_edit)
            .await;

        assert!(
            apply_result.is_ok(),
            "Failed to apply workspace edit: {:?}",
            apply_result
        );
        info!("✅ LSP workspace edit applied successfully!");

        // Save the buffer to persist changes to disk
        let result = client.execute_lua("vim.cmd('write')").await;
        assert!(result.is_ok(), "Failed to save buffer: {result:?}");

        // File operations should be synchronous in Neovim

        // Read the file content to verify the rename was applied
        let updated_content =
            fs::read_to_string(&temp_file_path).expect("Failed to read updated file");
        info!("Updated content:\n{}", updated_content);

        // The function name should have been changed from "Greet" to "GreetUser"
        assert!(
            updated_content.contains("GreetUser"),
            "File should contain the new function name 'GreetUser'"
        );
        assert!(
            !updated_content.contains("func Greet("),
            "File should no longer contain the old function signature 'func Greet('"
        );
    } else {
        panic!("⚠️ LSP rename not supported or position not renameable");
    }

    // Temp directory and file automatically cleaned up when temp_dir is dropped
}

#[tokio::test]
#[traced_test]
#[cfg(any(unix, windows))]
async fn test_lsp_rename_without_prepare() {
    // Create a temporary directory and file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_file_path = temp_dir.path().join("test_main.go");

    // Create a Go file with code that gopls can analyze
    let go_content = get_testdata_content("main.go");
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

    // Set up LSP and wait for readiness
    setup_lsp_ready_only(&mut client)
        .await
        .expect("LSP setup should complete");

    // Get LSP clients
    let lsp_clients = client.lsp_get_clients().await.unwrap();
    let gopls_client = lsp_clients
        .iter()
        .find(|c| c.name == "gopls")
        .expect("gopls client should be available");

    info!("Found gopls client: {:?}", gopls_client);

    // Try to rename the Greet function to SayHello WITHOUT prepare rename (line 6, character 5)
    let document = DocumentIdentifier::AbsolutePath(temp_file_path.clone());
    let position = Position {
        line: 6,      // Greet function definition line (0-indexed)
        character: 5, // Position of "Greet" function name
    };

    info!("Testing rename of Greet function to SayHello (without prepare)...");
    let rename_result = client
        .lsp_rename("gopls", document, position, "SayHello")
        .await;

    info!("Rename result: {:?}", rename_result);

    if let Ok(Some(workspace_edit)) = rename_result {
        info!("✅ LSP rename successful!");
        info!("Workspace edit: {:?}", workspace_edit);

        // Apply the workspace edit to test the functionality
        info!("Applying workspace edit...");
        let apply_result = client
            .lsp_apply_workspace_edit("gopls", workspace_edit)
            .await;

        assert!(
            apply_result.is_ok(),
            "Failed to apply workspace edit: {:?}",
            apply_result
        );
        info!("✅ LSP workspace edit applied successfully!");

        // Save the buffer to persist changes to disk
        let result = client.execute_lua("vim.cmd('write')").await;
        assert!(result.is_ok(), "Failed to save buffer: {result:?}");

        // File operations should be synchronous in Neovim

        // Read the file content to verify the rename was applied
        let updated_content =
            fs::read_to_string(&temp_file_path).expect("Failed to read updated file");
        info!("Updated content:\n{}", updated_content);

        // The function name should have been changed from "Greet" to "SayHello"
        assert!(
            updated_content.contains("SayHello"),
            "File should contain the new function name 'SayHello'"
        );
        assert!(
            !updated_content.contains("func Greet("),
            "File should no longer contain the old function signature 'func Greet('"
        );
    } else {
        panic!("⚠️ LSP rename not supported or position not renameable");
    }

    // Temp directory and file automatically cleaned up when temp_dir is dropped
}

// Helper function to set up Neovim instance with LSP for formatting tests
async fn setup_formatting_test_helper() -> (
    TempDir,
    NeovimIpcGuard,
    NeovimClient<tokio::net::UnixStream>,
) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_file_path = temp_dir.path().join("test_formatting_split.ts");
    // Create a poorly formatted TypeScript file that needs formatting
    let unformatted_ts_content = r#"import {Console} from 'console';
import    * as fs from 'fs';

interface User{
name:string;
age:number;
}

class UserService{
constructor(private users:User[]=[]){}

addUser(user:User):void{
this.users.push(user);
}

getUsers():User[]{
return this.users;
}
}

function main(){
const service=new UserService();
const user:User={name:"Alice",age:30};
service.addUser(user);
console.log(service.getUsers());
}

main();
"#;

    fs::write(&temp_file_path, unformatted_ts_content)
        .expect("Failed to write temp TypeScript file");

    let ipc_path = generate_random_ipc_path();
    let child = setup_neovim_instance_ipc_advance(
        &ipc_path,
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        temp_file_path.to_str().unwrap(),
    )
    .await;
    let guard = NeovimIpcGuard::new(child, ipc_path.clone());
    let mut client = NeovimClient::new();

    // Connect to instance
    let result = client.connect_path(&ipc_path).await;
    assert!(result.is_ok(), "Failed to connect to instance");

    // Set up LSP and wait for analysis to complete
    setup_lsp_with_analysis(&mut client)
        .await
        .expect("LSP setup and analysis should complete");

    (temp_dir, guard, client)
}

#[tokio::test]
#[traced_test]
async fn test_lsp_formatting_and_apply_edits() {
    let (_temp_dir, _guard, client) = setup_formatting_test_helper().await;

    use crate::neovim::FormattingOptions;
    let tab_options = FormattingOptions {
        tab_size: 4,
        insert_spaces: false,
        trim_trailing_whitespace: Some(true),
        insert_final_newline: Some(true),
        trim_final_newlines: Some(false),
        extras: std::collections::HashMap::new(),
    };

    // First get the text edits
    let result = client
        .lsp_formatting("ts_ls", DocumentIdentifier::from_buffer_id(0), tab_options)
        .await;

    assert!(result.is_ok(), "Failed to format with tabs: {result:?}");
    let text_edits = result.unwrap();
    assert!(!text_edits.is_empty(), "Should have text edits to apply");

    // Apply the text edits
    let apply_result = client
        .lsp_apply_text_edits(
            "ts_ls",
            DocumentIdentifier::from_buffer_id(0),
            text_edits.clone(),
        )
        .await;

    assert!(
        apply_result.is_ok(),
        "Failed to apply text edits: {apply_result:?}"
    );
    info!(
        "✅ Successfully applied {} text edits using lsp_apply_text_edits",
        text_edits.len()
    );

    // Verify the buffer content changed by checking it
    let content_check = client
        .execute_lua(r#"return table.concat(vim.api.nvim_buf_get_lines(0, 0, -1, false), "\n")"#)
        .await;

    assert!(
        content_check.is_ok(),
        "Failed to get buffer content after applying text edits: {content_check:?}"
    );
    info!("✅ Buffer content updated after applying text edits");
}

#[tokio::test]
#[traced_test]
async fn test_lsp_apply_text_edits() {
    // Create a temporary directory and file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_file_path = temp_dir.path().join("test_apply_edits.ts");

    // Create a poorly formatted TypeScript file that needs formatting
    let unformatted_ts_content = r#"import {Console} from 'console';
function main(){
console.log("Hello World")
}
main();
"#;
    fs::write(&temp_file_path, unformatted_ts_content)
        .expect("Failed to write temp TypeScript file");

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

    // Set up LSP and wait for analysis to complete
    setup_lsp_with_analysis(&mut client)
        .await
        .expect("LSP setup and analysis should complete");

    use crate::neovim::FormattingOptions;
    let tab_options = FormattingOptions {
        tab_size: 4,
        insert_spaces: false,
        trim_trailing_whitespace: Some(true),
        insert_final_newline: Some(true),
        trim_final_newlines: Some(false),
        extras: std::collections::HashMap::new(),
    };

    // First get the original buffer content
    let original_content = client
        .execute_lua(r#"return table.concat(vim.api.nvim_buf_get_lines(0, 0, -1, false), "\n")"#)
        .await;
    assert!(
        original_content.is_ok(),
        "Failed to get original buffer content: {original_content:?}"
    );
    let original = original_content.unwrap().as_str().unwrap().to_string();
    info!("Original buffer content length: {}", original.len());

    // Test 1: Get text edits first (without applying)
    let text_edits_result = client
        .lsp_formatting(
            "ts_ls",
            DocumentIdentifier::from_buffer_id(0),
            tab_options.clone(),
        )
        .await;
    assert!(
        text_edits_result.is_ok(),
        "Failed to get text edits for formatting: {text_edits_result:?}"
    );

    let text_edits = text_edits_result.unwrap();
    info!("✅ Got {} text edits for formatting", text_edits.len());

    // Test 2: Apply the text edits
    let apply_result = client
        .lsp_apply_text_edits("ts_ls", DocumentIdentifier::from_buffer_id(0), text_edits)
        .await;

    assert!(
        apply_result.is_ok(),
        "Failed to apply text edits: {apply_result:?}"
    );
    info!("✅ Successfully applied text edits");

    // Test 3: Verify the buffer content changed
    let new_content = client
        .execute_lua(r#"return table.concat(vim.api.nvim_buf_get_lines(0, 0, -1, false), "\n")"#)
        .await;
    assert!(
        new_content.is_ok(),
        "Failed to get new buffer content after applying text edits: {new_content:?}"
    );
    let new = new_content.unwrap().as_str().unwrap().to_string();
    info!("New buffer content length: {}", new.len());

    // The content should be different after formatting
    assert_ne!(
        original, new,
        "Buffer content should have changed after applying text edits"
    );

    // Temp directory and file automatically cleaned up when temp_dir is dropped
}

#[tokio::test]
#[traced_test]
async fn test_lsp_range_formatting_and_apply_edits() {
    let (_temp_dir, _guard, client) = setup_formatting_test_helper().await;

    use crate::neovim::FormattingOptions;
    let tab_options = FormattingOptions {
        tab_size: 4,
        insert_spaces: false,
        trim_trailing_whitespace: Some(true),
        insert_final_newline: Some(true),
        trim_final_newlines: Some(false),
        extras: std::collections::HashMap::new(),
    };

    // First get the text edits for a specific range
    let range = Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 5,
            character: 0,
        }, // First few lines
    };
    let result = client
        .lsp_range_formatting(
            "ts_ls",
            DocumentIdentifier::from_buffer_id(0),
            range,
            tab_options,
        )
        .await;

    assert!(
        result.is_ok(),
        "Failed to format range with tabs: {result:?}"
    );
    let text_edits = result.unwrap();
    assert!(
        !text_edits.is_empty(),
        "Should have text edits to apply for range formatting"
    );

    // get the original buffer content
    let original_content = client
        .execute_lua(r#"return table.concat(vim.api.nvim_buf_get_lines(0, 0, -1, false), "\n")"#)
        .await;
    assert!(
        original_content.is_ok(),
        "Failed to get original buffer content: {original_content:?}"
    );
    let original = original_content.unwrap().as_str().unwrap().to_string();
    info!("Original buffer content length: {}", original.len());

    // Apply the text edits
    let apply_result = client
        .lsp_apply_text_edits(
            "ts_ls",
            DocumentIdentifier::from_buffer_id(0),
            text_edits.clone(),
        )
        .await;

    assert!(
        apply_result.is_ok(),
        "Failed to apply text edits from range formatting: {apply_result:?}"
    );
    info!(
        "✅ Successfully applied {} text edits from range formatting using lsp_apply_text_edits",
        text_edits.len()
    );

    // Verify the buffer content changed by checking it
    let new_content = client
        .execute_lua(r#"return table.concat(vim.api.nvim_buf_get_lines(0, 0, -1, false), "\n")"#)
        .await;

    assert!(
        new_content.is_ok(),
        "Failed to get buffer content after applying range text edits: {new_content:?}"
    );
    info!("✅ Buffer content updated after applying range text edits");
    let new = new_content.unwrap().as_str().unwrap().to_string();
    info!("New buffer content length: {}", new.len());

    // The content should be different after formatting
    assert_ne!(
        original, new,
        "Buffer content should have changed after applying text edits"
    );
}
