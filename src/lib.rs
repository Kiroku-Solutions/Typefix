//! TypeFix - Core Library
//!
//! Hyper-lightweight, zero-latency typo correction and language detection engine.
//! Designed for high-density text input environments (EHR, legal documentation).

// Unsafe code is allowed only in specific modules (windows hooks)
// that require it for system-level keyboard interception.
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
#[cfg(not(target_arch = "wasm32"))]
pub mod hooks;
pub mod language;
pub mod pipeline;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub use core::{buffer::*, config::*, dict::*};
pub use correction::{damerau::*, engine::*, static_map::*};
#[cfg(target_os = "linux")]
pub use hooks::linux;
#[cfg(target_os = "macos")]
pub use hooks::macos;
#[cfg(not(target_arch = "wasm32"))]
pub use hooks::platform;
#[cfg(target_os = "windows")]
pub use hooks::windows;
pub use language::detector::*;
pub use language::resolver::*;
pub use pipeline::*;

use anyhow::Result;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::sync::Arc;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Global engine state - initialized once, never modified after
#[cfg(not(target_arch = "wasm32"))]
static ENGINE_STATE: Lazy<Arc<RwLock<EngineState>>> =
    Lazy::new(|| Arc::new(RwLock::new(EngineState::default())));

/// Engine state container
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Default)]
pub struct EngineState {
    /// Current active language
    pub active_language: String,
    /// FST dictionaries by language
    pub dictionaries: std::collections::HashMap<String, Arc<Dict>>,
    /// Stopwords by language
    pub stopwords: std::collections::HashMap<String, Arc<StopwordsSet>>,
    /// Static error maps by language
    pub error_maps: std::collections::HashMap<String, Arc<StaticErrorMap>>,
}

/// Initialize the engine with configuration
#[cfg(not(target_arch = "wasm32"))]
pub fn init(config: &core::config::Config) -> Result<()> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.logging.level));

    let _ = tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .try_init();

    tracing::info!("Initializing TypeFix v{}", env!("CARGO_PKG_VERSION"));

    // Load dictionaries for all supported languages first so the active
    // language is guaranteed to have its data available regardless of which
    // tier of the resolution chain selected it.
    for lang in &config.supported_languages {
        load_language_data(lang, &config.data_path)?;
    }

    // Resolve the active language using the priority chain:
    // user preference → system locale → default fallback.
    let resolved = language::resolver::resolve_language(config);
    tracing::info!(
        "Active language: '{}' (source: {:?})",
        resolved.code,
        resolved.source
    );

    let mut state = ENGINE_STATE.write();
    let active = resolved.code.clone();
    state.active_language = active.clone();

    if !state.dictionaries.contains_key(&active) {
        tracing::warn!(
            "Active language '{}' has no dictionary loaded (data/dictionaries/{}.json missing)",
            active,
            active
        );
    }

    tracing::info!(
        "Engine initialized with {} languages",
        config.supported_languages.len()
    );
    Ok(())
}

/// Load language-specific data (dictionary, stopwords, error map)
#[cfg(not(target_arch = "wasm32"))]
fn load_language_data(lang: &str, data_path: &std::path::Path) -> Result<()> {
    let mut state = ENGINE_STATE.write();

    // Load dictionary
    let fst_path = data_path.join("dictionaries").join(format!("{}.fst", lang));
    let json_path = data_path.join("dictionaries").join(format!("{}.json", lang));
    
    if !fst_path.exists() && json_path.exists() {
        tracing::info!("Compiling JSON dictionary to FST for language: {}", lang);
        if let Err(e) = Dict::compile_json_to_fst(&json_path, &fst_path) {
            tracing::error!("Failed to compile dictionary to FST: {}", e);
        }
    }

    if fst_path.exists() {
        let dict = Dict::from_fst_file(&fst_path)?;
        state.dictionaries.insert(lang.to_string(), Arc::new(dict));
        tracing::debug!("Loaded FST dictionary for {}", lang);
    }

    // Load stopwords
    let stopwords_path = data_path.join("stopwords").join(format!("{}.json", lang));
    if stopwords_path.exists() {
        let stopwords = StopwordsSet::from_json_file(&stopwords_path)?;
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
#[cfg(not(target_arch = "wasm32"))]
pub fn get_state() -> Arc<RwLock<EngineState>> {
    Arc::clone(&ENGINE_STATE)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(
    clippy::unwrap_used,
    reason = "test code uses unwrap for concise assertions"
)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::path::PathBuf;

    fn project_data_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    fn config_with(languages: Vec<String>, default: &str) -> core::config::Config {
        core::config::Config {
            data_path: project_data_path(),
            supported_languages: languages,
            default_language: default.to_string(),
            ..core::config::Config::default()
        }
    }

    #[test]
    #[serial]
    fn init_loads_all_supported_dictionaries() {
        let config = config_with(vec!["en".to_string(), "es".to_string()], "en");
        init(&config).expect("init should succeed");

        let state_arc = get_state();
        let state = state_arc.read();
        assert!(state.dictionaries.contains_key("en"));
        assert!(state.dictionaries.contains_key("es"));
        assert!(!state.active_language.is_empty());
    }

    #[test]
    #[serial]
    fn init_with_user_preferred_language_sets_active_language() {
        let config = core::config::Config {
            user_preferred_language: Some("es".to_string()),
            ..config_with(vec!["en".to_string(), "es".to_string()], "en")
        };
        init(&config).expect("init should succeed");

        let state_arc = get_state();
        let state = state_arc.read();
        assert_eq!(state.active_language, "es");
    }

    #[test]
    #[serial]
    fn init_falls_back_to_default_when_user_pref_not_supported() {
        let config = core::config::Config {
            user_preferred_language: Some("zz".to_string()),
            ..config_with(vec!["en".to_string()], "en")
        };
        init(&config).expect("init should succeed");

        let state_arc = get_state();
        let state = state_arc.read();
        assert_eq!(state.active_language, "en");
    }

    #[test]
    #[serial]
    fn init_twice_preserves_active_language_and_dictionaries() {
        let config = config_with(vec!["en".to_string()], "en");

        init(&config).expect("first init should succeed");
        let first_active;
        let first_dict_count;
        {
            let state_arc = get_state();
            let state = state_arc.read();
            first_active = state.active_language.clone();
            first_dict_count = state.dictionaries.len();
        }

        init(&config).expect("second init should succeed");
        let second_active;
        let second_dict_count;
        {
            let state_arc = get_state();
            let state = state_arc.read();
            second_active = state.active_language.clone();
            second_dict_count = state.dictionaries.len();
        }

        assert_eq!(first_active, second_active);
        assert_eq!(first_dict_count, second_dict_count);
    }

    #[test]
    #[serial]
    fn init_succeeds_even_when_active_language_has_no_dictionary() {
        let config = core::config::Config {
            user_preferred_language: Some("zz".to_string()),
            ..config_with(vec!["zz".to_string()], "zz")
        };
        let result = init(&config);
        assert!(result.is_ok());

        let state_arc = get_state();
        let state = state_arc.read();
        assert!(!state.dictionaries.contains_key("zz"));
        assert_eq!(state.active_language, "zz");
    }

    #[test]
    fn resolve_with_user_pref_returns_user_source() {
        let config = core::config::Config {
            user_preferred_language: Some("es".to_string()),
            supported_languages: vec!["en".to_string(), "es".to_string()],
            default_language: "en".to_string(),
            ..config_with(vec!["en".to_string(), "es".to_string()], "en")
        };
        let resolved = language::resolver::resolve_language_with(&config, Some("en-US"));
        assert_eq!(resolved.code, "es");
        assert_eq!(
            resolved.source,
            language::resolver::LanguageSource::UserPreference
        );
    }

    #[test]
    fn resolve_with_locale_match_returns_locale_source() {
        let config = core::config::Config {
            supported_languages: vec!["en".to_string(), "es".to_string()],
            default_language: "en".to_string(),
            ..config_with(vec!["en".to_string(), "es".to_string()], "en")
        };
        let resolved = language::resolver::resolve_language_with(&config, Some("es-ES"));
        assert_eq!(resolved.code, "es");
        assert_eq!(
            resolved.source,
            language::resolver::LanguageSource::SystemLocale
        );
    }

    #[test]
    fn resolve_with_no_match_returns_default_source() {
        let config = core::config::Config {
            supported_languages: vec!["en".to_string()],
            default_language: "en".to_string(),
            ..config_with(vec!["en".to_string()], "en")
        };
        let resolved = language::resolver::resolve_language_with(&config, Some("de-DE"));
        assert_eq!(resolved.code, "en");
        assert_eq!(
            resolved.source,
            language::resolver::LanguageSource::DefaultFallback
        );
    }

    #[test]
    #[serial]
    fn load_language_data_creates_dictionary_entry() {
        load_language_data("en", &project_data_path()).expect("load should succeed");

        let state_arc = get_state();
        let state = state_arc.read();
        assert!(state.dictionaries.contains_key("en"));
    }

    #[test]
    #[serial]
    fn load_language_data_succeeds_for_missing_language() {
        let result = load_language_data("nonexistent_lang", &project_data_path());
        assert!(result.is_ok());

        let state_arc = get_state();
        let state = state_arc.read();
        assert!(!state.dictionaries.contains_key("nonexistent_lang"));
    }

    #[test]
    fn get_state_returns_stable_handle() {
        let a = get_state();
        let b = get_state();
        assert!(Arc::ptr_eq(&a, &b));
    }
}
