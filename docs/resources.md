# MCP Resources

Access diagnostic and connection information through structured URI schemes:

## Available Resources

### Connection Monitoring

- **`nvim-connections://`**: List all active Neovim connections
  - Returns array of connection objects with `id` and `target` information
  - Useful for monitoring multiple concurrent Neovim instances

### Tool Registration Overview ⚠️ **(Experimental)**

- **`nvim-tools://`**: Overview of all tools and their connection mappings
  - Shows static tools (available to all connections) and dynamic tools
    (connection-specific)
  - Useful for understanding tool availability across connections

- **`nvim-tools://{connection_id}`**: List of tools available for a specific connection
  - Includes both static and connection-specific dynamic tools
  - Provides detailed view of tools available for a particular Neovim instance

*Note: Tool registration resources are experimental and may change in future versions.*

### Connection-Scoped Diagnostics

Diagnostic resources use connection-specific URIs via the
`nvim-diagnostics://` scheme:

- **`nvim-diagnostics://{connection_id}/workspace`**: All diagnostic messages
  across workspace for specific connection
- **`nvim-diagnostics://{connection_id}/buffer/{buffer_id}`**: Diagnostics for
  specific buffer on specific connection

## Usage Examples

### List Active Connections

```json
{
  "method": "resources/read",
  "params": {
    "uri": "nvim-connections://"
  }
}
```

### Get Connection-Specific Workspace Diagnostics

```json
{
  "method": "resources/read",
  "params": {
    "uri": "nvim-diagnostics://abc123def456/workspace"
  }
}
```

### Get Buffer Diagnostics for Specific Connection

```json
{
  "method": "resources/read",
  "params": {
    "uri": "nvim-diagnostics://abc123def456/buffer/1"
  }
}
```

All diagnostic resources return structured JSON with diagnostic information
including severity levels, messages, file paths, and line/column positions.
Connection IDs are deterministic BLAKE3 hashes of the target string for
consistent identification across sessions.
