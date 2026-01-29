//! A2A (Agent-to-Agent) Protocol Handler
//!
//! Supports A2A protocol bindings:
//! - JSONRPC (HTTP POST, application/json)
//! - gRPC (HTTP/2, application/grpc)
//! - HTTP+JSON (REST-style)

pub mod validator;
pub mod security;

pub use validator::{A2AMessage, A2ATask, A2AValidator, A2AValidationError};
pub use security::{A2ASecurityEnforcer, A2ASecurityError};

/// A2A protocol bindings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A2ABinding {
    /// JSON-RPC over HTTP POST
    JsonRpc,
    /// gRPC (HTTP/2)
    Grpc,
    /// HTTP+JSON (REST)
    HttpJson,
}

impl A2ABinding {
    /// Detect binding from headers
    pub fn detect(headers: &[(String, String)]) -> Option<Self> {
        for (name, value) in headers {
            let name_lower = name.to_lowercase();
            let value_lower = value.to_lowercase();

            if name_lower == "content-type" {
                if value_lower.contains("application/grpc") {
                    return Some(A2ABinding::Grpc);
                }
                if value_lower.contains("application/json") {
                    // Check for JSON-RPC vs REST
                    // JSON-RPC uses POST with specific structure
                    return Some(A2ABinding::JsonRpc);
                }
            }
        }

        None
    }
}

/// A2A handler for all bindings
pub struct A2AHandler {
    /// Validator
    validator: A2AValidator,
    /// Security enforcer
    security: A2ASecurityEnforcer,
    /// Allowed bindings
    allowed_bindings: Vec<A2ABinding>,
}

impl A2AHandler {
    /// Create a new A2A handler
    pub fn new() -> Self {
        Self {
            validator: A2AValidator::new(),
            security: A2ASecurityEnforcer::new(false), // TLS not required by default
            allowed_bindings: vec![A2ABinding::JsonRpc, A2ABinding::Grpc, A2ABinding::HttpJson],
        }
    }

    /// Create with TLS requirement
    pub fn with_tls(require_tls: bool) -> Self {
        Self {
            validator: A2AValidator::new(),
            security: A2ASecurityEnforcer::new(require_tls),
            allowed_bindings: vec![A2ABinding::JsonRpc, A2ABinding::Grpc, A2ABinding::HttpJson],
        }
    }

    /// Validate an A2A message
    pub fn validate_message(&self, body: &[u8]) -> Result<A2AMessage, A2AValidationError> {
        self.validator.validate_message(body)
    }

    /// Validate an A2A task
    pub fn validate_task(&self, body: &[u8]) -> Result<A2ATask, A2AValidationError> {
        self.validator.validate_task(body)
    }

    /// Check if binding is allowed
    pub fn is_binding_allowed(&self, binding: A2ABinding) -> bool {
        self.allowed_bindings.contains(&binding)
    }

    /// Get security enforcer
    pub fn security(&self) -> &A2ASecurityEnforcer {
        &self.security
    }

    /// Get validator
    pub fn validator(&self) -> &A2AValidator {
        &self.validator
    }
}

impl Default for A2AHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_grpc() {
        let headers = vec![("content-type".to_string(), "application/grpc".to_string())];
        assert_eq!(A2ABinding::detect(&headers), Some(A2ABinding::Grpc));
    }

    #[test]
    fn test_detect_json() {
        let headers = vec![("content-type".to_string(), "application/json".to_string())];
        assert_eq!(A2ABinding::detect(&headers), Some(A2ABinding::JsonRpc));
    }

    #[test]
    fn test_binding_allowed() {
        let handler = A2AHandler::new();
        assert!(handler.is_binding_allowed(A2ABinding::JsonRpc));
        assert!(handler.is_binding_allowed(A2ABinding::Grpc));
    }
}
