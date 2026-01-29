//! STDIO Transport Detection
//!
//! STDIO transport bypasses the mesh entirely - there's no network traffic
//! to intercept. This module provides detection and audit logging.
//!
//! STDIO blocking is enforced at multiple layers:
//! 1. Kubernetes NetworkPolicy (block non-mesh egress)
//! 2. Kyverno policy (block stdio commands in container args)
//! 3. Audit logging (detect stdio usage attempts)

/// STDIO bypass detection result
#[derive(Debug, Clone)]
pub struct StdioBypassAttempt {
    /// Type of bypass detected
    pub bypass_type: StdioBypassType,
    /// Description
    pub description: String,
    /// Severity
    pub severity: StdioSeverity,
}

/// Types of STDIO bypass
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdioBypassType {
    /// Header indicates stdio transport
    HeaderIndicator,
    /// Command pattern suggests stdio usage
    CommandPattern,
    /// Process spawn attempt
    ProcessSpawn,
}

/// Severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdioSeverity {
    /// Low - might be false positive
    Low,
    /// Medium - likely stdio attempt
    Medium,
    /// High - definite stdio attempt
    High,
}

/// STDIO detector
pub struct StdioDetector {
    /// Known STDIO MCP server commands
    known_commands: Vec<String>,
}

impl StdioDetector {
    /// Create a new STDIO detector
    pub fn new() -> Self {
        Self {
            known_commands: vec![
                "npx".to_string(),
                "uvx".to_string(),
                "python -m".to_string(),
                "node".to_string(),
                "mcp-server".to_string(),
                "stdio".to_string(),
            ],
        }
    }

    /// Detect STDIO bypass from headers
    pub fn detect_from_headers(&self, headers: &[(String, String)]) -> Option<StdioBypassAttempt> {
        for (name, value) in headers {
            let name_lower = name.to_lowercase();
            let value_lower = value.to_lowercase();

            // Check for explicit stdio transport header
            if name_lower == "x-mcp-transport" && value_lower == "stdio" {
                return Some(StdioBypassAttempt {
                    bypass_type: StdioBypassType::HeaderIndicator,
                    description: "x-mcp-transport header indicates STDIO transport".to_string(),
                    severity: StdioSeverity::High,
                });
            }

            // Check for stdio in other headers
            if value_lower.contains("stdio") {
                return Some(StdioBypassAttempt {
                    bypass_type: StdioBypassType::HeaderIndicator,
                    description: format!("STDIO reference in header {}: {}", name, value),
                    severity: StdioSeverity::Medium,
                });
            }
        }

        None
    }

    /// Detect STDIO patterns in request body
    pub fn detect_in_body(&self, body: &str) -> Option<StdioBypassAttempt> {
        let body_lower = body.to_lowercase();

        // Check for known STDIO command patterns
        for cmd in &self.known_commands {
            if body_lower.contains(&cmd.to_lowercase()) {
                // Check if it looks like a command invocation
                if body_lower.contains("command") || body_lower.contains("exec") {
                    return Some(StdioBypassAttempt {
                        bypass_type: StdioBypassType::CommandPattern,
                        description: format!("Possible STDIO MCP server command: {}", cmd),
                        severity: StdioSeverity::Medium,
                    });
                }
            }
        }

        // Check for explicit stdio mention
        if body_lower.contains("stdio") && body_lower.contains("transport") {
            return Some(StdioBypassAttempt {
                bypass_type: StdioBypassType::HeaderIndicator,
                description: "STDIO transport configuration in request body".to_string(),
                severity: StdioSeverity::High,
            });
        }

        None
    }

    /// Create audit event for STDIO bypass attempt
    pub fn create_audit_event(&self, attempt: &StdioBypassAttempt) -> StdioAuditEvent {
        StdioAuditEvent {
            event_type: "stdio_bypass_attempt".to_string(),
            bypass_type: format!("{:?}", attempt.bypass_type),
            description: attempt.description.clone(),
            severity: format!("{:?}", attempt.severity),
            action_taken: "blocked".to_string(),
            recommendation: "Use HTTP, SSE, or WebSocket transport for mesh visibility".to_string(),
        }
    }
}

impl Default for StdioDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Audit event for STDIO bypass attempts
#[derive(Debug, Clone)]
pub struct StdioAuditEvent {
    /// Event type
    pub event_type: String,
    /// Bypass type
    pub bypass_type: String,
    /// Description
    pub description: String,
    /// Severity
    pub severity: String,
    /// Action taken
    pub action_taken: String,
    /// Recommendation
    pub recommendation: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_stdio_header() {
        let detector = StdioDetector::new();
        let headers = vec![("x-mcp-transport".to_string(), "stdio".to_string())];

        let result = detector.detect_from_headers(&headers);

        assert!(result.is_some());
        assert_eq!(result.unwrap().bypass_type, StdioBypassType::HeaderIndicator);
    }

    #[test]
    fn test_no_detection_http() {
        let detector = StdioDetector::new();
        let headers = vec![("x-mcp-transport".to_string(), "http".to_string())];

        let result = detector.detect_from_headers(&headers);

        assert!(result.is_none());
    }

    #[test]
    fn test_detect_command_pattern() {
        let detector = StdioDetector::new();
        let body = r#"{"command": "npx @modelcontextprotocol/server-filesystem"}"#;

        let result = detector.detect_in_body(body);

        assert!(result.is_some());
    }

    #[test]
    fn test_detect_stdio_config() {
        let detector = StdioDetector::new();
        let body = r#"{"transport": "stdio", "server": "..."}"#;

        let result = detector.detect_in_body(body);

        assert!(result.is_some());
    }
}
