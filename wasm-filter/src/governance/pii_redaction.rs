//! PII Detection and Redaction Module
//!
//! This module provides detection and optional redaction of
//! Personally Identifiable Information (PII) in request/response bodies.
//!
//! Uses FSM-based pattern matching (no regex) for constant memory.

use crate::streaming::{Pattern, PatternScanner, ScanResult};

/// PII types that can be detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiiType {
    /// Social Security Number (XXX-XX-XXXX)
    Ssn,
    /// Credit Card Number (16 digits with optional separators)
    CreditCard,
    /// Email Address
    Email,
    /// Phone Number (various formats)
    Phone,
}

impl PiiType {
    /// Get the redaction placeholder for this PII type
    pub fn placeholder(&self) -> &'static str {
        match self {
            PiiType::Ssn => "[SSN REDACTED]",
            PiiType::CreditCard => "[CREDIT CARD REDACTED]",
            PiiType::Email => "[EMAIL REDACTED]",
            PiiType::Phone => "[PHONE REDACTED]",
        }
    }
}

/// PII match result
#[derive(Debug, Clone)]
pub struct PiiMatch {
    /// Type of PII detected
    pub pii_type: PiiType,
    /// Start position in the text
    pub start: usize,
    /// End position in the text
    pub end: usize,
    /// The matched value (for logging, may be partial)
    pub value_hint: String,
}

/// PII Redactor
///
/// Note: This is a simplified implementation that detects common patterns.
/// Production implementations should use more sophisticated detection.
pub struct PiiRedactor {
    /// Whether to log detections
    log_detections: bool,
    /// Action to take on detection
    action: PiiAction,
}

/// Action to take when PII is detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiiAction {
    /// Just log the detection
    Log,
    /// Redact the PII
    Redact,
    /// Block the request
    Block,
}

impl PiiRedactor {
    /// Create a new PII redactor
    pub fn new(action: PiiAction) -> Self {
        Self {
            log_detections: true,
            action,
        }
    }

    /// Scan text for PII
    pub fn scan(&self, text: &str) -> Vec<PiiMatch> {
        let mut matches = Vec::new();

        // Scan for SSN patterns (XXX-XX-XXXX)
        matches.extend(self.scan_ssn(text));

        // Scan for credit card patterns
        matches.extend(self.scan_credit_card(text));

        // Scan for email patterns
        matches.extend(self.scan_email(text));

        // Scan for phone patterns
        matches.extend(self.scan_phone(text));

        matches
    }

    /// Check if any PII is present
    pub fn contains_pii(&self, text: &str) -> bool {
        !self.scan(text).is_empty()
    }

    /// Get the configured action
    pub fn action(&self) -> PiiAction {
        self.action
    }

    // Simple SSN detection (XXX-XX-XXXX)
    fn scan_ssn(&self, text: &str) -> Vec<PiiMatch> {
        let mut matches = Vec::new();
        let bytes = text.as_bytes();
        let mut i = 0;

        while i + 10 < bytes.len() {
            // Check for pattern: DDD-DD-DDDD
            if self.is_ssn_pattern(&bytes[i..]) {
                matches.push(PiiMatch {
                    pii_type: PiiType::Ssn,
                    start: i,
                    end: i + 11,
                    value_hint: format!("***-**-{}", &text[i + 7..i + 11]),
                });
                i += 11;
            } else {
                i += 1;
            }
        }

        matches
    }

    fn is_ssn_pattern(&self, bytes: &[u8]) -> bool {
        if bytes.len() < 11 {
            return false;
        }

        // XXX-XX-XXXX
        bytes[0].is_ascii_digit()
            && bytes[1].is_ascii_digit()
            && bytes[2].is_ascii_digit()
            && bytes[3] == b'-'
            && bytes[4].is_ascii_digit()
            && bytes[5].is_ascii_digit()
            && bytes[6] == b'-'
            && bytes[7].is_ascii_digit()
            && bytes[8].is_ascii_digit()
            && bytes[9].is_ascii_digit()
            && bytes[10].is_ascii_digit()
    }

    // Simple credit card detection (16 digits with optional separators)
    fn scan_credit_card(&self, text: &str) -> Vec<PiiMatch> {
        let mut matches = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if let Some((end, card_hint)) = self.is_credit_card_pattern(&chars[i..]) {
                matches.push(PiiMatch {
                    pii_type: PiiType::CreditCard,
                    start: i,
                    end: i + end,
                    value_hint: card_hint,
                });
                i += end;
            } else {
                i += 1;
            }
        }

        matches
    }

    fn is_credit_card_pattern(&self, chars: &[char]) -> Option<(usize, String)> {
        let mut digit_count = 0;
        let mut end = 0;

        for (i, &c) in chars.iter().enumerate() {
            if c.is_ascii_digit() {
                digit_count += 1;
                end = i + 1;
            } else if c == '-' || c == ' ' {
                // Allow separators
                continue;
            } else {
                break;
            }

            if digit_count == 16 {
                return Some((end, "****-****-****-****".to_string()));
            }
        }

        None
    }

    // Simple email detection (contains @ with text before and after)
    fn scan_email(&self, text: &str) -> Vec<PiiMatch> {
        let mut matches = Vec::new();

        for (i, _) in text.match_indices('@') {
            // Find start of email (walk back to whitespace or start)
            let start = text[..i]
                .rfind(|c: char| c.is_whitespace() || c == '<' || c == '"')
                .map(|p| p + 1)
                .unwrap_or(0);

            // Find end of email (walk forward to whitespace or end)
            let after_at = &text[i + 1..];
            let end = after_at
                .find(|c: char| c.is_whitespace() || c == '>' || c == '"')
                .map(|p| i + 1 + p)
                .unwrap_or(text.len());

            // Validate there's text before @ and a dot after
            if i > start && end > i + 1 && text[i + 1..end].contains('.') {
                matches.push(PiiMatch {
                    pii_type: PiiType::Email,
                    start,
                    end,
                    value_hint: "[EMAIL]".to_string(),
                });
            }
        }

        matches
    }

    // Simple phone detection
    fn scan_phone(&self, text: &str) -> Vec<PiiMatch> {
        let mut matches = Vec::new();
        let bytes = text.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            if let Some((end, hint)) = self.is_phone_pattern(&bytes[i..]) {
                matches.push(PiiMatch {
                    pii_type: PiiType::Phone,
                    start: i,
                    end: i + end,
                    value_hint: hint,
                });
                i += end;
            } else {
                i += 1;
            }
        }

        matches
    }

    fn is_phone_pattern(&self, bytes: &[u8]) -> Option<(usize, String)> {
        // Simple pattern: 10+ consecutive digits with optional separators
        let mut digit_count = 0;
        let mut end = 0;

        for (i, &b) in bytes.iter().enumerate() {
            if b.is_ascii_digit() {
                digit_count += 1;
                end = i + 1;
            } else if b == b'-' || b == b' ' || b == b'(' || b == b')' || b == b'.' {
                // Allow common phone separators
                if digit_count > 0 {
                    end = i + 1;
                }
            } else {
                break;
            }
        }

        if digit_count >= 10 {
            Some((end, "***-***-****".to_string()))
        } else {
            None
        }
    }
}

impl Default for PiiRedactor {
    fn default() -> Self {
        Self::new(PiiAction::Log)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssn_detection() {
        let redactor = PiiRedactor::new(PiiAction::Log);
        let text = "My SSN is 123-45-6789";
        let matches = redactor.scan(text);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pii_type, PiiType::Ssn);
    }

    #[test]
    fn test_credit_card_detection() {
        let redactor = PiiRedactor::new(PiiAction::Log);
        let text = "Card: 4111-1111-1111-1111";
        let matches = redactor.scan(text);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pii_type, PiiType::CreditCard);
    }

    #[test]
    fn test_email_detection() {
        let redactor = PiiRedactor::new(PiiAction::Log);
        let text = "Contact me at user@example.com for details";
        let matches = redactor.scan(text);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pii_type, PiiType::Email);
    }

    #[test]
    fn test_phone_detection() {
        let redactor = PiiRedactor::new(PiiAction::Log);
        let text = "Call me at 555-123-4567";
        let matches = redactor.scan(text);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pii_type, PiiType::Phone);
    }

    #[test]
    fn test_no_pii() {
        let redactor = PiiRedactor::new(PiiAction::Log);
        let text = "What is the weather like today?";

        assert!(!redactor.contains_pii(text));
    }

    #[test]
    fn test_multiple_pii() {
        let redactor = PiiRedactor::new(PiiAction::Log);
        let text = "SSN: 123-45-6789, Email: test@example.com";
        let matches = redactor.scan(text);

        assert!(matches.len() >= 2);
    }
}
