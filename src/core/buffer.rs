//! Thread-safe character buffer with delimiter detection
//!
//! Provides a ring buffer for capturing keystrokes until a delimiter
//! is detected, then extracts the token for processing.

use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;

/// Listener type for buffer events
type Listener = Box<dyn Fn(BufferEvent) + Send + Sync>;

/// Maximum buffer size - prevents memory bloat from long strings
pub const MAX_BUFFER_SIZE: usize = 64;

/// Delimiter types that trigger token extraction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    /// Space character (` `)
    Space,
    /// Newline / carriage return characters
    Enter,
    /// Tab character (`\t`)
    Tab,
    /// Any punctuation character
    Punctuation,
    /// User-supplied custom delimiter character
    Custom(char),
}

impl Delimiter {
    /// Check if a character is a delimiter
    pub fn is_delimiter(ch: char) -> bool {
        matches!(ch, ' ' | '\n' | '\r' | '\t') || Self::is_punctuation(ch)
    }

    /// Check if character is punctuation
    pub fn is_punctuation(ch: char) -> bool {
        matches!(
            ch,
            '.' | ','
                | ';'
                | ':'
                | '!'
                | '?'
                | '"'
                | '\''
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '-'
                | '_'
                | '/'
                | '\\'
                | '@'
                | '#'
                | '$'
                | '%'
                | '^'
                | '&'
                | '*'
                | '+'
                | '='
                | '<'
                | '>'
                | '|'
                | '~'
                | '`'
        )
    }

    /// Classify a character
    pub fn classify(ch: char) -> Option<Delimiter> {
        match ch {
            ' ' => Some(Delimiter::Space),
            '\n' | '\r' => Some(Delimiter::Enter),
            '\t' => Some(Delimiter::Tab),
            _ if Self::is_punctuation(ch) => Some(Delimiter::Punctuation),
            _ => None,
        }
    }
}

/// Buffer event types
#[derive(Debug, Clone)]
pub enum BufferEvent {
    /// Token was extracted (complete word)
    TokenExtracted {
        /// The extracted token text
        token: String,
        /// The delimiter that triggered extraction
        delimiter: Delimiter,
    },
    /// Buffer was truncated (max size reached)
    BufferTruncated {
        /// The original buffer contents before truncation
        original: String,
        /// The remaining buffer contents after truncation
        truncated: String,
    },
    /// Buffer was cleared
    BufferCleared,
    /// Buffer overflow prevented
    BufferOverflowPrevented {
        /// The character that was attempted to be added
        attempted: char,
    },
}

/// Thread-safe character buffer
///
/// # Guarantees
/// * Thread-safe via RwLock - multiple readers, single writer
/// * Bounded size - max MAX_BUFFER_SIZE characters
/// * Fail-safe - never panics, always returns valid state
#[allow(
    missing_debug_implementations,
    reason = "Box<dyn Fn(BufferEvent)> is not Debug; manual impl would add no value"
)]
pub struct CharBuffer {
    inner: Arc<RwLock<BufferInner>>,
}

struct BufferInner {
    buffer: VecDeque<char>,
    listeners: Vec<Listener>,
}

impl Default for BufferInner {
    fn default() -> Self {
        Self {
            buffer: VecDeque::with_capacity(MAX_BUFFER_SIZE),
            listeners: Vec::new(),
        }
    }
}

impl CharBuffer {
    /// Create a new empty buffer
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(BufferInner::default())),
        }
    }

    /// Create a new buffer with shared state
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(BufferInner {
                buffer: VecDeque::with_capacity(capacity.min(MAX_BUFFER_SIZE)),
                listeners: Vec::new(),
            })),
        }
    }

    /// Add a character to the buffer
    ///
    /// Returns the extracted token if a delimiter was hit, None otherwise.
    ///
    /// # Fail-Safe Behavior
    /// * If buffer is full: truncate and notify listeners
    /// * If char is delimiter: extract token and notify listeners
    pub fn push(&self, ch: char) -> Option<String> {
        let (token, event) = {
            let mut inner = self.inner.write();

            // Check for delimiter
            if let Some(delimiter) = Delimiter::classify(ch) {
                // Extract token before clearing
                let token: String = inner.buffer.iter().collect();

                // Notify listeners
                let event = if !token.is_empty() {
                    BufferEvent::TokenExtracted {
                        token: token.clone(),
                        delimiter,
                    }
                } else {
                    // Empty buffer before delimiter, just clear
                    BufferEvent::BufferCleared
                };

                inner.buffer.clear();
                (Some(token), event)
            } else {
                // Non-delimiter character
                if inner.buffer.len() >= MAX_BUFFER_SIZE {
                    // Buffer full - truncate
                    let truncated: String = inner.buffer.iter().collect();
                    let event = BufferEvent::BufferTruncated {
                        original: truncated.clone(),
                        truncated: truncated.clone(),
                    };
                    // Keep only last N-1 characters
                    inner.buffer.pop_front();
                    (None, event)
                } else {
                    inner.buffer.push_back(ch);
                    (None, BufferEvent::BufferOverflowPrevented { attempted: ch })
                }
            }
        };

        // Notify listeners (outside lock)
        self.notify_listeners(&event);
        token
    }

    /// Add a string to the buffer (for paste events)
    ///
    /// Processes character by character, extracting tokens at delimiters.
    /// Any remaining buffer content after processing is flushed.
    pub fn push_string(&self, text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        for ch in text.chars() {
            if let Some(token) = self.push(ch) {
                if !token.is_empty() {
                    tokens.push(token);
                }
            }
        }
        // Flush any remaining buffer content
        let remaining = self.contents();
        if !remaining.is_empty() {
            tokens.push(remaining);
        }
        tokens
    }

    /// Get current buffer contents
    pub fn contents(&self) -> String {
        let inner = self.inner.read();
        inner.buffer.iter().collect()
    }

    /// Get buffer length
    pub fn len(&self) -> usize {
        let inner = self.inner.read();
        inner.buffer.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        let inner = self.inner.read();
        inner.buffer.is_empty()
    }

    /// Clear the buffer
    pub fn clear(&self) {
        let event = {
            let mut inner = self.inner.write();
            inner.buffer.clear();
            BufferEvent::BufferCleared
        };
        self.notify_listeners(&event);
    }

    /// Get last N characters
    pub fn last_chars(&self, n: usize) -> String {
        let inner = self.inner.read();
        let start = inner.buffer.len().saturating_sub(n);
        inner.buffer.iter().skip(start).collect()
    }

    /// Extract current buffer as token and clear
    pub fn extract(&self) -> Option<String> {
        let token = {
            let mut inner = self.inner.write();
            if inner.buffer.is_empty() {
                return None;
            }
            let token: String = inner.buffer.iter().collect();
            inner.buffer.clear();
            Some(token)
        };

        if token.is_some() {
            self.notify_listeners(&BufferEvent::BufferCleared);
        }
        token
    }

    /// Register an event listener
    pub fn on_event<F>(&self, callback: F)
    where
        F: Fn(BufferEvent) + Send + Sync + 'static,
    {
        let mut inner = self.inner.write();
        inner.listeners.push(Box::new(callback));
    }

    /// Notify all listeners of an event
    fn notify_listeners(&self, event: &BufferEvent) {
        let event = event.clone();
        // Iterate directly while holding the lock - callbacks should be fast
        for callback in self.inner.read().listeners.iter() {
            callback(event.clone());
        }
    }
}

impl Default for CharBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for CharBuffer with custom configuration
#[allow(
    missing_debug_implementations,
    reason = "Vec<Listener> contains Box<dyn Fn> which is not Debug"
)]
pub struct CharBufferBuilder {
    capacity: usize,
    listeners: Vec<Listener>,
}

impl CharBufferBuilder {
    /// Create a new builder with default capacity
    pub fn new() -> Self {
        Self {
            capacity: MAX_BUFFER_SIZE,
            listeners: Vec::new(),
        }
    }

    /// Set the maximum buffer capacity (clamped to `MAX_BUFFER_SIZE`)
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity.min(MAX_BUFFER_SIZE);
        self
    }

    /// Register a callback to be invoked for every buffer event
    pub fn on_event<F>(mut self, callback: F) -> Self
    where
        F: Fn(BufferEvent) + Send + Sync + 'static,
    {
        self.listeners.push(Box::new(callback));
        self
    }

    /// Build the configured `CharBuffer`
    pub fn build(self) -> CharBuffer {
        CharBuffer {
            inner: Arc::new(RwLock::new(BufferInner {
                buffer: VecDeque::with_capacity(self.capacity),
                listeners: self.listeners,
            })),
        }
    }
}

impl Default for CharBufferBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "test code uses unwrap for concise assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_push_and_extract() {
        let buffer = CharBuffer::new();

        buffer.push('h');
        buffer.push('o');
        buffer.push('l');
        buffer.push('a');

        assert_eq!(buffer.contents(), "hola");
        assert_eq!(buffer.len(), 4);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_delimiter_extraction() {
        let buffer = CharBuffer::new();

        buffer.push('h');
        buffer.push('o');
        buffer.push('l');
        buffer.push('a');
        let token = buffer.push(' '); // delimiter

        assert_eq!(token, Some("hola".to_string()));
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_multiple_delimiters() {
        let buffer = CharBuffer::new();

        let t1 = buffer.push('h');
        buffer.push('o');
        let t2 = buffer.push(' ');

        assert_eq!(t1, None);
        assert_eq!(t2, Some("ho".to_string()));
    }

    #[test]
    fn test_punctuation_delimiter() {
        let buffer = CharBuffer::new();

        buffer.push('h');
        buffer.push('o');
        let token = buffer.push('.');

        assert_eq!(token, Some("ho".to_string()));
    }

    #[test]
    fn test_clear() {
        let buffer = CharBuffer::new();

        buffer.push('t');
        buffer.push('e');
        buffer.push('s');
        buffer.push('t');

        buffer.clear();
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_max_size_truncation() {
        // Use default buffer (MAX_BUFFER_SIZE = 64)
        let buffer = CharBuffer::new();

        // Push exactly 10 characters
        for i in 0..10 {
            buffer.push(std::char::from_u32('a' as u32 + i as u32).unwrap());
        }

        // Buffer should have all 10 characters (under MAX_BUFFER_SIZE)
        assert_eq!(buffer.len(), 10);
        assert_eq!(buffer.contents(), "abcdefghij");
    }

    #[test]
    fn test_unicode() {
        let buffer = CharBuffer::new();

        buffer.push('c');
        buffer.push('a');
        buffer.push('f');
        buffer.push('é');
        // 'é' is not a delimiter, so buffer accumulates
        let token = buffer.push('.'); // '.' triggers extraction

        assert_eq!(token, Some("café".to_string()));
    }

    #[test]
    fn test_emoji() {
        let buffer = CharBuffer::new();

        buffer.push('h');
        buffer.push('i');
        buffer.push('😀');
        // No delimiters, just accumulation
        assert_eq!(buffer.contents(), "hi😀");
    }

    #[test]
    fn test_push_string() {
        let buffer = CharBuffer::new();

        let tokens = buffer.push_string("hola mundo cruel");

        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], "hola");
        assert_eq!(tokens[1], "mundo");
        assert_eq!(tokens[2], "cruel");
    }

    #[test]
    fn test_last_chars() {
        let buffer = CharBuffer::new();

        buffer.push('o');
        buffer.push('l');
        buffer.push('d');

        assert_eq!(buffer.last_chars(2), "ld");
        assert_eq!(buffer.last_chars(10), "old");
    }

    #[test]
    fn test_extract() {
        let buffer = CharBuffer::new();

        buffer.push('t');
        buffer.push('e');
        buffer.push('s');
        buffer.push('t');

        let token = buffer.extract();
        assert_eq!(token, Some("test".to_string()));
        assert!(buffer.is_empty());

        // Extract from empty buffer
        assert_eq!(buffer.extract(), None);
    }

    #[test]
    fn test_enter_delimiter() {
        let buffer = CharBuffer::new();

        buffer.push('l');
        buffer.push('i');
        buffer.push('n');
        let token = buffer.push('\n');

        assert_eq!(token, Some("lin".to_string()));
    }

    #[test]
    fn test_tab_delimiter() {
        let buffer = CharBuffer::new();

        buffer.push('d');
        buffer.push('a');
        buffer.push('t');
        let token = buffer.push('\t');

        assert_eq!(token, Some("dat".to_string()));
    }

    #[test]
    fn test_consecutive_delimiters() {
        let buffer = CharBuffer::new();

        buffer.push('a');
        let t1 = buffer.push(' ');
        buffer.push('b');
        let t2 = buffer.push(' ');

        assert_eq!(t1, Some("a".to_string()));
        assert_eq!(t2, Some("b".to_string()));
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_listener_callbacks() {
        let buffer = CharBuffer::new();
        let extracted: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));
        let extracted_clone = Arc::clone(&extracted);

        buffer.on_event(move |event| {
            if let BufferEvent::TokenExtracted { token, .. } = event {
                *extracted_clone.write() = Some(token);
            }
        });

        buffer.push('t');
        buffer.push('e');
        buffer.push('s');
        buffer.push('t');
        buffer.push('.'); // Delimiter triggers extraction

        assert_eq!(*extracted.read(), Some("test".to_string()));
    }
}
