//! MCP HTTP Transport Handler
//!
//! Handles MCP over HTTP request/response.
//! Validates JSON-RPC 2.0 format and checks method permissions.

use super::jsonrpc::{JsonRpcRequest, JsonRpcError, JsonRpcResponse};
use super::McpValidationError;

/// MCP HTTP transport handler
pub struct McpHttpHandler {
    /// Allowed methods
    allowed_methods: Vec<String>,
}

impl McpHttpHandler {
    /// Create a new HTTP handler
    pub fn new(allowed_methods: Vec<String>) -> Self {
        Self { allowed_methods }
    }

    /// Validate an HTTP request body
    pub fn validate_request(&self, body: &[u8]) -> Result<JsonRpcRequest, McpValidationError> {
        // Parse JSON
        let request: JsonRpcRequest = serde_json::from_slice(body)
            .map_err(|e| McpValidationError::InvalidJson(e.to_string()))?;

        // Validate JSON-RPC format
        if let Err(e) = request.validate() {
            return Err(McpValidationError::InvalidFormat(e.to_string()));
        }

        // Check if method is allowed
        if !self.is_method_allowed(&request.method) {
            return Err(McpValidationError::MethodNotAllowed(request.method.clone()));
        }

        Ok(request)
    }

    /// Check if a method is allowed
    pub fn is_method_allowed(&self, method: &str) -> bool {
        self.allowed_methods.iter().any(|m| m == "*" || m == method)
    }

    /// Create a blocked response
    pub fn create_blocked_response(&self, id: serde_json::Value, reason: &str) -> JsonRpcResponse {
        JsonRpcResponse::error(id, JsonRpcError::policy_violation(reason))
    }

    /// Validate a batch request
    pub fn validate_batch(&self, body: &[u8]) -> Result<Vec<JsonRpcRequest>, McpValidationError> {
        // Try to parse as array
        let requests: Vec<JsonRpcRequest> = serde_json::from_slice(body)
            .map_err(|e| McpValidationError::InvalidJson(e.to_string()))?;

        // Validate each request
        for request in &requests {
            if let Err(e) = request.validate() {
                return Err(McpValidationError::InvalidFormat(e.to_string()));
            }
            if !self.is_method_allowed(&request.method) {
                return Err(McpValidationError::MethodNotAllowed(request.method.clone()));
            }
        }

        Ok(requests)
    }
}

impl Default for McpHttpHandler {
    fn default() -> Self {
        Self::new(vec!["*".to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_request() {
        let handler = McpHttpHandler::new(vec!["*".to_string()]);
        let body = r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#;

        let result = handler.validate_request(body.as_bytes());
        assert!(result.is_ok());

        let request = result.unwrap();
        assert_eq!(request.method, "tools/list");
    }

    #[test]
    fn test_invalid_json() {
        let handler = McpHttpHandler::new(vec!["*".to_string()]);
        let body = "not json";

        let result = handler.validate_request(body.as_bytes());
        assert!(matches!(result, Err(McpValidationError::InvalidJson(_))));
    }

    #[test]
    fn test_method_not_allowed() {
        let handler = McpHttpHandler::new(vec!["tools/list".to_string()]);
        let body = r#"{"jsonrpc":"2.0","method":"tools/call","id":1}"#;

        let result = handler.validate_request(body.as_bytes());
        assert!(matches!(result, Err(McpValidationError::MethodNotAllowed(_))));
    }

    #[test]
    fn test_wildcard_allows_all() {
        let handler = McpHttpHandler::new(vec!["*".to_string()]);

        assert!(handler.is_method_allowed("tools/list"));
        assert!(handler.is_method_allowed("tools/call"));
        assert!(handler.is_method_allowed("resources/read"));
    }

    #[test]
    fn test_batch_request() {
        let handler = McpHttpHandler::new(vec!["*".to_string()]);
        let body = r#"[{"jsonrpc":"2.0","method":"tools/list","id":1},{"jsonrpc":"2.0","method":"ping","id":2}]"#;

        let result = handler.validate_batch(body.as_bytes());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }
}
