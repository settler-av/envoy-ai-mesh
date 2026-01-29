//! MCP WebSocket Transport Handler
//!
//! Handles MCP over WebSocket with bidirectional frame inspection.
//! MCP only uses text frames (JSON-RPC), binary frames are blocked.

use crate::streaming::{RingBuffer, Pattern, ScanResult};
use super::jsonrpc::JsonRpcRequest;

/// WebSocket opcode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsOpcode {
    /// Continuation frame
    Continuation = 0x0,
    /// Text frame (UTF-8)
    Text = 0x1,
    /// Binary frame
    Binary = 0x2,
    /// Connection close
    Close = 0x8,
    /// Ping
    Ping = 0x9,
    /// Pong
    Pong = 0xA,
    /// Unknown
    Unknown = 0xFF,
}

impl From<u8> for WsOpcode {
    fn from(byte: u8) -> Self {
        match byte & 0x0F {
            0x0 => WsOpcode::Continuation,
            0x1 => WsOpcode::Text,
            0x2 => WsOpcode::Binary,
            0x8 => WsOpcode::Close,
            0x9 => WsOpcode::Ping,
            0xA => WsOpcode::Pong,
            _ => WsOpcode::Unknown,
        }
    }
}

/// WebSocket connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsState {
    /// Connection open
    Open,
    /// Closing handshake initiated
    Closing,
    /// Connection closed
    Closed,
}

/// MCP WebSocket transport handler
pub struct McpWebSocketHandler {
    /// Connection state
    state: WsState,
    /// Ring buffer for pattern detection
    ring_buffer: Option<RingBuffer>,
    /// Buffer for fragmented messages
    fragment_buffer: Vec<u8>,
    /// Current fragment opcode
    fragment_opcode: Option<WsOpcode>,
    /// Message counter
    message_count: u64,
}

impl McpWebSocketHandler {
    /// Create a new WebSocket handler
    pub fn new() -> Self {
        Self {
            state: WsState::Open,
            ring_buffer: None,
            fragment_buffer: Vec::with_capacity(4096),
            fragment_opcode: None,
            message_count: 0,
        }
    }

    /// Initialize ring buffer with patterns
    pub fn init_patterns(&mut self, patterns: Vec<String>, buffer_size: usize) {
        let patterns: Vec<Pattern> = patterns
            .iter()
            .map(|s| Pattern::from_string(s))
            .collect();
        self.ring_buffer = Some(RingBuffer::new(buffer_size, patterns));
    }

    /// Process a WebSocket frame
    pub fn on_frame(&mut self, opcode: WsOpcode, payload: &[u8], fin: bool) -> WsFrameAction {
        match opcode {
            WsOpcode::Text => {
                // Text frames contain JSON-RPC messages
                self.on_text_frame(payload, fin)
            }
            WsOpcode::Binary => {
                // Binary frames not allowed for MCP
                WsFrameAction::Block("Binary WebSocket frames not allowed for MCP".to_string())
            }
            WsOpcode::Continuation => {
                // Continue fragmented message
                self.on_continuation_frame(payload, fin)
            }
            WsOpcode::Close => {
                self.state = WsState::Closing;
                WsFrameAction::Continue
            }
            WsOpcode::Ping | WsOpcode::Pong => {
                // Control frames, allow through
                WsFrameAction::Continue
            }
            WsOpcode::Unknown => {
                WsFrameAction::Block("Unknown WebSocket opcode".to_string())
            }
        }
    }

    /// Process a text frame
    fn on_text_frame(&mut self, payload: &[u8], fin: bool) -> WsFrameAction {
        // Scan payload for patterns
        if let Some(ref mut rb) = self.ring_buffer {
            if let ScanResult::Match(m) = rb.process_chunk(payload) {
                return WsFrameAction::Block(format!(
                    "Pattern '{}' detected in WebSocket message",
                    m.pattern_name
                ));
            }
        }

        if fin {
            // Complete message
            self.message_count += 1;

            // Validate JSON-RPC if we have the full payload
            if let Err(e) = self.validate_message(payload) {
                return WsFrameAction::Block(e);
            }
        } else {
            // Start of fragmented message
            self.fragment_opcode = Some(WsOpcode::Text);
            self.fragment_buffer.extend_from_slice(payload);
        }

        WsFrameAction::Continue
    }

    /// Process a continuation frame
    fn on_continuation_frame(&mut self, payload: &[u8], fin: bool) -> WsFrameAction {
        // Scan payload for patterns
        if let Some(ref mut rb) = self.ring_buffer {
            if let ScanResult::Match(m) = rb.process_chunk(payload) {
                return WsFrameAction::Block(format!(
                    "Pattern '{}' detected in WebSocket message",
                    m.pattern_name
                ));
            }
        }

        // Check if we're expecting a continuation
        if self.fragment_opcode.is_none() {
            return WsFrameAction::Block("Unexpected continuation frame".to_string());
        }

        // Limit fragment buffer size to prevent DoS
        if self.fragment_buffer.len() + payload.len() > 10 * 1024 * 1024 {
            self.fragment_buffer.clear();
            self.fragment_opcode = None;
            return WsFrameAction::Block("WebSocket message too large".to_string());
        }

        self.fragment_buffer.extend_from_slice(payload);

        if fin {
            // Complete fragmented message
            self.message_count += 1;

            // Validate if it was a text message
            if self.fragment_opcode == Some(WsOpcode::Text) {
                if let Err(e) = self.validate_message(&self.fragment_buffer) {
                    self.fragment_buffer.clear();
                    self.fragment_opcode = None;
                    return WsFrameAction::Block(e);
                }
            }

            self.fragment_buffer.clear();
            self.fragment_opcode = None;
        }

        WsFrameAction::Continue
    }

    /// Validate a JSON-RPC message
    fn validate_message(&self, payload: &[u8]) -> Result<(), String> {
        // Try to parse as JSON-RPC
        let text = std::str::from_utf8(payload)
            .map_err(|_| "Invalid UTF-8 in WebSocket message".to_string())?;

        // Parse JSON
        let request: Result<JsonRpcRequest, _> = serde_json::from_str(text);
        if let Ok(req) = request {
            // Validate JSON-RPC format
            if let Err(e) = req.validate() {
                return Err(format!("Invalid JSON-RPC: {}", e));
            }
        }
        // If it's not a valid request, it might be a response or notification - allow

        Ok(())
    }

    /// Get connection state
    pub fn state(&self) -> WsState {
        self.state
    }

    /// Get message count
    pub fn message_count(&self) -> u64 {
        self.message_count
    }

    /// Reset handler state
    pub fn reset(&mut self) {
        self.state = WsState::Open;
        self.fragment_buffer.clear();
        self.fragment_opcode = None;
        self.message_count = 0;
        if let Some(ref mut rb) = self.ring_buffer {
            rb.reset();
        }
    }
}

impl Default for McpWebSocketHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Action to take after processing WebSocket frame
#[derive(Debug, Clone)]
pub enum WsFrameAction {
    /// Continue processing
    Continue,
    /// Block the message
    Block(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_frame() {
        let mut handler = McpWebSocketHandler::new();
        handler.init_patterns(vec!["jailbreak".to_string()], 4096);

        let payload = r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#;
        let result = handler.on_frame(WsOpcode::Text, payload.as_bytes(), true);

        assert!(matches!(result, WsFrameAction::Continue));
    }

    #[test]
    fn test_binary_blocked() {
        let mut handler = McpWebSocketHandler::new();

        let result = handler.on_frame(WsOpcode::Binary, &[0x00, 0x01, 0x02], true);

        assert!(matches!(result, WsFrameAction::Block(_)));
    }

    #[test]
    fn test_pattern_detection() {
        let mut handler = McpWebSocketHandler::new();
        handler.init_patterns(vec!["jailbreak".to_string()], 4096);

        let payload = r#"{"jsonrpc":"2.0","method":"prompt","params":{"text":"jailbreak"},"id":1}"#;
        let result = handler.on_frame(WsOpcode::Text, payload.as_bytes(), true);

        assert!(matches!(result, WsFrameAction::Block(_)));
    }

    #[test]
    fn test_fragmented_message() {
        let mut handler = McpWebSocketHandler::new();

        // First fragment
        let result1 = handler.on_frame(WsOpcode::Text, b"{\"jsonrpc\":", false);
        assert!(matches!(result1, WsFrameAction::Continue));

        // Continuation
        let result2 = handler.on_frame(WsOpcode::Continuation, b"\"2.0\"}", true);
        assert!(matches!(result2, WsFrameAction::Continue));
    }

    #[test]
    fn test_close_frame() {
        let mut handler = McpWebSocketHandler::new();
        assert_eq!(handler.state(), WsState::Open);

        handler.on_frame(WsOpcode::Close, &[], true);
        assert_eq!(handler.state(), WsState::Closing);
    }
}
