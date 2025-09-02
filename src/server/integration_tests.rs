use rmcp::{
    model::{CallToolRequestParam, ReadResourceRequestParam},
    serde_json::{Map, Value},
    service::ServiceExt,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use tokio::process::Command;
use tracing::{error, info};
use tracing_test::traced_test;

use crate::{server::core::b3sum, test_utils::*};

// Macro to create an MCP service using the pre-compiled binary
macro_rules! create_mcp_service {
    () => {{
        let command = Command::new(get_compiled_binary()).configure(|cmd| {
            cmd.args(["--connect", "manual"]);
        });
        ().serve(TokioChildProcess::new(command)?)
            .await
            .map_err(|e| {
                error!("Failed to connect to server: {}", e);
                e
            })?
    }};
    ($target:expr) => {{
        // Macro to create an MCP service with auto-connect to a specific target
        let command = Command::new(get_compiled_binary()).configure(|cmd| {
            cmd.args(["--connect", $target]);
        });
        ().serve(TokioChildProcess::new(command)?)
            .await
            .map_err(|e| {
                error!("Failed to connect to server: {}", e);
                e
            })?
    }};
}

/// Macro to setup auto-connected service and return (service, connection_id, guard)
/// This eliminates the boilerplate for auto-connect tests
macro_rules! setup_connected_service {
    () => {{
        let ipc_path = generate_random_ipc_path();
        let _guard = setup_test_neovim_instance(&ipc_path).await?;
        let service = create_mcp_service!(&ipc_path);
        let connection_id = b3sum(&ipc_path)[..7].to_string();
        (service, connection_id, _guard)
    }};
    ($cfg_path:expr, $open_file:expr) => {{
        let ipc_path = generate_random_ipc_path();
        let child = setup_neovim_instance_socket_advance(&ipc_path, $cfg_path, $open_file).await;
        let _guard = NeovimIpcGuard::new(child, ipc_path.clone());
        let service = create_mcp_service!(&ipc_path);
        let connection_id = b3sum(&ipc_path)[..7].to_string();
        (service, connection_id, _guard)
    }};
}

/// Macro to connect to a Neovim instance through MCP service and get connection_id
macro_rules! connect_to_neovim {
    ($service:expr, $ipc_path:expr) => {{
        let mut connect_args = Map::new();
        connect_args.insert("target".to_string(), Value::String($ipc_path));

        let connect_result = $service
            .call_tool(CallToolRequestParam {
                name: "connect".into(),
                arguments: Some(connect_args),
            })
            .await?;

        extract_connection_id(&connect_result)?
    }};
}

// Helper function to extract connection_id from connect response
fn extract_connection_id(
    result: &rmcp::model::CallToolResult,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(content) = result.content.first() {
        // The content should be JSON
        let json_str = match &content.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => return Err("Expected text content".into()),
        };

        // Parse JSON
        let json_value: serde_json::Value = serde_json::from_str(json_str)?;
        if let Some(connection_id) = json_value["connection_id"].as_str() {
            return Ok(connection_id.to_string());
        }
    }
    Err("Failed to extract connection_id from response".into())
}

#[traced_test]
#[tokio::test]
async fn test_graceful_close_mcp_server() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP server using pre-compiled binary");

    let mut child = Command::new(get_compiled_binary())
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    // Wait a moment to ensure the server starts
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Check if the process is still running
    match child.try_wait()? {
        Some(status) => {
            error!("MCP server exited prematurely with status: {}", status);
            return Err("MCP server exited prematurely".into());
        }
        None => {
            info!("MCP server is running");
        }
    }

    // Clean up: terminate the server process
    // Close stdin
    if let Some(stdin) = child.stdin.take() {
        drop(stdin);
    }
    // Or send SIGTERM signal

    child.wait().await?;
    info!("MCP server process terminated");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_mcp_server_connection() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP client to test nvim-mcp server");

    // Connect to the server using pre-compiled binary
    let service = create_mcp_service!();

    // Get server information
    let server_info = service.peer_info();
    info!("Connected to server: {:#?}", server_info);

    // Verify server info contains expected information
    if let Some(info) = server_info {
        assert!(info.instructions.is_some());
        if let Some(ref instructions) = info.instructions {
            assert!(instructions.contains("Neovim"));
        }

        // Verify server capabilities
        assert!(info.capabilities.tools.is_some());
    } else {
        panic!("Expected server info to be present");
    }

    // Gracefully close the connection
    service.cancel().await?;
    info!("MCP server connection test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_list_tools() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP client to test nvim-mcp server");

    let service = create_mcp_service!();

    // List available tools
    let tools = service.list_tools(Default::default()).await?;
    info!("Available tools: {:#?}", tools);

    // Verify we have the expected tools
    let tool_names: Vec<&str> = tools.tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(tool_names.contains(&"connect"));
    assert!(tool_names.contains(&"connect_tcp"));
    assert!(tool_names.contains(&"disconnect"));
    assert!(tool_names.contains(&"list_buffers"));
    assert!(tool_names.contains(&"lsp_clients"));
    assert!(tool_names.contains(&"lsp_references"));

    // Verify tool descriptions are present
    for tool in &tools.tools {
        assert!(tool.description.is_some());
        assert!(!tool.description.as_ref().unwrap().is_empty());
    }

    service.cancel().await?;
    info!("List tools test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_connect_nvim() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP client to test nvim-mcp server");

    let service = create_mcp_service!();

    // Start a test Neovim instance
    let ipc_path = generate_random_ipc_path();
    let _guard = setup_test_neovim_instance(&ipc_path).await?;

    // Create arguments as Map (based on rmcp expectations)
    let mut arguments = Map::new();
    arguments.insert("target".to_string(), Value::String(ipc_path.clone()));

    // Test successful connection
    let result = service
        .call_tool(CallToolRequestParam {
            name: "connect".into(),
            arguments: Some(arguments),
        })
        .await?;

    info!("Connect result: {:#?}", result);
    assert!(!result.content.is_empty());

    // Verify the response contains success message
    if let Some(content) = result.content.first() {
        if let Some(text) = content.as_text() {
            assert!(text.text.contains("connection_id"));
        } else {
            panic!("Expected text content in connect result");
        }
    } else {
        panic!("No content in connect result");
    }

    // Test that connecting again succeeds (IPC connections allow reconnection)
    let mut arguments2 = Map::new();
    arguments2.insert("target".to_string(), Value::String(ipc_path.clone()));

    let result = service
        .call_tool(CallToolRequestParam {
            name: "connect".into(),
            arguments: Some(arguments2),
        })
        .await;

    // For IPC connections, we allow reconnection to the same path
    assert!(
        result.is_ok(),
        "Should be able to reconnect to the same IPC path"
    );

    // Cleanup happens automatically via guard
    service.cancel().await?;
    info!("Connect nvim tool test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_invalid_connection_id_handling() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP client to test invalid connection ID handling");

    let service = create_mcp_service!();

    // Test that all connection-aware tools fail with invalid connection ID
    let invalid_connection_id = "invalid_connection_id".to_string();
    let tools_to_test = vec![
        "disconnect",
        "list_buffers",
        "exec_lua",
        "lsp_clients",
        "cursor_position",
        "navigate",
    ];

    for tool_name in tools_to_test {
        let mut args = Map::new();
        args.insert(
            "connection_id".to_string(),
            Value::String(invalid_connection_id.clone()),
        );

        // Add tool-specific required arguments
        match tool_name {
            "exec_lua" => {
                args.insert("code".to_string(), Value::String("return 42".to_string()));
            }
            "navigate" => {
                args.insert("document".to_string(), serde_json::json!({"buffer_id": 1}));
                args.insert("line".to_string(), Value::Number(0.into()));
                args.insert("character".to_string(), Value::Number(0.into()));
            }
            _ => {}
        }

        let result = service
            .call_tool(CallToolRequestParam {
                name: tool_name.into(),
                arguments: Some(args),
            })
            .await;

        assert!(
            result.is_err(),
            "{} should fail with invalid connection ID",
            tool_name
        );
    }

    service.cancel().await?;
    info!("Invalid connection ID handling test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_disconnect_nvim() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP client to test nvim-mcp server");

    let service = create_mcp_service!();

    // Now connect first, then test disconnect
    let ipc_path = generate_random_ipc_path();
    let _guard = setup_test_neovim_instance(&ipc_path).await?;

    // Connect first
    let mut connect_args = Map::new();
    connect_args.insert("target".to_string(), Value::String(ipc_path.clone()));

    let connect_result = service
        .call_tool(CallToolRequestParam {
            name: "connect".into(),
            arguments: Some(connect_args),
        })
        .await?;

    let connection_id = extract_connection_id(&connect_result)?;

    // Now test successful disconnect
    let mut disconnect_args = Map::new();
    disconnect_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );

    let result = service
        .call_tool(CallToolRequestParam {
            name: "disconnect".into(),
            arguments: Some(disconnect_args),
        })
        .await?;

    info!("Disconnect result: {:#?}", result);
    assert!(!result.content.is_empty());

    // Verify the response contains success message
    if let Some(content) = result.content.first() {
        if let Some(text) = content.as_text() {
            assert!(text.text.contains("Disconnected from Neovim"));
            assert!(text.text.contains(&ipc_path));
        } else {
            panic!("Expected text content in disconnect result");
        }
    } else {
        panic!("No content in disconnect result");
    }

    // Test that disconnecting again fails (not connected)
    let mut disconnect_args2 = Map::new();
    disconnect_args2.insert("connection_id".to_string(), Value::String(connection_id));

    let result = service
        .call_tool(CallToolRequestParam {
            name: "disconnect".into(),
            arguments: Some(disconnect_args2),
        })
        .await;

    assert!(
        result.is_err(),
        "Should not be able to disconnect when not connected"
    );

    // Cleanup happens automatically via guard
    service.cancel().await?;
    info!("Disconnect nvim tool test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_list_buffers_tool() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP client to test nvim-mcp server");

    // Start Neovim instance and use auto-connect service
    let (service, connection_id, _guard) = setup_connected_service!();

    // Now test list buffers
    let mut list_buffers_args = Map::new();
    list_buffers_args.insert("connection_id".to_string(), Value::String(connection_id));

    let result = service
        .call_tool(CallToolRequestParam {
            name: "list_buffers".into(),
            arguments: Some(list_buffers_args),
        })
        .await?;

    info!("List buffers result: {:#?}", result);
    assert!(!result.content.is_empty());

    // Verify the response contains buffer information
    if let Some(content) = result.content.first() {
        if let Some(text) = content.as_text() {
            // The response should be JSON with buffer info
            assert!(text.text.contains("\"id\""));
            assert!(text.text.contains("\"name\""));
            assert!(text.text.contains("\"line_count\""));
            // Should have at least the initial empty buffer with id 1
            assert!(text.text.contains("\"id\":1"));
        } else {
            panic!("Expected text content in list buffers result");
        }
    } else {
        panic!("No content in list buffers result");
    }

    // Cleanup happens automatically via guard
    service.cancel().await?;
    info!("List buffers tool test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_complete_workflow() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP client to test nvim-mcp server");

    let service = create_mcp_service!();

    // Start Neovim instance
    let ipc_path = generate_random_ipc_path();
    let _guard = setup_test_neovim_instance(&ipc_path).await?;

    // Step 1: Connect to Neovim
    info!("Step 1: Connecting to Neovim");
    let connection_id = connect_to_neovim!(service, ipc_path);
    info!(
        "✓ Connected successfully with connection_id: {}",
        connection_id
    );

    // Step 2: List buffers
    info!("Step 2: Listing buffers");
    let mut list_buffers_args = Map::new();
    list_buffers_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );

    let result = service
        .call_tool(CallToolRequestParam {
            name: "list_buffers".into(),
            arguments: Some(list_buffers_args),
        })
        .await?;

    assert!(!result.content.is_empty());
    info!("✓ Listed buffers successfully");

    // Step 3: Get LSP clients
    info!("Step 3: Getting LSP clients");
    let mut lsp_clients_args = Map::new();
    lsp_clients_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );

    let result = service
        .call_tool(CallToolRequestParam {
            name: "lsp_clients".into(),
            arguments: Some(lsp_clients_args),
        })
        .await?;

    assert!(!result.content.is_empty());
    info!("✓ Got LSP clients successfully");

    // Step 4: Disconnect
    info!("Step 4: Disconnecting from Neovim");
    let mut disconnect_args = Map::new();
    disconnect_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );

    let result = service
        .call_tool(CallToolRequestParam {
            name: "disconnect".into(),
            arguments: Some(disconnect_args),
        })
        .await?;

    assert!(!result.content.is_empty());
    info!("✓ Disconnected successfully");

    // Step 5: Verify we can't list buffers after disconnect
    info!("Step 5: Verifying disconnect");
    let mut invalid_list_args = Map::new();
    invalid_list_args.insert("connection_id".to_string(), Value::String(connection_id));

    let result = service
        .call_tool(CallToolRequestParam {
            name: "list_buffers".into(),
            arguments: Some(invalid_list_args),
        })
        .await;

    assert!(
        result.is_err(),
        "Should not be able to list buffers after disconnect"
    );
    info!("✓ Verified disconnect state");

    // Cleanup happens automatically via guard
    service.cancel().await?;
    info!("Complete workflow test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP client to test nvim-mcp server");

    let service = create_mcp_service!();

    // Test connecting to invalid address
    let mut invalid_args = Map::new();
    invalid_args.insert(
        "target".to_string(),
        Value::String("invalid:99999".to_string()),
    );

    let result = service
        .call_tool(CallToolRequestParam {
            name: "connect_tcp".into(),
            arguments: Some(invalid_args),
        })
        .await;

    assert!(result.is_err(), "Should fail to connect to invalid address");

    // Test calling tools with missing arguments
    let result = service
        .call_tool(CallToolRequestParam {
            name: "connect_tcp".into(),
            arguments: None,
        })
        .await;

    assert!(result.is_err(), "Should fail when arguments are missing");

    // Test calling non-existent tool
    let result = service
        .call_tool(CallToolRequestParam {
            name: "non_existent_tool".into(),
            arguments: None,
        })
        .await;

    assert!(
        result.is_err(),
        "Should fail when calling non-existent tool"
    );

    service.cancel().await?;
    info!("Error handling test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_exec_lua_tool() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP client to test nvim-mcp server");

    // Use auto-connect setup
    let (service, connection_id, _guard) = setup_connected_service!();

    // Test successful Lua execution
    let mut lua_args = Map::new();
    lua_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    lua_args.insert("code".to_string(), Value::String("return 42".to_string()));

    let result = service
        .call_tool(CallToolRequestParam {
            name: "exec_lua".into(),
            arguments: Some(lua_args),
        })
        .await?;

    info!("Exec Lua result: {:#?}", result);
    assert!(!result.content.is_empty());

    // Verify the response contains Lua result
    if let Some(content) = result.content.first() {
        if let Some(text) = content.as_text() {
            assert!(text.text.contains("42"));
        } else {
            panic!("Expected text content in exec_lua result");
        }
    } else {
        panic!("No content in exec_lua result");
    }

    // Test Lua execution with string result
    let mut lua_args = Map::new();
    lua_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    lua_args.insert(
        "code".to_string(),
        Value::String("return 'hello world'".to_string()),
    );

    let result = service
        .call_tool(CallToolRequestParam {
            name: "exec_lua".into(),
            arguments: Some(lua_args),
        })
        .await?;

    assert!(!result.content.is_empty());

    // Test successful string execution
    let mut string_lua_args = Map::new();
    string_lua_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    string_lua_args.insert(
        "code".to_string(),
        Value::String("return 'hello world'".to_string()),
    );

    let result = service
        .call_tool(CallToolRequestParam {
            name: "exec_lua".into(),
            arguments: Some(string_lua_args),
        })
        .await?;

    assert!(!result.content.is_empty());

    // Cleanup happens automatically via guard
    service.cancel().await?;
    info!("Exec Lua tool test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_lsp_clients_tool() -> Result<(), Box<dyn std::error::Error>> {
    let (service, connection_id, _guard) = setup_connected_service!();

    // Now test lsp_clients
    let mut lsp_clients_args = Map::new();
    lsp_clients_args.insert("connection_id".to_string(), Value::String(connection_id));

    let result = service
        .call_tool(CallToolRequestParam {
            name: "lsp_clients".into(),
            arguments: Some(lsp_clients_args),
        })
        .await?;

    info!("LSP clients result: {:#?}", result);
    assert!(!result.content.is_empty());

    // Verify the response contains content
    if let Some(_content) = result.content.first() {
        // Content received successfully - the JSON parsing is handled by the MCP framework
        info!("LSP clients content received successfully");
    } else {
        panic!("No content in lsp_clients result");
    }

    // Cleanup happens automatically via guard
    service.cancel().await?;
    info!("LSP clients tool test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_list_diagnostic_resources() -> Result<(), Box<dyn std::error::Error>> {
    let (service, _, _guard) = setup_connected_service!();

    // Test list_resources
    let result = service.list_resources(None).await?;
    info!("List resources result: {:#?}", result);

    // Verify we have the connections resource
    assert!(!result.resources.is_empty());

    let connections_resource = result
        .resources
        .iter()
        .find(|r| r.raw.uri == "nvim-connections://");

    assert!(
        connections_resource.is_some(),
        "Should have connections resource"
    );

    if let Some(resource) = connections_resource {
        assert_eq!(resource.raw.name, "Active Neovim Connections");
        assert!(resource.raw.description.is_some());
        assert_eq!(resource.raw.mime_type, Some("application/json".to_string()));
    }

    service.cancel().await?;
    info!("List diagnostic resources test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_read_workspace_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
    let (service, connection_id, _guard) = setup_connected_service!();

    // Test read workspace diagnostics resource
    let result = service
        .read_resource(ReadResourceRequestParam {
            uri: format!("nvim-diagnostics://{connection_id}/workspace"),
        })
        .await?;

    info!("Read workspace diagnostics result: {:#?}", result);
    assert!(!result.contents.is_empty());

    // Verify the response contains diagnostic data
    if let Some(_content) = result.contents.first() {
        // Content received successfully - the actual parsing can be tested
        // in more detailed unit tests if needed
        info!("Successfully received resource content");
    } else {
        panic!("No content in workspace diagnostics result");
    }

    // Test reading invalid resource URI
    let result = service
        .read_resource(ReadResourceRequestParam {
            uri: "nvim-diagnostics://invalid/workspace".to_string(),
        })
        .await;

    assert!(result.is_err(), "Should fail for invalid connection ID");

    // Test reading buffer diagnostics resource
    let result = service
        .read_resource(ReadResourceRequestParam {
            uri: format!("nvim-diagnostics://{connection_id}/buffer/1"),
        })
        .await?;

    assert!(!result.contents.is_empty());
    info!("Buffer diagnostics resource read successfully");

    // Test invalid buffer ID
    let result = service
        .read_resource(ReadResourceRequestParam {
            uri: format!("nvim-diagnostics://{connection_id}/buffer/invalid"),
        })
        .await;

    assert!(result.is_err(), "Should fail for invalid buffer ID");

    // Cleanup happens automatically via guard
    service.cancel().await?;
    info!("Read workspace diagnostics test completed successfully");

    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_lsp_organize_imports_non_existent_file() -> Result<(), Box<dyn std::error::Error>> {
    let (service, connection_id, _guard) = setup_connected_service!();

    // Test lsp_organize_imports with valid connection but non-existent file
    let mut args = Map::new();
    args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    args.insert(
        "document".to_string(),
        Value::String(r#"{"project_relative_path": "non_existent_file.go"}"#.to_string()),
    );
    args.insert(
        "lsp_client_name".to_string(),
        Value::String("gopls".to_string()),
    );
    args.insert("apply_edits".to_string(), Value::Bool(false));

    let result = service
        .call_tool(CallToolRequestParam {
            name: "lsp_organize_imports".into(),
            arguments: Some(args),
        })
        .await;
    info!("Organize imports result: {:#?}", result);

    assert!(result.is_err(), "lsp_organize_imports should fail with LSP");
    let r = result.unwrap_err();
    // The result should contain either success message or actions
    assert!(r.to_string().contains("No such file or directory"));

    service.cancel().await?;
    info!("Non-existent file test completed successfully");

    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_lsp_organize_imports_with_lsp() -> Result<(), Box<dyn std::error::Error>> {
    let (service, connection_id, _guard) = setup_connected_service!(
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        get_testdata_path("main.go").to_str().unwrap()
    );

    // Wait for LSP client (gopls) to be ready
    let mut wait_lsp_args = Map::new();
    wait_lsp_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    wait_lsp_args.insert(
        "client_name".to_string(),
        Value::String("gopls".to_string()),
    );
    wait_lsp_args.insert("timeout_ms".to_string(), Value::Number(5000.into()));

    service
        .call_tool(CallToolRequestParam {
            name: "wait_for_lsp_ready".into(),
            arguments: Some(wait_lsp_args),
        })
        .await?;

    info!("LSP client ready");

    // Test lsp_organize_imports with apply_edits=true
    let mut args = Map::new();
    args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    args.insert(
        "document".to_string(),
        Value::String(r#"{"buffer_id": 0}"#.to_string()),
    );
    args.insert(
        "lsp_client_name".to_string(),
        Value::String("gopls".to_string()),
    );
    args.insert("apply_edits".to_string(), Value::Bool(true));

    let result = service
        .call_tool(CallToolRequestParam {
            name: "lsp_organize_imports".into(),
            arguments: Some(args),
        })
        .await;

    assert!(
        result.is_ok(),
        "lsp_organize_imports should succeed with LSP"
    );
    let r = result.unwrap();
    info!("Organize imports with LSP succeeded: {:?}", r);
    // The result should contain either success message or actions
    assert!(!r.content.is_empty());
    assert!(
        serde_json::to_string(&r)
            .unwrap()
            .contains("No organize imports actions available for this document")
    );

    service.cancel().await?;
    info!("LSP organize imports test completed successfully");

    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_lsp_organize_imports_inspect_mode() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing lsp_organize_imports in inspect mode (apply_edits=false)");
    let (service, connection_id, _guard) = setup_connected_service!(
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        get_testdata_path("organize_imports.go").to_str().unwrap()
    );

    // Wait for LSP client (gopls) to be ready
    let mut wait_lsp_args = Map::new();
    wait_lsp_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    wait_lsp_args.insert(
        "client_name".to_string(),
        Value::String("gopls".to_string()),
    );
    wait_lsp_args.insert("timeout_ms".to_string(), Value::Number(5000.into()));

    service
        .call_tool(CallToolRequestParam {
            name: "wait_for_lsp_ready".into(),
            arguments: Some(wait_lsp_args),
        })
        .await?;

    info!("LSP client ready");

    // Test lsp_organize_imports with apply_edits=false (inspect mode)
    let mut inspect_args = Map::new();
    inspect_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    inspect_args.insert(
        "document".to_string(),
        Value::String(r#"{"buffer_id": 0}"#.to_string()),
    );
    inspect_args.insert(
        "lsp_client_name".to_string(),
        Value::String("gopls".to_string()),
    );
    inspect_args.insert("apply_edits".to_string(), Value::Bool(false));

    let result = service
        .call_tool(CallToolRequestParam {
            name: "lsp_organize_imports".into(),
            arguments: Some(inspect_args),
        })
        .await;

    assert!(
        result.is_ok(),
        "lsp_organize_imports should succeed in inspect mode"
    );

    let r = result.unwrap();
    info!("Organize imports inspection succeeded: {:?}", r);
    // The result should contain either code actions or a message about no actions
    assert!(!r.content.is_empty());
    assert!(
        serde_json::to_string(&r)
            .unwrap()
            .contains("documentChanges")
    );

    service.cancel().await?;
    info!("Inspect mode test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_lua_tools_end_to_end_workflow() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing end-to-end Lua tools workflow");

    // Connect to pre-compiled MCP server
    let service = create_mcp_service!();

    info!("Connected to server");

    info!("starting IPC Neovim for testing");

    let ipc_path = generate_random_socket_path();
    let cfg_path = "src/testdata/cfg_lsp.lua";
    let open_file = "src/testdata/main.go";

    let child =
        crate::test_utils::setup_neovim_instance_ipc_advance(&ipc_path, cfg_path, open_file).await;
    let _guard = NeovimIpcGuard::new(child, ipc_path.clone());

    // Neovim should be ready for connection immediately after process start

    let mut connect_args = Map::new();
    connect_args.insert("target".to_string(), Value::String(ipc_path));

    let result = service
        .call_tool(CallToolRequestParam {
            name: "connect".into(),
            arguments: Some(connect_args),
        })
        .await?;

    // Setup Lua tools configuration in Neovim
    let connection_id = extract_connection_id(&result)?;

    // Test tool discovery by listing tools (should include our custom tool)
    let tools_result = service.list_tools(Default::default()).await?;
    info!("Available tools after Lua setup: {:?}", tools_result);

    // Check if our custom tool is discovered
    let tools_contain_save_buffer = tools_result
        .tools
        .iter()
        .any(|tool| tool.name == "save_buffer");
    assert!(
        tools_contain_save_buffer,
        "Custom save_buffer tool should be discovered"
    );

    // Test custom tool execution
    let mut tool_args = Map::new();
    tool_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    tool_args.insert(
        "buffer_id".to_string(),
        Value::Number(serde_json::Number::from(1)),
    );
    let tool_result = service
        .call_tool(CallToolRequestParam {
            name: "save_buffer".into(),
            arguments: Some(tool_args),
        })
        .await;

    // The tool execution may fail (buffer not valid or no file), but it should not crash
    info!("Custom tool execution result: {:?}", tool_result);
    assert!(
        tool_result.is_ok(),
        "Custom tool should execute without error"
    );

    // Test error handling with invalid parameters
    let mut invalid_args = Map::new();
    invalid_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    invalid_args.insert(
        "buffer_id".to_string(),
        Value::Number(serde_json::Number::from(-1)),
    );

    // Test connection cleanup removes tools
    let mut disconnect_args = Map::new();
    disconnect_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );

    let disconnect_result = service
        .call_tool(CallToolRequestParam {
            name: "disconnect".into(),
            arguments: Some(disconnect_args),
        })
        .await?;

    info!("Disconnect result: {:?}", disconnect_result);

    let error_result = service
        .call_tool(CallToolRequestParam {
            name: "save_buffer".into(),
            arguments: Some(invalid_args),
        })
        .await;
    info!("Error handling test result: {:?}", error_result);
    assert!(
        error_result.is_err(),
        "Should fail when calling save_buffer with invalid parameters"
    );

    service.cancel().await?;
    info!("End-to-end Lua tools test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_cursor_position_tool() -> Result<(), Box<dyn std::error::Error>> {
    let (service, connection_id, _guard) = setup_connected_service!();

    // Test successful cursor_position call
    let mut cursor_args = Map::new();
    cursor_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );

    let result = service
        .call_tool(CallToolRequestParam {
            name: "cursor_position".into(),
            arguments: Some(cursor_args),
        })
        .await?;

    info!("Cursor position result: {:#?}", result);
    assert!(!result.content.is_empty());

    // Verify the response contains cursor position data
    if let Some(content) = result.content.first() {
        if let Some(text) = content.as_text() {
            // Parse JSON response
            let cursor_data: serde_json::Value = serde_json::from_str(&text.text)?;

            // Verify required fields are present
            assert!(
                cursor_data["buffer_name"].is_string(),
                "Should have bufname field"
            );
            assert!(
                cursor_data["buffer_id"].is_number(),
                "Should have buffer_id field"
            );
            assert!(
                cursor_data["window_id"].is_number(),
                "Should have window_id field"
            );
            assert!(cursor_data["row"].is_number(), "Should have row field");
            assert!(cursor_data["col"].is_number(), "Should have col field");

            // Verify coordinates are zero-based (should be 0,0 for new buffer)
            let row = cursor_data["row"].as_i64().expect("row should be a number");
            let col = cursor_data["col"].as_i64().expect("col should be a number");

            assert!(row >= 0, "Row should be zero-based (>= 0)");
            assert!(col >= 0, "Col should be zero-based (>= 0)");

            info!(
                "Cursor position: bufname={}, row={}, col={}",
                cursor_data["bufname"], row, col
            );
        } else {
            panic!("Expected text content in cursor_position result");
        }
    } else {
        panic!("No content in cursor_position result");
    }

    // Cleanup happens automatically via guard
    service.cancel().await?;
    info!("Cursor position tool test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_navigate_tool() -> Result<(), Box<dyn std::error::Error>> {
    let (service, connection_id, _guard) = setup_connected_service!();

    // Create a temporary directory and test file
    let temp_dir = tempfile::tempdir()?;
    let test_file = temp_dir.path().join("test_navigate.txt");
    std::fs::write(&test_file, "line 1\nline 2\nline 3\n")?;

    // Test 1: Navigate to absolute path
    info!("Test 1: Navigate to absolute path");
    let mut navigate_args = Map::new();
    navigate_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    navigate_args.insert(
        "document".to_string(),
        serde_json::json!({
            "absolute_path": test_file.to_string_lossy()
        }),
    );
    navigate_args.insert(
        "line".to_string(),
        Value::Number(serde_json::Number::from(1)),
    );
    navigate_args.insert(
        "character".to_string(),
        Value::Number(serde_json::Number::from(3)),
    );

    let result = service
        .call_tool(CallToolRequestParam {
            name: "navigate".into(),
            arguments: Some(navigate_args),
        })
        .await?;

    // Verify navigation result
    assert!(!result.content.is_empty());
    if let Some(content) = result.content.first()
        && let rmcp::model::RawContent::Text(text_content) = &content.raw
    {
        let navigate_data: serde_json::Value = serde_json::from_str(&text_content.text)?;

        assert!(
            navigate_data["success"].as_bool().unwrap_or(false),
            "Navigation should succeed"
        );
        assert_eq!(navigate_data["line"].as_str().unwrap_or_default(), "line 2");
        info!("✓ Successfully navigated to absolute path");
    }

    // Test 2: Navigate with invalid file path
    info!("Test 2: Navigate to non-existent file");
    let mut invalid_navigate_args = Map::new();
    invalid_navigate_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    invalid_navigate_args.insert(
        "document".to_string(),
        serde_json::json!({
            "absolute_path": "/non/existent/file.txt"
        }),
    );

    let result = service
        .call_tool(CallToolRequestParam {
            name: "navigate".into(),
            arguments: Some(invalid_navigate_args),
        })
        .await;

    assert!(
        result.is_err(),
        "Should fail to navigate to non-existent file"
    );
    info!("✓ Correctly handled invalid file path");

    // Test 3: Navigate to current buffer by ID
    info!("Test 3: Navigate by buffer ID");

    // First get the current buffer list
    let mut list_buffers_args = Map::new();
    list_buffers_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );

    let buffer_result = service
        .call_tool(CallToolRequestParam {
            name: "list_buffers".into(),
            arguments: Some(list_buffers_args),
        })
        .await?;

    if let Some(content) = buffer_result.content.first()
        && let rmcp::model::RawContent::Text(text_content) = &content.raw
    {
        let buffers_data: serde_json::Value = serde_json::from_str(&text_content.text)?;

        if let Some(buffers_array) = buffers_data.as_array()
            && let Some(first_buffer) = buffers_array.first()
            && let Some(buffer_id) = first_buffer["id"].as_u64()
        {
            // Navigate to this buffer
            let mut buffer_navigate_args = Map::new();
            buffer_navigate_args.insert(
                "connection_id".to_string(),
                Value::String(connection_id.clone()),
            );
            buffer_navigate_args.insert(
                "document".to_string(),
                serde_json::json!({
                    "buffer_id": buffer_id
                }),
            );
            buffer_navigate_args.insert(
                "line".to_string(),
                Value::Number(serde_json::Number::from(0)),
            );
            buffer_navigate_args.insert(
                "character".to_string(),
                Value::Number(serde_json::Number::from(0)),
            );

            let result = service
                .call_tool(CallToolRequestParam {
                    name: "navigate".into(),
                    arguments: Some(buffer_navigate_args),
                })
                .await?;

            if let Some(content) = result.content.first()
                && let rmcp::model::RawContent::Text(text_content) = &content.raw
            {
                let navigate_data: serde_json::Value = serde_json::from_str(&text_content.text)?;
                assert!(
                    navigate_data["success"].as_bool().unwrap_or(false),
                    "Navigation by buffer ID should succeed"
                );
                info!("✓ Successfully navigated by buffer ID");
            }
        }
    }

    // Cleanup
    service.cancel().await?;
    info!("Navigate tool test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_lsp_call_hierarchy_prepare() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing lsp_call_hierarchy_prepare");
    let (service, connection_id, _guard) = setup_connected_service!(
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        get_testdata_path("call_hierarchy.go").to_str().unwrap()
    );

    // Test call hierarchy prepare - tool should exist and handle the request
    let mut args = Map::new();
    args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    args.insert(
        "document".to_string(),
        serde_json::json!({
            "buffer_id": 0
        }),
    );
    args.insert(
        "lsp_client_name".to_string(),
        Value::String("gopls".to_string()),
    );
    args.insert(
        "line".to_string(),
        Value::Number(serde_json::Number::from(16)),
    ); // caller function line (0-based)
    args.insert(
        "character".to_string(),
        Value::Number(serde_json::Number::from(5)),
    ); // inside function name

    let result = service
        .call_tool(CallToolRequestParam {
            name: "lsp_call_hierarchy_prepare".into(),
            arguments: Some(args),
        })
        .await;

    info!("Call hierarchy prepare result: {:?}", result);

    // The tool should exist and execute (even if LSP is not available, it should not panic)
    // It may return an error about LSP not being available, but the tool itself should work
    match result {
        Ok(tool_result) => {
            info!("Tool executed successfully: {:?}", tool_result);
            assert!(!tool_result.content.is_empty());
        }
        Err(e) => {
            panic!("Tool failed as expected (LSP may not be ready): {}", e);
            // Tool failure due to LSP unavailability is acceptable in test environment
        }
    }

    service.cancel().await?;
    info!("Call hierarchy prepare test completed successfully");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_lsp_call_hierarchy_incoming_calls() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing lsp_call_hierarchy_incoming_calls");
    let (service, connection_id, _guard) = setup_connected_service!(
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        get_testdata_path("call_hierarchy.go").to_str().unwrap()
    );

    // Create a mock call hierarchy item for testing
    let mock_hierarchy_item = serde_json::json!({
        "name": "helper",
        "kind": 12, // Function symbol kind
        "uri": get_file_uri("call_hierarchy.go"),
        "range": {
            "start": {"line": 5, "character": 5},
            "end": {"line": 5, "character": 11}
        },
        "selectionRange": {
            "start": {"line": 5, "character": 5},
            "end": {"line": 5, "character": 11}
        }
    });

    // Test incoming calls tool
    let mut args = Map::new();
    args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    args.insert(
        "lsp_client_name".to_string(),
        Value::String("gopls".to_string()),
    );
    args.insert("item".to_string(), mock_hierarchy_item);

    let result = service
        .call_tool(CallToolRequestParam {
            name: "lsp_call_hierarchy_incoming_calls".into(),
            arguments: Some(args),
        })
        .await;

    info!("Call hierarchy incoming calls result: {:?}", result);

    // The tool should exist and execute (even if LSP is not available, it should not panic)
    match result {
        Ok(tool_result) => {
            info!("Tool executed successfully: {:?}", tool_result);
            assert!(!tool_result.content.is_empty());
        }
        Err(e) => {
            panic!("Tool failed as expected (LSP may not be ready): {}", e);
            // Tool failure due to LSP unavailability is acceptable in test environment
        }
    }

    service.cancel().await?;
    info!("Call hierarchy incoming calls test completed successfully");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_lsp_call_hierarchy_outgoing_calls() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing lsp_call_hierarchy_outgoing_calls");
    let (service, connection_id, _guard) = setup_connected_service!(
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        get_testdata_path("call_hierarchy.go").to_str().unwrap()
    );

    // Create a mock call hierarchy item for testing
    let mock_hierarchy_item = serde_json::json!({
        "name": "caller",
        "kind": 12, // Function symbol kind
        "uri": get_file_uri("call_hierarchy.go"),
        "range": {
            "start": {"line": 16, "character": 5},
            "end": {"line": 16, "character": 11}
        },
        "selectionRange": {
            "start": {"line": 16, "character": 5},
            "end": {"line": 16, "character": 11}
        }
    });

    // Test outgoing calls tool
    let mut args = Map::new();
    args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    args.insert(
        "lsp_client_name".to_string(),
        Value::String("gopls".to_string()),
    );
    args.insert("item".to_string(), mock_hierarchy_item);

    let result = service
        .call_tool(CallToolRequestParam {
            name: "lsp_call_hierarchy_outgoing_calls".into(),
            arguments: Some(args),
        })
        .await;

    info!("Call hierarchy outgoing calls result: {:?}", result);

    // The tool should exist and execute (even if LSP is not available, it should not panic)
    match result {
        Ok(tool_result) => {
            info!("Tool executed successfully: {:?}", tool_result);
            assert!(!tool_result.content.is_empty());
        }
        Err(e) => {
            panic!("Tool failed as expected (LSP may not be ready): {}", e);
            // Tool failure due to LSP unavailability is acceptable in test environment
        }
    }

    service.cancel().await?;
    info!("Call hierarchy outgoing calls test completed successfully");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_lsp_type_hierarchy_prepare() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing lsp_type_hierarchy_prepare");
    let (service, connection_id, _guard) = setup_connected_service!(
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        get_testdata_path("type_hierarchy.go").to_str().unwrap()
    );

    // Test type hierarchy prepare - tool should exist and handle the request
    let mut args = Map::new();
    args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    args.insert(
        "document".to_string(),
        serde_json::json!({
            "buffer_id": 0
        }),
    );
    args.insert(
        "lsp_client_name".to_string(),
        Value::String("gopls".to_string()),
    );
    args.insert(
        "line".to_string(),
        Value::Number(serde_json::Number::from(11)),
    ); // Shape interface line
    args.insert(
        "character".to_string(),
        Value::Number(serde_json::Number::from(10)),
    ); // inside interface name

    let result = service
        .call_tool(CallToolRequestParam {
            name: "lsp_type_hierarchy_prepare".into(),
            arguments: Some(args),
        })
        .await;

    info!("Type hierarchy prepare result: {:?}", result);

    // The tool should exist and execute (even if LSP is not available, it should not panic)
    // It may return an error about LSP not being available, but the tool itself should work
    match result {
        Ok(tool_result) => {
            info!("Tool executed successfully: {:?}", tool_result);
            assert!(!tool_result.content.is_empty());
        }
        Err(e) => {
            panic!("Tool failed as expected (LSP may not be ready): {}", e);
            // Tool failure due to LSP unavailability is acceptable in test environment
        }
    }

    service.cancel().await?;
    info!("Type hierarchy prepare test completed successfully");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_lsp_type_hierarchy_supertypes() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing lsp_type_hierarchy_supertypes");
    let (service, connection_id, _guard) = setup_connected_service!(
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        get_testdata_path("type_hierarchy.go").to_str().unwrap()
    );

    // Create a mock type hierarchy item for testing
    let mock_hierarchy_item = serde_json::json!({
        "name": "ColoredShape",
        "kind": 11, // Interface symbol kind
        "uri": get_file_uri("type_hierarchy.go"),
        "range": {
            "start": {"line": 11, "character": 5},
            "end": {"line": 11, "character": 17}
        },
        "selectionRange": {
            "start": {"line": 11, "character": 5},
            "end": {"line": 11, "character": 17}
        }
    });

    // Test supertypes tool
    let mut args = Map::new();
    args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    args.insert(
        "lsp_client_name".to_string(),
        Value::String("gopls".to_string()),
    );
    args.insert("item".to_string(), mock_hierarchy_item);

    let result = service
        .call_tool(CallToolRequestParam {
            name: "lsp_type_hierarchy_supertypes".into(),
            arguments: Some(args),
        })
        .await;

    info!("Type hierarchy supertypes result: {:?}", result);

    // The tool should exist and execute (even if LSP is not available, it should not panic)
    match result {
        Ok(tool_result) => {
            info!("Tool executed successfully: {:?}", tool_result);
            assert!(!tool_result.content.is_empty());
        }
        Err(e) => {
            panic!("Tool failed as expected (LSP may not be ready): {}", e);
            // Tool failure due to LSP unavailability is acceptable in test environment
        }
    }

    service.cancel().await?;
    info!("Type hierarchy supertypes test completed successfully");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_lsp_type_hierarchy_subtypes() -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing lsp_type_hierarchy_subtypes");
    let (service, connection_id, _guard) = setup_connected_service!(
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        get_testdata_path("type_hierarchy.go").to_str().unwrap()
    );

    // Create a mock type hierarchy item for testing
    let mock_hierarchy_item = serde_json::json!({
        "name": "Shape",
        "kind": 11, // Interface symbol kind
        "uri": get_file_uri("type_hierarchy.go"),
        "range": {
            "start": {"line": 5, "character": 5},
            "end": {"line": 5, "character": 10}
        },
        "selectionRange": {
            "start": {"line": 5, "character": 5},
            "end": {"line": 5, "character": 10}
        }
    });

    // Test subtypes tool
    let mut args = Map::new();
    args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    args.insert(
        "lsp_client_name".to_string(),
        Value::String("gopls".to_string()),
    );
    args.insert("item".to_string(), mock_hierarchy_item);

    let result = service
        .call_tool(CallToolRequestParam {
            name: "lsp_type_hierarchy_subtypes".into(),
            arguments: Some(args),
        })
        .await;

    info!("Type hierarchy subtypes result: {:?}", result);

    // The tool should exist and execute (even if LSP is not available, it should not panic)
    match result {
        Ok(tool_result) => {
            info!("Tool executed successfully: {:?}", tool_result);
            assert!(!tool_result.content.is_empty());
        }
        Err(e) => {
            panic!("Tool failed as expected (LSP may not be ready): {}", e);
            // Tool failure due to LSP unavailability is acceptable in test environment
        }
    }

    service.cancel().await?;
    info!("Type hierarchy subtypes test completed successfully");
    Ok(())
}
