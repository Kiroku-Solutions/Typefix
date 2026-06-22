//! Core module - fundamental data structures and utilities

/// Buffer management
pub mod buffer;
/// Configuration handling
pub mod config;
/// Dictionary implementations
pub mod dict;
/// Encoding utilities
pub mod encoder;

pub use buffer::{BufferEvent, CharBuffer, CharBufferBuilder};
pub use config::Config;
pub use dict::Dict;
