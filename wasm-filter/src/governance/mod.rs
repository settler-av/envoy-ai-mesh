//! Governance module for AI-Guard
//!
//! This module provides:
//! - Streaming body scanner
//! - Prompt injection detection
//! - PII redaction
//! - Token counting
//! - Rate limiting

pub mod body_scanner;
pub mod prompt_injection;
pub mod pii_redaction;
pub mod token_counter;
pub mod rate_limiter;

pub use body_scanner::{StreamingBodyScanner, ScanDecision};
pub use prompt_injection::PromptInjectionDetector;
pub use pii_redaction::{PiiRedactor, PiiMatch, PiiType};
pub use token_counter::{TokenCounter, TokenUsage};
pub use rate_limiter::{RateLimiter, RateDecision};
