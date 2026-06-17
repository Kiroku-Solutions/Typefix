//! TypeFix - Core Library
//!
//! Hyper-lightweight, zero-latency typo correction and language detection engine.
//! Designed for high-density text input environments (EHR, legal documentation).

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented
)]
#![warn(
    clippy::cargo,
    clippy::perf,
    clippy::style,
    missing_docs,
    missing_debug_implementations
)]
#![allow(
    clippy::multiple_crate_versions,
    reason = "transitive dependency from windows crate (1.3.2) and bitflags (2.13.0); not actionable at our level"
)]

pub mod core;
pub mod correction;
pub mod hooks;
pub mod language;
pub mod pipeline;

pub use core::{buffer::*, config::*, trie::*};
pub use correction::{damerau::*, engine::*, static_map::*};
#[cfg(target_os = "linux")]
pub use hooks::linux;
#[cfg(target_os = "macos")]
pub use hooks::macos;
pub use hooks::platform;
#[cfg(target_os = "windows")]
pub use hooks::windows;
pub use language::detector::*;
pub use pipeline::*;

use anyhow::Result;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::sync::Arc;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Global engine state - initialized once, never modified after
static ENGINE_STATE: Lazy<Arc<RwLock<EngineState>>> =
    Lazy::new(|| Arc::new(RwLock::new(EngineState::default())));

/// Engine state container
#[derive(Debug, Default)]
pub struct EngineState {
    /// Current active language
    pub active_language: String,
    /// Trie dictionaries by language
    pub dictionaries: std::collections::HashMap<String, Arc<Trie>>,
    /// Stopwords by language
    pub stopwords: std::collections::HashMap<String, Arc<StopwordsTrie>>,
    /// Static error maps by language
    pub error_maps: std::collections::HashMap<String, Arc<StaticErrorMap>>,
}

/// Initialize the engine with configuration
pub fn init(config: &core::config::Config) -> Result<()> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.logging.level));

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    tracing::info!("Initializing TypeFix v{}", env!("CARGO_PKG_VERSION"));

    let mut state = ENGINE_STATE.write();
    state.active_language = config.default_language.clone();

    // Load dictionaries
    for lang in &config.supported_languages {
        load_language_data(lang, &config.data_path)?;
    }

    tracing::info!(
        "Engine initialized with {} languages",
        config.supported_languages.len()
    );
    Ok(())
}

/// Load language-specific data (dictionary, stopwords, error map)
fn load_language_data(lang: &str, data_path: &std::path::Path) -> Result<()> {
    let mut state = ENGINE_STATE.write();

    // Load dictionary
    let dict_path = data_path
        .join("dictionaries")
        .join(format!("{}.json", lang));
    if dict_path.exists() {
        let trie = Trie::from_json_file(&dict_path)?;
        state.dictionaries.insert(lang.to_string(), Arc::new(trie));
        tracing::debug!("Loaded dictionary for {}", lang);
    }

    // Load stopwords
    let stopwords_path = data_path.join("stopwords").join(format!("{}.json", lang));
    if stopwords_path.exists() {
        let stopwords = StopwordsTrie::from_json_file(&stopwords_path)?;
        state
            .stopwords
            .insert(lang.to_string(), Arc::new(stopwords));
        tracing::debug!("Loaded stopwords for {}", lang);
    }

    // Load error map
    let errors_path = data_path.join("errors").join(format!("{}.json", lang));
    if errors_path.exists() {
        let error_map = StaticErrorMap::from_json_file(&errors_path)?;
        state
            .error_maps
            .insert(lang.to_string(), Arc::new(error_map));
        tracing::debug!("Loaded error map for {}", lang);
    }

    Ok(())
}

/// Get current engine state
pub fn get_state() -> Arc<RwLock<EngineState>> {
    Arc::clone(&ENGINE_STATE)
}
