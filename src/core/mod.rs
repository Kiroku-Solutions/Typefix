//! Core module - fundamental data structures and utilities

pub mod buffer;
pub mod config;
pub mod trie;

pub use buffer::{BufferEvent, CharBuffer, CharBufferBuilder};
pub use config::Config;
pub use trie::Trie;
