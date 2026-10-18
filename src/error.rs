//! Error helpers: map internal failures to MCP protocol errors.

use rmcp::ErrorData as McpError;

/// Build an internal MCP error from any displayable failure.
pub fn internal(msg: impl std::fmt::Display) -> McpError {
    McpError::internal_error(msg.to_string(), None)
}
