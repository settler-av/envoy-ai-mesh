//! Protocol Handlers for AI-Guard
//!
//! This module provides handlers for:
//! - MCP (Model Context Protocol) - HTTP, SSE, WebSocket transports
//! - A2A (Agent-to-Agent) - JSONRPC, gRPC, HTTP+JSON bindings

pub mod mcp;
pub mod a2a;

pub use mcp::{McpHandler, McpTransport, McpRequest, McpResponse, McpValidationError};
pub use a2a::{A2AHandler, A2ABinding, A2AMessage, A2AValidationError};
