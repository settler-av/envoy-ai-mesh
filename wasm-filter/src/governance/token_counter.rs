//! Token Counter Module
//!
//! Extracts token usage from AI API responses for cost attribution.
//! Supports common AI provider formats (OpenAI, Anthropic, etc.)

use serde::Deserialize;
use std::collections::HashMap;

/// Token usage information
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    /// Number of tokens in the prompt
    pub prompt_tokens: u32,
    /// Number of tokens in the completion
    pub completion_tokens: u32,
    /// Total tokens used
    pub total_tokens: u32,
    /// Estimated cost in USD (if known)
    pub estimated_cost_usd: Option<f64>,
    /// Model used (if extracted)
    pub model: Option<String>,
}

impl TokenUsage {
    /// Create empty token usage
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate total if not set
    pub fn calculate_total(&mut self) {
        if self.total_tokens == 0 {
            self.total_tokens = self.prompt_tokens + self.completion_tokens;
        }
    }
}

/// Token counter for extracting usage from responses
pub struct TokenCounter {
    /// Model pricing (tokens per dollar)
    pricing: HashMap<String, TokenPricing>,
}

/// Pricing for a specific model
#[derive(Clone)]
struct TokenPricing {
    input_per_1k: f64,
    output_per_1k: f64,
}

impl TokenCounter {
    /// Create a new token counter with default pricing
    pub fn new() -> Self {
        let mut pricing = HashMap::new();

        // OpenAI pricing (approximate, as of 2024)
        pricing.insert(
            "gpt-4".to_string(),
            TokenPricing {
                input_per_1k: 0.03,
                output_per_1k: 0.06,
            },
        );
        pricing.insert(
            "gpt-4-turbo".to_string(),
            TokenPricing {
                input_per_1k: 0.01,
                output_per_1k: 0.03,
            },
        );
        pricing.insert(
            "gpt-3.5-turbo".to_string(),
            TokenPricing {
                input_per_1k: 0.0005,
                output_per_1k: 0.0015,
            },
        );

        // Anthropic pricing (approximate)
        pricing.insert(
            "claude-3-opus".to_string(),
            TokenPricing {
                input_per_1k: 0.015,
                output_per_1k: 0.075,
            },
        );
        pricing.insert(
            "claude-3-sonnet".to_string(),
            TokenPricing {
                input_per_1k: 0.003,
                output_per_1k: 0.015,
            },
        );

        Self { pricing }
    }

    /// Extract token usage from response headers
    pub fn extract_from_headers(&self, headers: &[(String, String)]) -> Option<TokenUsage> {
        let mut usage = TokenUsage::new();
        let mut found = false;

        for (name, value) in headers {
            let name_lower = name.to_lowercase();

            // OpenAI-style headers
            if name_lower == "x-ratelimit-remaining-tokens" {
                // Not directly usage, but indicates token tracking
            }

            // Check for usage headers (some proxies add these)
            if name_lower.contains("x-usage-prompt-tokens") {
                if let Ok(v) = value.parse() {
                    usage.prompt_tokens = v;
                    found = true;
                }
            }
            if name_lower.contains("x-usage-completion-tokens") {
                if let Ok(v) = value.parse() {
                    usage.completion_tokens = v;
                    found = true;
                }
            }
            if name_lower.contains("x-usage-total-tokens") {
                if let Ok(v) = value.parse() {
                    usage.total_tokens = v;
                    found = true;
                }
            }
        }

        if found {
            usage.calculate_total();
            Some(usage)
        } else {
            None
        }
    }

    /// Extract token usage from response body (JSON)
    pub fn extract_from_body(&self, body: &[u8]) -> Option<TokenUsage> {
        // Try to parse as JSON
        let text = std::str::from_utf8(body).ok()?;
        
        // Try OpenAI format
        if let Some(usage) = self.extract_openai_format(text) {
            return Some(usage);
        }

        // Try Anthropic format
        if let Some(usage) = self.extract_anthropic_format(text) {
            return Some(usage);
        }

        None
    }

    /// Extract from OpenAI format: {"usage": {"prompt_tokens": N, ...}}
    fn extract_openai_format(&self, text: &str) -> Option<TokenUsage> {
        #[derive(Deserialize)]
        struct OpenAIResponse {
            usage: Option<OpenAIUsage>,
            model: Option<String>,
        }

        #[derive(Deserialize)]
        struct OpenAIUsage {
            prompt_tokens: Option<u32>,
            completion_tokens: Option<u32>,
            total_tokens: Option<u32>,
        }

        let response: OpenAIResponse = serde_json::from_str(text).ok()?;
        let api_usage = response.usage?;

        let mut usage = TokenUsage {
            prompt_tokens: api_usage.prompt_tokens.unwrap_or(0),
            completion_tokens: api_usage.completion_tokens.unwrap_or(0),
            total_tokens: api_usage.total_tokens.unwrap_or(0),
            model: response.model.clone(),
            estimated_cost_usd: None,
        };

        usage.calculate_total();

        // Calculate cost if model is known
        if let Some(model) = &response.model {
            usage.estimated_cost_usd = self.calculate_cost(model, &usage);
        }

        Some(usage)
    }

    /// Extract from Anthropic format: {"usage": {"input_tokens": N, ...}}
    fn extract_anthropic_format(&self, text: &str) -> Option<TokenUsage> {
        #[derive(Deserialize)]
        struct AnthropicResponse {
            usage: Option<AnthropicUsage>,
            model: Option<String>,
        }

        #[derive(Deserialize)]
        struct AnthropicUsage {
            input_tokens: Option<u32>,
            output_tokens: Option<u32>,
        }

        let response: AnthropicResponse = serde_json::from_str(text).ok()?;
        let api_usage = response.usage?;

        let mut usage = TokenUsage {
            prompt_tokens: api_usage.input_tokens.unwrap_or(0),
            completion_tokens: api_usage.output_tokens.unwrap_or(0),
            total_tokens: 0,
            model: response.model.clone(),
            estimated_cost_usd: None,
        };

        usage.calculate_total();

        // Calculate cost if model is known
        if let Some(model) = &response.model {
            usage.estimated_cost_usd = self.calculate_cost(model, &usage);
        }

        Some(usage)
    }

    /// Calculate cost for a given model and usage
    pub fn calculate_cost(&self, model: &str, usage: &TokenUsage) -> Option<f64> {
        // Find pricing for model (partial match)
        let pricing = self.pricing.iter().find(|(k, _)| model.contains(k.as_str()));

        if let Some((_, pricing)) = pricing {
            let input_cost = (usage.prompt_tokens as f64 / 1000.0) * pricing.input_per_1k;
            let output_cost = (usage.completion_tokens as f64 / 1000.0) * pricing.output_per_1k;
            Some(input_cost + output_cost)
        } else {
            None
        }
    }
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_openai_format() {
        let counter = TokenCounter::new();
        let body = r#"{"id":"123","usage":{"prompt_tokens":10,"completion_tokens":20,"total_tokens":30},"model":"gpt-4"}"#;

        let usage = counter.extract_from_body(body.as_bytes()).unwrap();

        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
        assert!(usage.estimated_cost_usd.is_some());
    }

    #[test]
    fn test_extract_anthropic_format() {
        let counter = TokenCounter::new();
        let body = r#"{"content":"Hello","usage":{"input_tokens":15,"output_tokens":25},"model":"claude-3-sonnet"}"#;

        let usage = counter.extract_from_body(body.as_bytes()).unwrap();

        assert_eq!(usage.prompt_tokens, 15);
        assert_eq!(usage.completion_tokens, 25);
        assert_eq!(usage.total_tokens, 40);
    }

    #[test]
    fn test_calculate_cost() {
        let counter = TokenCounter::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 1000,
            total_tokens: 2000,
            model: Some("gpt-4".to_string()),
            estimated_cost_usd: None,
        };

        let cost = counter.calculate_cost("gpt-4", &usage);
        assert!(cost.is_some());
        // GPT-4: $0.03/1K input + $0.06/1K output = $0.09
        assert!((cost.unwrap() - 0.09).abs() < 0.001);
    }

    #[test]
    fn test_no_usage() {
        let counter = TokenCounter::new();
        let body = r#"{"error":"invalid request"}"#;

        let usage = counter.extract_from_body(body.as_bytes());
        assert!(usage.is_none());
    }
}
