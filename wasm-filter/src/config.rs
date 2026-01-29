//! Configuration module for AI-Guard Wasm Filter
//!
//! CRITICAL: Configuration is loaded from Envoy plugin configuration,
//! NOT from external files. This avoids file I/O in the Wasm sandbox.

use serde::Deserialize;

/// Filter configuration loaded from Envoy plugin configuration
#[derive(Clone, Debug, Deserialize)]
pub struct FilterConfig {
    /// Patterns to detect in request body (prompt injection signatures)
    #[serde(default = "default_blocked_patterns")]
    pub blocked_patterns: Vec<String>,

    /// PII types to detect
    #[serde(default = "default_pii_types")]
    pub pii_types: Vec<String>,

    /// MCP methods allowed
    #[serde(default = "default_mcp_methods")]
    pub mcp_allowed_methods: Vec<String>,

    /// Maximum body size to inspect (prevent OOM)
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,

    /// Ring buffer size for streaming inspection
    #[serde(default = "default_ring_buffer_size")]
    pub ring_buffer_size: usize,

    /// Whether to log matched patterns (for debugging)
    #[serde(default = "default_log_matches")]
    pub log_matches: bool,
}

fn default_blocked_patterns() -> Vec<String> {
    vec![
        "ignore previous instructions".to_string(),
        "ignore all previous".to_string(),
        "disregard previous".to_string(),
        "forget your instructions".to_string(),
        "override your instructions".to_string(),
        "ignore your system prompt".to_string(),
        "bypass your restrictions".to_string(),
        "jailbreak".to_string(),
        "DAN mode".to_string(),
        "delete database".to_string(),
        "drop table".to_string(),
        "rm -rf".to_string(),
    ]
}

fn default_pii_types() -> Vec<String> {
    vec![
        "ssn".to_string(),
        "credit_card".to_string(),
        "email".to_string(),
    ]
}

fn default_mcp_methods() -> Vec<String> {
    vec!["*".to_string()]
}

fn default_max_body_size() -> usize {
    10 * 1024 * 1024 // 10MB
}

fn default_ring_buffer_size() -> usize {
    64 * 1024 // 64KB
}

fn default_log_matches() -> bool {
    true
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            blocked_patterns: default_blocked_patterns(),
            pii_types: default_pii_types(),
            mcp_allowed_methods: default_mcp_methods(),
            max_body_size: default_max_body_size(),
            ring_buffer_size: default_ring_buffer_size(),
            log_matches: default_log_matches(),
        }
    }
}

impl FilterConfig {
    /// Parse configuration from JSON bytes (from Envoy plugin configuration)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ConfigError> {
        let config_str = std::str::from_utf8(bytes)
            .map_err(|e| ConfigError::InvalidUtf8(e.to_string()))?;
        
        serde_json::from_str(config_str)
            .map_err(|e| ConfigError::InvalidJson(e.to_string()))
    }

    /// Check if an MCP method is allowed
    pub fn is_mcp_method_allowed(&self, method: &str) -> bool {
        self.mcp_allowed_methods.iter().any(|m| m == "*" || m == method)
    }
}

/// Configuration parsing errors
#[derive(Debug)]
pub enum ConfigError {
    InvalidUtf8(String),
    InvalidJson(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::InvalidUtf8(e) => write!(f, "Invalid UTF-8: {}", e),
            ConfigError::InvalidJson(e) => write!(f, "Invalid JSON: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FilterConfig::default();
        assert!(!config.blocked_patterns.is_empty());
        assert!(config.max_body_size > 0);
        assert!(config.ring_buffer_size > 0);
    }

    #[test]
    fn test_parse_config() {
        let json = r#"{"blocked_patterns": ["test"], "max_body_size": 1024}"#;
        let config = FilterConfig::from_bytes(json.as_bytes()).unwrap();
        assert_eq!(config.blocked_patterns, vec!["test"]);
        assert_eq!(config.max_body_size, 1024);
    }

    #[test]
    fn test_mcp_method_allowed() {
        let config = FilterConfig::default();
        assert!(config.is_mcp_method_allowed("tools/call"));
        
        let restricted = FilterConfig {
            mcp_allowed_methods: vec!["tools/list".to_string()],
            ..Default::default()
        };
        assert!(restricted.is_mcp_method_allowed("tools/list"));
        assert!(!restricted.is_mcp_method_allowed("tools/call"));
    }
}
