//! MCP (Model Context Protocol) Handler
//!
//! Supports all MCP transports per specification 2025-11-25:
//! - HTTP (request/response)
//! - SSE (Server-Sent Events)
//! - WebSocket (bidirectional)
//! - STDIO (BLOCKED - off-mesh)

pub mod jsonrpc;
pub mod http;
pub mod sse;
pub mod websocket;
pub mod stdio_detect;

pub use jsonrpc::{JsonRpcRequest, JsonRpcResponse, JsonRpcError};
pub use http::McpHttpHandler;
pub use sse::McpSseHandler;
pub use websocket::McpWebSocketHandler;

/// MCP transport types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpTransport {
    /// HTTP request/response
    Http,
    /// Server-Sent Events
    Sse,
    /// WebSocket
    WebSocket,
    /// Streamable HTTP (chunked)
    StreamableHttp,
    /// STDIO - BLOCKED (off-mesh, no visibility)
    Stdio,
}

impl McpTransport {
    /// Check if this transport is allowed
    pub fn is_allowed(&self) -> bool {
        !matches!(self, McpTransport::Stdio)
    }

    /// Detect transport from headers
    pub fn detect(headers: &[(String, String)]) -> Option<Self> {
        for (name, value) in headers {
            let name_lower = name.to_lowercase();
            let value_lower = value.to_lowercase();

            // Check for WebSocket upgrade
            if name_lower == "upgrade" && value_lower == "websocket" {
                return Some(McpTransport::WebSocket);
            }

            // Check for SSE
            if name_lower == "accept" && value_lower.contains("text/event-stream") {
                return Some(McpTransport::Sse);
            }

            // Check for MCP transport header
            if name_lower == "x-mcp-transport" {
                return match value_lower.as_str() {
                    "http" => Some(McpTransport::Http),
                    "sse" => Some(McpTransport::Sse),
                    "websocket" => Some(McpTransport::WebSocket),
                    "stdio" => Some(McpTransport::Stdio),
                    _ => None,
                };
            }
        }

        // Default to HTTP
        Some(McpTransport::Http)
    }
}

/// MCP request wrapper
#[derive(Debug, Clone)]
pub struct McpRequest {
    /// JSON-RPC request
    pub jsonrpc: JsonRpcRequest,
    /// Detected transport
    pub transport: McpTransport,
}

/// MCP response wrapper
#[derive(Debug, Clone)]
pub struct McpResponse {
    /// JSON-RPC response
    pub jsonrpc: JsonRpcResponse,
}

/// MCP handler for all transports
pub struct McpHandler {
    /// HTTP handler
    http_handler: McpHttpHandler,
    /// SSE handler
    sse_handler: McpSseHandler,
    /// WebSocket handler
    websocket_handler: McpWebSocketHandler,
    /// Allowed methods
    allowed_methods: Vec<String>,
    /// Block STDIO transport
    block_stdio: bool,
}

impl McpHandler {
    /// Create a new MCP handler
    pub fn new(allowed_methods: Vec<String>) -> Self {
        Self {
            http_handler: McpHttpHandler::new(allowed_methods.clone()),
            sse_handler: McpSseHandler::new(),
            websocket_handler: McpWebSocketHandler::new(),
            allowed_methods,
            block_stdio: true,
        }
    }

    /// Validate an MCP request
    pub fn validate_request(&self, body: &[u8], transport: McpTransport) -> Result<McpRequest, McpValidationError> {
        // Block STDIO transport
        if transport == McpTransport::Stdio && self.block_stdio {
            return Err(McpValidationError::TransportBlocked("STDIO transport is blocked for mesh visibility".to_string()));
        }

        // Parse JSON-RPC request
        let jsonrpc = self.http_handler.validate_request(body)?;

        Ok(McpRequest {
            jsonrpc,
            transport,
        })
    }

    /// Check if a method is allowed
    pub fn is_method_allowed(&self, method: &str) -> bool {
        self.allowed_methods.iter().any(|m| m == "*" || m == method)
    }

    /// Get HTTP handler
    pub fn http(&mut self) -> &mut McpHttpHandler {
        &mut self.http_handler
    }

    /// Get SSE handler
    pub fn sse(&mut self) -> &mut McpSseHandler {
        &mut self.sse_handler
    }

    /// Get WebSocket handler
    pub fn websocket(&mut self) -> &mut McpWebSocketHandler {
        &mut self.websocket_handler
    }
}

impl Default for McpHandler {
    fn default() -> Self {
        Self::new(vec!["*".to_string()])
    }
}

/// MCP validation errors
#[derive(Debug, Clone)]
pub enum McpValidationError {
    /// Invalid JSON
    InvalidJson(String),
    /// Invalid JSON-RPC version
    InvalidVersion(String),
    /// Method not allowed
    MethodNotAllowed(String),
    /// Transport blocked
    TransportBlocked(String),
    /// Missing required field
    MissingField(String),
    /// Invalid message format
    InvalidFormat(String),
}

impl std::fmt::Display for McpValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpValidationError::InvalidJson(e) => write!(f, "Invalid JSON: {}", e),
            McpValidationError::InvalidVersion(v) => write!(f, "Invalid JSON-RPC version: {}", v),
            McpValidationError::MethodNotAllowed(m) => write!(f, "Method not allowed: {}", m),
            McpValidationError::TransportBlocked(t) => write!(f, "Transport blocked: {}", t),
            McpValidationError::MissingField(field) => write!(f, "Missing field: {}", field),
            McpValidationError::InvalidFormat(e) => write!(f, "Invalid format: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_detection_websocket() {
        let headers = vec![("upgrade".to_string(), "websocket".to_string())];
        assert_eq!(McpTransport::detect(&headers), Some(McpTransport::WebSocket));
    }

    #[test]
    fn test_transport_detection_sse() {
        let headers = vec![("accept".to_string(), "text/event-stream".to_string())];
        assert_eq!(McpTransport::detect(&headers), Some(McpTransport::Sse));
    }

    #[test]
    fn test_transport_detection_http() {
        let headers = vec![("content-type".to_string(), "application/json".to_string())];
        assert_eq!(McpTransport::detect(&headers), Some(McpTransport::Http));
    }

    #[test]
    fn test_stdio_blocked() {
        assert!(!McpTransport::Stdio.is_allowed());
        assert!(McpTransport::Http.is_allowed());
    }
}
