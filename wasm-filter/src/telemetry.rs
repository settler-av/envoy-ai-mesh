//! Telemetry Module for AI-Guard
//!
//! Provides OpenTelemetry-compatible logging and metrics.
//! In Wasm, we emit structured logs that can be collected by
//! Envoy's access logging or external collectors.

use log::{info, warn};
use serde::Serialize;

/// Audit event types
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    /// Request allowed
    RequestAllowed,
    /// Request blocked
    RequestBlocked,
    /// PII detected
    PiiDetected,
    /// Rate limited
    RateLimited,
    /// A2AS control triggered
    A2asControl,
    /// STDIO bypass attempt
    StdioBypassAttempt,
}

/// Audit event for logging
#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    /// Event type
    pub event_type: AuditEventType,
    /// Timestamp (seconds since epoch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_secs: Option<u64>,
    /// Request ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Agent ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Protocol (MCP, A2A)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    /// Transport (HTTP, SSE, WebSocket)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    /// Method called
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Reason for action
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Pattern matched (if blocked)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_pattern: Option<String>,
    /// A2AS control that triggered
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a2as_control: Option<String>,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(event_type: AuditEventType) -> Self {
        Self {
            event_type,
            timestamp_secs: None,
            request_id: None,
            agent_id: None,
            protocol: None,
            transport: None,
            method: None,
            reason: None,
            matched_pattern: None,
            a2as_control: None,
            metadata: None,
        }
    }

    /// Set request ID
    pub fn with_request_id(mut self, id: &str) -> Self {
        self.request_id = Some(id.to_string());
        self
    }

    /// Set agent ID
    pub fn with_agent_id(mut self, id: &str) -> Self {
        self.agent_id = Some(id.to_string());
        self
    }

    /// Set protocol
    pub fn with_protocol(mut self, protocol: &str) -> Self {
        self.protocol = Some(protocol.to_string());
        self
    }

    /// Set transport
    pub fn with_transport(mut self, transport: &str) -> Self {
        self.transport = Some(transport.to_string());
        self
    }

    /// Set method
    pub fn with_method(mut self, method: &str) -> Self {
        self.method = Some(method.to_string());
        self
    }

    /// Set reason
    pub fn with_reason(mut self, reason: &str) -> Self {
        self.reason = Some(reason.to_string());
        self
    }

    /// Set matched pattern
    pub fn with_pattern(mut self, pattern: &str) -> Self {
        self.matched_pattern = Some(pattern.to_string());
        self
    }

    /// Set A2AS control
    pub fn with_a2as_control(mut self, control: &str) -> Self {
        self.a2as_control = Some(control.to_string());
        self
    }

    /// Log the event
    pub fn emit(&self) {
        // Serialize to JSON for structured logging
        match serde_json::to_string(self) {
            Ok(json) => {
                match self.event_type {
                    AuditEventType::RequestBlocked
                    | AuditEventType::StdioBypassAttempt
                    | AuditEventType::RateLimited => {
                        warn!("[AI-GUARD-AUDIT] {}", json);
                    }
                    _ => {
                        info!("[AI-GUARD-AUDIT] {}", json);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to serialize audit event: {}", e);
            }
        }
    }
}

/// Create a blocked request audit event
pub fn audit_blocked(reason: &str, pattern: Option<&str>) -> AuditEvent {
    let mut event = AuditEvent::new(AuditEventType::RequestBlocked)
        .with_reason(reason);
    
    if let Some(p) = pattern {
        event = event.with_pattern(p);
    }
    
    event
}

/// Create an allowed request audit event
pub fn audit_allowed() -> AuditEvent {
    AuditEvent::new(AuditEventType::RequestAllowed)
}

/// Create a PII detected audit event
pub fn audit_pii(pii_type: &str) -> AuditEvent {
    AuditEvent::new(AuditEventType::PiiDetected)
        .with_reason(&format!("PII type '{}' detected", pii_type))
}

/// Create a rate limited audit event
pub fn audit_rate_limited(limit: &str) -> AuditEvent {
    AuditEvent::new(AuditEventType::RateLimited)
        .with_reason(&format!("Rate limit '{}' exceeded", limit))
}

/// Create an A2AS control audit event
pub fn audit_a2as(control: &str, action: &str) -> AuditEvent {
    AuditEvent::new(AuditEventType::A2asControl)
        .with_a2as_control(control)
        .with_reason(action)
}

/// Create a STDIO bypass attempt audit event
pub fn audit_stdio_bypass(description: &str) -> AuditEvent {
    AuditEvent::new(AuditEventType::StdioBypassAttempt)
        .with_reason(description)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_serialization() {
        let event = AuditEvent::new(AuditEventType::RequestBlocked)
            .with_request_id("req-123")
            .with_reason("prompt injection")
            .with_pattern("ignore previous");

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("request_blocked"));
        assert!(json.contains("ignore previous"));
    }

    #[test]
    fn test_audit_blocked() {
        let event = audit_blocked("prompt injection", Some("jailbreak"));
        assert!(event.matched_pattern.is_some());
    }

    #[test]
    fn test_audit_pii() {
        let event = audit_pii("ssn");
        assert!(event.reason.as_ref().unwrap().contains("ssn"));
    }
}
