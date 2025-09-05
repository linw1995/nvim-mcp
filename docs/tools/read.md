Read document content with universal document identification

Supports reading from:

- Buffer IDs (for currently open files)
- Project-relative paths (relative to project root)
- Absolute file paths

Parameters:

- `connection_id`: Target Neovim connection
- `document`: DocumentIdentifier specifying the target document
- `start`: Start line index (0-based, optional, default: 0)
- `end`: End line index, exclusive (0-based, optional, default: -1 for end of buffer)

Examples:

```json
// Read entire buffer
{"connection_id": "abc123", "document": {"buffer_id": 0}}

// Read specific line range from project file
{"connection_id": "abc123", "document": {"project_relative_path": "src/main.rs"},
 "start": 5, "end": 15}

// Read from absolute path
{"connection_id": "abc123", "document": {"absolute_path": "/home/user/project/src/main.rs"}}
```
