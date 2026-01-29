//! Streaming module for memory-efficient body inspection
//!
//! This module provides streaming primitives that:
//! - Use fixed memory allocation (ring buffer)
//! - Handle UTF-8 boundaries across chunks
//! - Perform pattern matching with FSM (no regex)

pub mod utf8_buffer;
pub mod ring_buffer;
pub mod pattern_fsm;

pub use utf8_buffer::Utf8Buffer;
pub use ring_buffer::RingBuffer;
pub use pattern_fsm::{Pattern, PatternMatch, PatternScanner, PatternState, ScanResult};
