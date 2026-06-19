//! Core module - fundamental data structures and utilities

pub mod buffer;
pub mod config;
pub mod dict;

pub use buffer::{BufferEvent, CharBuffer, CharBufferBuilder};
pub use config::Config;
pub use dict::Dict;
