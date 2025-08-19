use std::time::Duration;

use rmcp::{
    model::{CallToolRequestParam, ReadResourceRequestParam},
    serde_json::{Map, Value},
    service::ServiceExt,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use tokio::{process::Command, time};
use tracing::{error, info};
use tracing_test::traced_test;

use crate::test_utils::*;

// Helper function to extract connection_id from connect response
fn extract_connection_id(
    result: &rmcp::model::CallToolResult,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(content) = result.content.as_ref().and_then(|c| c.first()) {
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

#[tokio::test]
#[traced_test]
async fn test_mcp_server_connection() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP client to test nvim-mcp server");

    // Connect to the server running as a child process (exact copy from original)
    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

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

    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

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
async fn test_connect_nvim_tcp_tool() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP client to test nvim-mcp server");

    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

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
    assert!(!result.content.as_ref().is_none_or(|c| c.is_empty()));

    // Verify the response contains success message
    if let Some(content) = result.content.as_ref().and_then(|c| c.first()) {
        if let Some(text) = content.as_text() {
            assert!(text.text.contains("Connected to Neovim"));
            assert!(text.text.contains(&ipc_path));
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
async fn test_disconnect_nvim_tcp_tool() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP client to test nvim-mcp server");

    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

    // Test disconnect without valid connection (should fail)
    let mut invalid_disconnect_args = Map::new();
    invalid_disconnect_args.insert(
        "connection_id".to_string(),
        Value::String("invalid_connection_id".to_string()),
    );

    let result = service
        .call_tool(CallToolRequestParam {
            name: "disconnect".into(),
            arguments: Some(invalid_disconnect_args),
        })
        .await;

    assert!(
        result.is_err(),
        "Disconnect should fail with invalid connection ID"
    );

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
    assert!(!result.content.as_ref().is_none_or(|c| c.is_empty()));

    // Verify the response contains success message
    if let Some(content) = result.content.as_ref().and_then(|c| c.first()) {
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

    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

    // Test list buffers without connection (should fail)
    let mut invalid_args = Map::new();
    invalid_args.insert(
        "connection_id".to_string(),
        Value::String("invalid_connection_id".to_string()),
    );

    let result = service
        .call_tool(CallToolRequestParam {
            name: "list_buffers".into(),
            arguments: Some(invalid_args),
        })
        .await;

    assert!(
        result.is_err(),
        "List buffers should fail with invalid connection ID"
    );

    // Now connect first, then test list buffers
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
    assert!(!result.content.as_ref().is_none_or(|c| c.is_empty()));

    // Verify the response contains buffer information
    if let Some(content) = result.content.as_ref().and_then(|c| c.first()) {
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

    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

    // Start Neovim instance
    let ipc_path = generate_random_ipc_path();
    let _guard = setup_test_neovim_instance(&ipc_path).await?;

    // Step 1: Connect to Neovim
    info!("Step 1: Connecting to Neovim");
    let mut connect_args = Map::new();
    connect_args.insert("target".to_string(), Value::String(ipc_path.clone()));

    let connect_result = service
        .call_tool(CallToolRequestParam {
            name: "connect".into(),
            arguments: Some(connect_args),
        })
        .await?;

    assert!(!connect_result.content.as_ref().is_none_or(|c| c.is_empty()));
    let connection_id = extract_connection_id(&connect_result)?;
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

    assert!(!result.content.as_ref().is_none_or(|c| c.is_empty()));
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

    assert!(!result.content.as_ref().is_none_or(|c| c.is_empty()));
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

    assert!(!result.content.as_ref().is_none_or(|c| c.is_empty()));
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

    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

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

    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

    // Test exec_lua without connection (should fail)
    let mut lua_args = Map::new();
    lua_args.insert(
        "connection_id".to_string(),
        Value::String("invalid_connection_id".to_string()),
    );
    lua_args.insert("code".to_string(), Value::String("return 42".to_string()));

    let result = service
        .call_tool(CallToolRequestParam {
            name: "exec_lua".into(),
            arguments: Some(lua_args),
        })
        .await;

    assert!(
        result.is_err(),
        "exec_lua should fail with invalid connection ID"
    );

    // Now connect first, then test exec_lua
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
    assert!(!result.content.as_ref().is_none_or(|c| c.is_empty()));

    // Verify the response contains Lua result
    if let Some(content) = result.content.as_ref().and_then(|c| c.first()) {
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

    assert!(!result.content.as_ref().is_none_or(|c| c.is_empty()));

    // Test error handling for invalid Lua
    let mut invalid_lua_args = Map::new();
    invalid_lua_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    invalid_lua_args.insert(
        "code".to_string(),
        Value::String("invalid lua syntax !!!".to_string()),
    );

    let result = service
        .call_tool(CallToolRequestParam {
            name: "exec_lua".into(),
            arguments: Some(invalid_lua_args),
        })
        .await;

    assert!(result.is_err(), "Should fail for invalid Lua syntax");

    // Test error handling for empty code
    let mut empty_lua_args = Map::new();
    empty_lua_args.insert(
        "connection_id".to_string(),
        Value::String(connection_id.clone()),
    );
    empty_lua_args.insert("code".to_string(), Value::String("".to_string()));

    let result = service
        .call_tool(CallToolRequestParam {
            name: "exec_lua".into(),
            arguments: Some(empty_lua_args),
        })
        .await;

    assert!(result.is_err(), "Should fail for empty Lua code");

    // Test missing code argument
    let mut missing_code_args = Map::new();
    missing_code_args.insert("connection_id".to_string(), Value::String(connection_id));

    let result = service
        .call_tool(CallToolRequestParam {
            name: "exec_lua".into(),
            arguments: Some(missing_code_args),
        })
        .await;

    assert!(result.is_err(), "Should fail when code argument is missing");

    // Cleanup happens automatically via guard
    service.cancel().await?;
    info!("Exec Lua tool test completed successfully");

    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_lsp_clients_tool() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP client to test nvim-mcp server");

    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

    // Test lsp_clients without connection (should fail)
    let mut invalid_args = Map::new();
    invalid_args.insert(
        "connection_id".to_string(),
        Value::String("invalid_connection_id".to_string()),
    );

    let result = service
        .call_tool(CallToolRequestParam {
            name: "lsp_clients".into(),
            arguments: Some(invalid_args),
        })
        .await;

    assert!(
        result.is_err(),
        "lsp_clients should fail with invalid connection ID"
    );

    // Now connect first, then test lsp_clients
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
    assert!(!result.content.as_ref().is_none_or(|c| c.is_empty()));

    // Verify the response contains content
    if let Some(_content) = result.content.as_ref().and_then(|c| c.first()) {
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
    info!("Starting MCP client to test diagnostic resources");

    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

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
    info!("Starting MCP client to test reading workspace diagnostics");

    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

    // Start Neovim instance
    let ipc_path = generate_random_ipc_path();
    let _guard = setup_test_neovim_instance(&ipc_path).await?;

    // Connect to Neovim first
    let mut connect_args = Map::new();
    connect_args.insert("target".to_string(), Value::String(ipc_path.clone()));

    let connect_result = service
        .call_tool(CallToolRequestParam {
            name: "connect".into(),
            arguments: Some(connect_args),
        })
        .await?;

    let connection_id = extract_connection_id(&connect_result)?;

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
    info!("Testing lsp_organize_imports with non-existent file");

    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

    // Start a test Neovim instance
    let ipc_path = generate_random_ipc_path();
    let _guard = setup_neovim_instance_ipc_advance(
        &ipc_path,
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        get_testdata_path("organize_imports.go").to_str().unwrap(),
    )
    .await;

    // Establish connection
    let connection_id = {
        let mut connect_args = Map::new();
        connect_args.insert("target".to_string(), Value::String(ipc_path.clone()));

        let result = service
            .call_tool(CallToolRequestParam {
                name: "connect".into(),
                arguments: Some(connect_args),
            })
            .await?;

        info!("Connection established successfully");
        extract_connection_id(&result)?
    };

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
    info!("Testing lsp_organize_imports with LSP setup");

    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

    // Start a test Neovim instance with LSP
    let ipc_path = generate_random_ipc_path();
    let _guard = setup_neovim_instance_ipc_advance(
        &ipc_path,
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        get_testdata_path("main.go").to_str().unwrap(),
    )
    .await;

    time::sleep(Duration::from_secs(1)).await; // Ensure LSP is ready

    // Establish connection
    let connection_id = {
        let mut connect_args = Map::new();
        connect_args.insert("target".to_string(), Value::String(ipc_path.clone()));

        let result = service
            .call_tool(CallToolRequestParam {
                name: "connect".into(),
                arguments: Some(connect_args),
            })
            .await?;

        info!("Connection established successfully");
        extract_connection_id(&result)?
    };

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
    assert!(r.content.is_some());
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

    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

    // Start a test Neovim instance with LSP
    let ipc_path = generate_random_ipc_path();
    let _guard = setup_neovim_instance_ipc_advance(
        &ipc_path,
        get_testdata_path("cfg_lsp.lua").to_str().unwrap(),
        get_testdata_path("organize_imports.go").to_str().unwrap(),
    )
    .await;

    time::sleep(Duration::from_secs(1)).await; // Ensure LSP is ready

    // Establish connection
    let connection_id = {
        let mut connect_args = Map::new();
        connect_args.insert("target".to_string(), Value::String(ipc_path.clone()));

        let result = service
            .call_tool(CallToolRequestParam {
                name: "connect".into(),
                arguments: Some(connect_args),
            })
            .await?;

        info!("Connection established successfully");
        extract_connection_id(&result)?
    };

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
    assert!(r.content.is_some());
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

    // Start MCP server with child process
    let service = ()
        .serve(TokioChildProcess::new(Command::new("cargo").configure(
            |cmd| {
                cmd.args(["run", "--bin", "nvim-mcp"]);
            },
        ))?)
        .await
        .map_err(|e| {
            error!("Failed to connect to server: {}", e);
            e
        })?;

    info!("Connected to server");

    let connection_id = {
        info!("starting IPC Neovim for testing");

        let test_ipc_path = generate_random_socket_path();
        let cfg_path = "src/testdata/cfg_lsp.lua";
        let open_file = "src/testdata/main.go";

        let _neovim_child = crate::test_utils::setup_neovim_instance_ipc_advance(
            &test_ipc_path,
            cfg_path,
            open_file,
        )
        .await;

        // Wait for Neovim to be ready
        time::sleep(Duration::from_millis(2000)).await;

        let mut connect_args = Map::new();
        connect_args.insert("target".to_string(), Value::String(test_ipc_path));

        let result = service
            .call_tool(CallToolRequestParam {
                name: "connect".into(),
                arguments: Some(connect_args),
            })
            .await?;

        // Setup Lua tools configuration in Neovim
        extract_connection_id(&result)?
    };

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
