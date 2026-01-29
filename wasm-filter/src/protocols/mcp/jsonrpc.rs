//! JSON-RPC 2.0 Types for MCP
//!
//! MCP uses JSON-RPC 2.0 as its wire protocol.
//! This module provides validation and parsing.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// MUST be "2.0"
    pub jsonrpc: String,
    /// Request method
    pub method: String,
    /// Request parameters (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    /// Request ID (optional for notifications)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
}

impl JsonRpcRequest {
    /// Validate the request
    pub fn validate(&self) -> Result<(), JsonRpcValidationError> {
        // Check JSON-RPC version
        if self.jsonrpc != "2.0" {
            return Err(JsonRpcValidationError::InvalidVersion(self.jsonrpc.clone()));
        }

        // Check method is not empty
        if self.method.is_empty() {
            return Err(JsonRpcValidationError::EmptyMethod);
        }

        // Check method doesn't start with "rpc." (reserved)
        if self.method.starts_with("rpc.") {
            return Err(JsonRpcValidationError::ReservedMethod(self.method.clone()));
        }

        Ok(())
    }

    /// Check if this is a notification (no id)
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }

    /// Get the request ID as a string (for logging)
    pub fn id_string(&self) -> String {
        match &self.id {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            Some(v) => v.to_string(),
            None => "notification".to_string(),
        }
    }
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// MUST be "2.0"
    pub jsonrpc: String,
    /// Result (mutually exclusive with error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error (mutually exclusive with result)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    /// Response ID (MUST match request, null for errors to notifications)
    pub id: Value,
}

impl JsonRpcResponse {
    /// Create a success response
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response
    pub fn error(id: Value, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(error),
            id,
        }
    }

    /// Check if this is an error response
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

/// JSON-RPC 2.0 Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    /// Standard error: Parse error
    pub fn parse_error() -> Self {
        Self {
            code: -32700,
            message: "Parse error".to_string(),
            data: None,
        }
    }

    /// Standard error: Invalid Request
    pub fn invalid_request(message: &str) -> Self {
        Self {
            code: -32600,
            message: format!("Invalid Request: {}", message),
            data: None,
        }
    }

    /// Standard error: Method not found
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {}", method),
            data: None,
        }
    }

    /// Standard error: Invalid params
    pub fn invalid_params(message: &str) -> Self {
        Self {
            code: -32602,
            message: format!("Invalid params: {}", message),
            data: None,
        }
    }

    /// Standard error: Internal error
    pub fn internal_error(message: &str) -> Self {
        Self {
            code: -32603,
            message: format!("Internal error: {}", message),
            data: None,
        }
    }

    /// AI-Guard error: Policy violation
    pub fn policy_violation(reason: &str) -> Self {
        Self {
            code: -32000,
            message: format!("Policy violation: {}", reason),
            data: Some(serde_json::json!({
                "blocked_by": "ai-guard",
                "reason": reason
            })),
        }
    }
}

/// JSON-RPC validation errors
#[derive(Debug, Clone)]
pub enum JsonRpcValidationError {
    /// Invalid JSON-RPC version
    InvalidVersion(String),
    /// Empty method
    EmptyMethod,
    /// Reserved method (rpc.*)
    ReservedMethod(String),
}

impl std::fmt::Display for JsonRpcValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonRpcValidationError::InvalidVersion(v) => {
                write!(f, "Invalid JSON-RPC version: {} (expected 2.0)", v)
            }
            JsonRpcValidationError::EmptyMethod => write!(f, "Method cannot be empty"),
            JsonRpcValidationError::ReservedMethod(m) => {
                write!(f, "Reserved method prefix: {}", m)
            }
        }
    }
}

/// Common MCP method names
pub mod methods {
    /// Initialize connection
    pub const INITIALIZE: &str = "initialize";
    /// Shutdown connection
    pub const SHUTDOWN: &str = "shutdown";
    /// List available tools
    pub const TOOLS_LIST: &str = "tools/list";
    /// Call a tool
    pub const TOOLS_CALL: &str = "tools/call";
    /// List resources
    pub const RESOURCES_LIST: &str = "resources/list";
    /// Read a resource
    pub const RESOURCES_READ: &str = "resources/read";
    /// List prompts
    pub const PROMPTS_LIST: &str = "prompts/list";
    /// Get a prompt
    pub const PROMPTS_GET: &str = "prompts/get";
    /// Ping
    pub const PING: &str = "ping";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_request() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/list".to_string(),
            params: None,
            id: Some(Value::Number(1.into())),
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_invalid_version() {
        let request = JsonRpcRequest {
            jsonrpc: "1.0".to_string(),
            method: "test".to_string(),
            params: None,
            id: Some(Value::Number(1.into())),
        };

        assert!(matches!(
            request.validate(),
            Err(JsonRpcValidationError::InvalidVersion(_))
        ));
    }

    #[test]
    fn test_reserved_method() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "rpc.internal".to_string(),
            params: None,
            id: Some(Value::Number(1.into())),
        };

        assert!(matches!(
            request.validate(),
            Err(JsonRpcValidationError::ReservedMethod(_))
        ));
    }

    #[test]
    fn test_notification() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "notify".to_string(),
            params: None,
            id: None,
        };

        assert!(request.is_notification());
    }

    #[test]
    fn test_error_response() {
        let error = JsonRpcError::policy_violation("prompt injection detected");
        let response = JsonRpcResponse::error(Value::Number(1.into()), error);

        assert!(response.is_error());
    }
}
