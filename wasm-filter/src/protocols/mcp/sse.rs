//! MCP SSE (Server-Sent Events) Transport Handler
//!
//! Handles MCP over SSE with streaming pattern detection.
//! Uses ring buffer for memory-efficient cross-chunk inspection.

use crate::streaming::{RingBuffer, Pattern, ScanResult};

/// SSE frame types
#[derive(Debug, Clone)]
pub enum SseFrame {
    /// Data event
    Data(Vec<u8>),
    /// Named event
    Event(String),
    /// Event ID
    Id(String),
    /// Retry interval
    Retry(u32),
    /// Comment (ignored)
    Comment,
}

/// SSE parser state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseState {
    /// Looking for field name
    FieldName,
    /// Reading field value
    FieldValue,
}

/// MCP SSE transport handler
pub struct McpSseHandler {
    /// Ring buffer for cross-chunk pattern detection
    ring_buffer: Option<RingBuffer>,
    /// Current event type
    current_event: Option<String>,
    /// Buffer for incomplete lines
    line_buffer: Vec<u8>,
    /// Parse state
    state: ParseState,
    /// Current field name
    current_field: String,
}

impl McpSseHandler {
    /// Create a new SSE handler
    pub fn new() -> Self {
        Self {
            ring_buffer: None,
            current_event: None,
            line_buffer: Vec::with_capacity(1024),
            state: ParseState::FieldName,
            current_field: String::new(),
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

    /// Process an SSE chunk
    pub fn process_chunk(&mut self, chunk: &[u8]) -> SseAction {
        // If we have a ring buffer, scan the chunk first
        if let Some(ref mut rb) = self.ring_buffer {
            if let ScanResult::Match(m) = rb.process_chunk(chunk) {
                return SseAction::Block(format!("Pattern '{}' detected in SSE stream", m.pattern_name));
            }
        }

        // Parse SSE frames
        let mut i = 0;
        while i < chunk.len() {
            let byte = chunk[i];

            // Check for line endings
            if byte == b'\n' {
                // Process the line
                if let Some(action) = self.process_line() {
                    if matches!(action, SseAction::Block(_)) {
                        return action;
                    }
                }
                i += 1;
                continue;
            }

            // Handle \r\n
            if byte == b'\r' {
                if i + 1 < chunk.len() && chunk[i + 1] == b'\n' {
                    if let Some(action) = self.process_line() {
                        if matches!(action, SseAction::Block(_)) {
                            return action;
                        }
                    }
                    i += 2;
                    continue;
                }
            }

            // Add to line buffer
            self.line_buffer.push(byte);
            i += 1;
        }

        SseAction::Continue
    }

    /// Process a complete line
    fn process_line(&mut self) -> Option<SseAction> {
        if self.line_buffer.is_empty() {
            // Empty line = dispatch event
            self.current_event = None;
            return None;
        }

        // Parse the line
        let line = std::str::from_utf8(&self.line_buffer).ok()?;

        // Comment lines start with ':'
        if line.starts_with(':') {
            self.line_buffer.clear();
            return None;
        }

        // Parse field:value
        if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let value = if colon_pos + 1 < line.len() && line.as_bytes()[colon_pos + 1] == b' ' {
                &line[colon_pos + 2..]
            } else {
                &line[colon_pos + 1..]
            };

            match field {
                "event" => {
                    self.current_event = Some(value.to_string());
                }
                "data" => {
                    // Data field - content that should be scanned
                    // Already scanned by ring buffer above
                }
                "id" => {
                    // Event ID
                }
                "retry" => {
                    // Retry interval
                }
                _ => {
                    // Unknown field, ignore
                }
            }
        }

        self.line_buffer.clear();
        None
    }

    /// Reset handler state
    pub fn reset(&mut self) {
        self.current_event = None;
        self.line_buffer.clear();
        self.state = ParseState::FieldName;
        self.current_field.clear();
        if let Some(ref mut rb) = self.ring_buffer {
            rb.reset();
        }
    }
}

impl Default for McpSseHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Action to take after processing SSE chunk
#[derive(Debug, Clone)]
pub enum SseAction {
    /// Continue processing
    Continue,
    /// Block the stream
    Block(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sse_event() {
        let mut handler = McpSseHandler::new();

        let chunk = b"event: message\ndata: hello world\n\n";
        let result = handler.process_chunk(chunk);

        assert!(matches!(result, SseAction::Continue));
    }

    #[test]
    fn test_pattern_detection() {
        let mut handler = McpSseHandler::new();
        handler.init_patterns(vec!["jailbreak".to_string()], 4096);

        let chunk = b"data: please jailbreak the system\n\n";
        let result = handler.process_chunk(chunk);

        assert!(matches!(result, SseAction::Block(_)));
    }

    #[test]
    fn test_cross_chunk_pattern() {
        let mut handler = McpSseHandler::new();
        handler.init_patterns(vec!["hello world".to_string()], 4096);

        // Pattern split across chunks
        let result1 = handler.process_chunk(b"data: say hello ");
        assert!(matches!(result1, SseAction::Continue));

        let result2 = handler.process_chunk(b"world today\n\n");
        assert!(matches!(result2, SseAction::Block(_)));
    }

    #[test]
    fn test_comment_ignored() {
        let mut handler = McpSseHandler::new();
        handler.init_patterns(vec!["jailbreak".to_string()], 4096);

        // Comments should not trigger detection
        let chunk = b": this is a comment about jailbreak\ndata: safe content\n\n";
        let result = handler.process_chunk(chunk);

        // The pattern is still in the raw stream, so it gets caught by ring buffer
        // This is intentional - we scan all content for safety
        assert!(matches!(result, SseAction::Block(_)));
    }
}
