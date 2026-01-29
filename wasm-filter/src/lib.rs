//! AI-Guard Wasm Filter for Envoy Proxy
//!
//! Headless AI Governance using the Distributed Interceptor Pattern.
//! Implements streaming body inspection with constant memory usage.
//!
//! Key features:
//! - Streaming body scanner (ring buffer, no accumulation)
//! - UTF-8 boundary handling across chunks
//! - FSM-based pattern matching (no regex)
//! - Prompt injection detection
//! - PII detection
//! - Token counting and rate limiting
//!
//! Targets: wasm32-wasi (Envoy proxy-wasm ABI)

use log::{debug, info, warn};
use proxy_wasm::traits::{Context, HttpContext, RootContext};
use proxy_wasm::types::{Action, ContextType, LogLevel};
use std::cell::RefCell;

pub mod config;
pub mod streaming;
pub mod governance;
pub mod protocols;
pub mod telemetry;

use config::FilterConfig;
use governance::{ScanDecision, StreamingBodyScanner, TokenCounter};

// Thread-local storage for filter configuration
thread_local! {
    static CONFIG: RefCell<FilterConfig> = RefCell::new(FilterConfig::default());
}

/// Root context for filter lifecycle management
struct AiGuardRootContext {
    config: FilterConfig,
}

impl AiGuardRootContext {
    fn new() -> Self {
        Self {
            config: FilterConfig::default(),
        }
    }
}

impl Context for AiGuardRootContext {}

impl RootContext for AiGuardRootContext {
    fn on_configure(&mut self, _plugin_configuration_size: usize) -> bool {
        // CRITICAL: Load configuration from Envoy plugin configuration, NOT external files
        if let Some(config_bytes) = self.get_plugin_configuration() {
            match FilterConfig::from_bytes(&config_bytes) {
                Ok(config) => {
                    info!(
                        "AI-Guard: Loaded configuration with {} blocked patterns",
                        config.blocked_patterns.len()
                    );
                    self.config = config;
                }
                Err(e) => {
                    warn!("AI-Guard: Failed to parse config: {}, using defaults", e);
                }
            }
        } else {
            info!("AI-Guard: No configuration provided, using defaults");
        }

        // Store config in thread-local for HTTP contexts to access
        CONFIG.with(|c| {
            *c.borrow_mut() = self.config.clone();
        });

        info!(
            "AI-Guard Filter initialized - {} patterns, {}KB ring buffer",
            self.config.blocked_patterns.len(),
            self.config.ring_buffer_size / 1024
        );

        true
    }

    fn create_http_context(&self, context_id: u32) -> Option<Box<dyn HttpContext>> {
        Some(Box::new(AiGuardHttpContext::new(context_id)))
    }

    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }
}

/// HTTP context for per-request processing
///
/// CRITICAL: Uses streaming body scanner - does NOT accumulate body in memory.
struct AiGuardHttpContext {
    context_id: u32,
    /// Streaming body scanner (ring buffer based)
    scanner: StreamingBodyScanner,
    /// Token counter for cost attribution
    token_counter: TokenCounter,
    /// Track if we've already sent a block response
    request_blocked: bool,
    /// Configuration snapshot for this request
    config: FilterConfig,
    /// Content type of request
    is_text_content: bool,
    /// Number of request-body bytes already processed.
    ///
    /// CRITICAL: In proxy-wasm, `body_size` in `on_http_request_body` is the
    /// size of the buffered body so far (not just the new chunk). We must
    /// only read and scan the newly appended bytes to avoid reprocessing and
    /// to keep filter memory usage flat.
    body_bytes_processed: usize,
}

impl AiGuardHttpContext {
    fn new(context_id: u32) -> Self {
        let config = CONFIG.with(|c| c.borrow().clone());
        let scanner = StreamingBodyScanner::new(&config);

        Self {
            context_id,
            scanner,
            token_counter: TokenCounter::new(),
            request_blocked: false,
            config,
            is_text_content: true,
            body_bytes_processed: 0,
        }
    }

    /// Send a 403 Forbidden response with JSON error body
    fn send_block_response(&mut self, reason: &str) {
        if self.request_blocked {
            return; // Already blocked, don't send duplicate response
        }

        self.request_blocked = true;

        let error_body = serde_json::json!({
            "error": "Request Blocked by AI-Guard",
            "reason": reason,
            "status": 403,
            "headers": {
                "x-ai-guard-blocked": "true",
                "x-ai-guard-reason": "policy-violation"
            }
        });

        let body_bytes = error_body.to_string();

        warn!(
            "[context_id={}] BLOCKED: {}",
            self.context_id, reason
        );

        self.send_http_response(
            403,
            vec![
                ("content-type", "application/json"),
                ("x-ai-guard-blocked", "true"),
                ("x-ai-guard-action", "block"),
            ],
            Some(body_bytes.as_bytes()),
        );
    }
}

impl Context for AiGuardHttpContext {}

impl HttpContext for AiGuardHttpContext {
    fn on_http_request_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        debug!(
            "[context_id={}] Processing request headers",
            self.context_id
        );

        // Log request path for debugging
        if let Some(path) = self.get_http_request_header(":path") {
            debug!("[context_id={}] Request path: {}", self.context_id, path);
        }

        // Check Content-Type - only inspect JSON/text bodies
        if let Some(content_type) = self.get_http_request_header("content-type") {
            let ct_lower = content_type.to_lowercase();
            if !ct_lower.contains("json")
                && !ct_lower.contains("text")
                && !ct_lower.contains("form")
            {
                debug!(
                    "[context_id={}] Skipping non-text content-type: {}",
                    self.context_id, content_type
                );
                self.is_text_content = false;
                return Action::Continue;
            }
        }

        Action::Continue
    }

    fn on_http_request_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        // If already blocked, don't process further
        if self.request_blocked {
            return Action::Pause;
        }

        // Skip inspection for non-text content
        if !self.is_text_content {
            return Action::Continue;
        }

        debug!(
            "[context_id={}] Body chunk: {} bytes, end_of_stream: {}",
            self.context_id, body_size, end_of_stream
        );

        // Only read the newly appended bytes (do NOT re-read the full body).
        if body_size < self.body_bytes_processed {
            // Body buffer was reset by Envoy (unexpected), reset our cursor.
            self.body_bytes_processed = 0;
        }
        let new_len = body_size.saturating_sub(self.body_bytes_processed);

        if new_len == 0 {
            return if end_of_stream { Action::Continue } else { Action::Pause };
        }

        if let Some(new_bytes) = self.get_http_request_body(self.body_bytes_processed, new_len) {
            self.body_bytes_processed += new_bytes.len();

            // CRITICAL: Stream through scanner - O(n) time, O(1) filter memory
            match self.scanner.on_body_chunk(&new_bytes, end_of_stream) {
                ScanDecision::Block(reason) => {
                    self.send_block_response(&reason);
                    return Action::Pause;
                }
                ScanDecision::Continue => {
                    // More chunks expected, keep buffering
                    return Action::Pause;
                }
                ScanDecision::Allow => {
                    // Body is safe, forward to upstream
                    debug!(
                        "[context_id={}] Body passed security check ({} bytes)",
                        self.context_id,
                        self.scanner.total_bytes()
                    );
                }
                ScanDecision::Skip(reason) => {
                    debug!(
                        "[context_id={}] Skipping scan: {}",
                        self.context_id, reason
                    );
                }
            }
        }

        Action::Continue
    }

    fn on_http_response_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        // Add header to indicate request was inspected
        self.set_http_response_header("x-ai-guard-inspected", Some("true"));

        Action::Continue
    }

    fn on_http_response_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        // Extract token usage from response body (for cost attribution)
        if end_of_stream {
            if let Some(body) = self.get_http_response_body(0, body_size) {
                if let Some(usage) = self.token_counter.extract_from_body(&body) {
                    info!(
                        "[context_id={}] Token usage: prompt={}, completion={}, total={}",
                        self.context_id,
                        usage.prompt_tokens,
                        usage.completion_tokens,
                        usage.total_tokens
                    );

                    if let Some(cost) = usage.estimated_cost_usd {
                        info!(
                            "[context_id={}] Estimated cost: ${:.4}",
                            self.context_id, cost
                        );
                    }

                    // Add usage headers for observability
                    self.set_http_response_header(
                        "x-ai-guard-tokens-total",
                        Some(&usage.total_tokens.to_string()),
                    );
                }
            }
        }

        Action::Continue
    }

    fn on_log(&mut self) {
        // Log completion of request processing
        if self.request_blocked {
            info!(
                "[context_id={}] Request BLOCKED by AI-Guard",
                self.context_id
            );
        } else {
            debug!(
                "[context_id={}] Request processing complete ({} bytes scanned)",
                self.context_id,
                self.scanner.total_bytes()
            );
        }
    }
}

// Register the filter with proxy-wasm runtime
proxy_wasm::main! {{
    proxy_wasm::set_log_level(LogLevel::Debug);
    proxy_wasm::set_root_context(|_| -> Box<dyn RootContext> {
        Box::new(AiGuardRootContext::new())
    });
}}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = FilterConfig::default();
        assert!(!config.blocked_patterns.is_empty());
        assert!(config.max_body_size > 0);
        assert!(config.ring_buffer_size > 0);
    }

    #[test]
    fn test_scanner_creation() {
        let config = FilterConfig::default();
        let scanner = StreamingBodyScanner::new(&config);
        assert!(!scanner.is_complete());
    }
}
