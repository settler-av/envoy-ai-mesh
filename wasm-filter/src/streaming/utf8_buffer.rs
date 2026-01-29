//! UTF-8 Boundary Handler
//!
//! CRITICAL: Multi-byte UTF-8 characters can split across chunk boundaries.
//! This module handles the "split emoji" problem by buffering incomplete
//! sequences until the next chunk arrives.
//!
//! A UTF-8 character can be 1-4 bytes:
//! - 1 byte:  0xxxxxxx (ASCII)
//! - 2 bytes: 110xxxxx 10xxxxxx
//! - 3 bytes: 1110xxxx 10xxxxxx 10xxxxxx
//! - 4 bytes: 11110xxx 10xxxxxx 10xxxxxx 10xxxxxx

/// Handles UTF-8 sequences that split across chunk boundaries.
pub struct Utf8Buffer {
    /// Leftover bytes from previous chunk (max 4 bytes for UTF-8)
    leftover: [u8; 4],
    /// Number of leftover bytes
    leftover_len: usize,
}

impl Utf8Buffer {
    /// Create a new UTF-8 boundary handler
    pub fn new() -> Self {
        Self {
            leftover: [0u8; 4],
            leftover_len: 0,
        }
    }

    /// Process a new chunk, handling any leftover bytes from previous chunk.
    /// Returns a view of the processable content.
    pub fn process_chunk<'a>(&mut self, chunk: &'a [u8]) -> ProcessedChunk<'a> {
        // Step 1: If we have leftover bytes, try to complete the UTF-8 sequence
        let prefix = if self.leftover_len > 0 {
            self.complete_sequence(chunk)
        } else {
            None
        };

        // Step 2: Determine how much of the chunk we consumed for the prefix
        let chunk_start = if let Some(ref _p) = prefix {
            // We consumed (expected_len - leftover_len) bytes from chunk
            let expected = Self::sequence_length(self.leftover[0]);
            let consumed = expected.saturating_sub(self.leftover_len);
            consumed.min(chunk.len())
        } else if self.leftover_len > 0 {
            // Failed to complete, chunk might start with continuation bytes
            // Find where valid data starts
            let mut start = 0;
            while start < chunk.len() && Self::is_continuation(chunk[start]) {
                start += 1;
            }
            start
        } else {
            0
        };

        // Reset leftover after attempting completion
        if prefix.is_some() || self.leftover_len > 0 {
            self.leftover_len = 0;
        }

        // Step 3: Find where valid UTF-8 ends in this chunk
        let remaining_chunk = &chunk[chunk_start..];
        let (valid_end, new_leftover) = self.find_valid_boundary(remaining_chunk);

        // Step 4: Store any new leftover bytes
        if let Some((start, len)) = new_leftover {
            let src_start = chunk_start + start;
            self.leftover[..len].copy_from_slice(&chunk[src_start..src_start + len]);
            self.leftover_len = len;
        }

        ProcessedChunk {
            prefix,
            main: &remaining_chunk[..valid_end],
        }
    }

    /// Try to complete a UTF-8 sequence using bytes from the new chunk
    fn complete_sequence(&self, chunk: &[u8]) -> Option<Vec<u8>> {
        if self.leftover_len == 0 || chunk.is_empty() {
            return None;
        }

        let expected_len = Self::sequence_length(self.leftover[0]);
        let needed = expected_len.saturating_sub(self.leftover_len);

        if needed == 0 || chunk.len() < needed {
            return None;
        }

        // Verify the continuation bytes
        for i in 0..needed {
            if !Self::is_continuation(chunk[i]) {
                return None;
            }
        }

        // Build the complete sequence
        let mut complete = Vec::with_capacity(expected_len);
        complete.extend_from_slice(&self.leftover[..self.leftover_len]);
        complete.extend_from_slice(&chunk[..needed]);

        // Validate it's actually valid UTF-8
        if std::str::from_utf8(&complete).is_ok() {
            Some(complete)
        } else {
            None
        }
    }

    /// Check if byte is a UTF-8 continuation byte (10xxxxxx)
    #[inline]
    pub fn is_continuation(byte: u8) -> bool {
        (byte & 0b11000000) == 0b10000000
    }

    /// Get expected length of UTF-8 sequence from first byte
    #[inline]
    pub fn sequence_length(first_byte: u8) -> usize {
        match first_byte {
            0x00..=0x7F => 1, // ASCII
            0xC0..=0xDF => 2, // 2-byte sequence
            0xE0..=0xEF => 3, // 3-byte sequence
            0xF0..=0xF7 => 4, // 4-byte sequence
            _ => 1,           // Invalid, treat as single byte
        }
    }

    /// Find the last valid UTF-8 boundary in the chunk
    fn find_valid_boundary(&self, chunk: &[u8]) -> (usize, Option<(usize, usize)>) {
        if chunk.is_empty() {
            return (0, None);
        }

        // Scan backwards from end to find incomplete sequence
        let mut i = chunk.len();
        while i > 0 && i > chunk.len().saturating_sub(4) {
            i -= 1;
            if !Self::is_continuation(chunk[i]) {
                // Found start of a sequence
                let expected_len = Self::sequence_length(chunk[i]);
                let available = chunk.len() - i;

                if available < expected_len {
                    // Incomplete sequence at end - need to buffer it
                    return (i, Some((i, available)));
                }
                // Complete sequence found, we're done
                break;
            }
        }

        (chunk.len(), None)
    }

    /// Reset the buffer state
    pub fn reset(&mut self) {
        self.leftover_len = 0;
    }
}

impl Default for Utf8Buffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of processing a chunk
pub struct ProcessedChunk<'a> {
    /// Completed sequence from leftover + start of chunk (if any)
    pub prefix: Option<Vec<u8>>,
    /// Main chunk content (valid UTF-8 boundary)
    pub main: &'a [u8],
}

impl<'a> ProcessedChunk<'a> {
    /// Check if there's any content to process
    pub fn is_empty(&self) -> bool {
        self.prefix.is_none() && self.main.is_empty()
    }

    /// Get total bytes available for processing
    pub fn len(&self) -> usize {
        self.prefix.as_ref().map(|p| p.len()).unwrap_or(0) + self.main.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_passthrough() {
        let mut buf = Utf8Buffer::new();
        let chunk = b"Hello, World!";
        let processed = buf.process_chunk(chunk);

        assert!(processed.prefix.is_none());
        assert_eq!(processed.main, chunk);
    }

    #[test]
    fn test_complete_utf8() {
        let mut buf = Utf8Buffer::new();
        // "Hello 🦀" in UTF-8
        let chunk = "Hello 🦀".as_bytes();
        let processed = buf.process_chunk(chunk);

        assert!(processed.prefix.is_none());
        assert_eq!(processed.main, chunk);
    }

    #[test]
    fn test_split_emoji() {
        let mut buf = Utf8Buffer::new();
        
        // 🦀 is F0 9F A6 80 in UTF-8
        let emoji = "🦀".as_bytes();
        assert_eq!(emoji.len(), 4);

        // First chunk: "Hi " + first 2 bytes of emoji
        let chunk1 = &[b'H', b'i', b' ', 0xF0, 0x9F];
        let processed1 = buf.process_chunk(chunk1);
        
        // Should process "Hi " and buffer the incomplete emoji
        assert!(processed1.prefix.is_none());
        assert_eq!(processed1.main, b"Hi ");

        // Second chunk: last 2 bytes of emoji + "!"
        let chunk2 = &[0xA6, 0x80, b'!'];
        let processed2 = buf.process_chunk(chunk2);

        // Should have the complete emoji as prefix
        assert!(processed2.prefix.is_some());
        let prefix = processed2.prefix.unwrap();
        assert_eq!(prefix, emoji);
        assert_eq!(processed2.main, b"!");
    }

    #[test]
    fn test_sequence_length() {
        assert_eq!(Utf8Buffer::sequence_length(b'A'), 1);      // ASCII
        assert_eq!(Utf8Buffer::sequence_length(0xC3), 2);      // 2-byte
        assert_eq!(Utf8Buffer::sequence_length(0xE2), 3);      // 3-byte
        assert_eq!(Utf8Buffer::sequence_length(0xF0), 4);      // 4-byte
    }

    #[test]
    fn test_is_continuation() {
        assert!(!Utf8Buffer::is_continuation(b'A'));           // Not continuation
        assert!(Utf8Buffer::is_continuation(0x80));            // Continuation
        assert!(Utf8Buffer::is_continuation(0xBF));            // Continuation
        assert!(!Utf8Buffer::is_continuation(0xC0));           // Start byte
    }
}
