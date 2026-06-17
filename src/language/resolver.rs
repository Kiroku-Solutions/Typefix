//! Language resolution module
//!
//! Determines which language the engine should use based on a three-tier priority chain:
//! 1. User's explicit preference (`Config::user_preferred_language`)
//! 2. System locale detection (via `sys-locale`)
//! 3. Default fallback (`Config::default_language`)

use crate::core::config::Config;

/// How the resolved language was determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageSource {
    /// Language was explicitly set by the user in configuration.
    UserPreference,
    /// Language was detected from the operating system locale.
    SystemLocale,
    /// Fell back to the configured default language.
    DefaultFallback,
}

/// Result of language resolution containing both the code and its provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLanguage {
    /// Resolved ISO 639-1 language code.
    pub code: String,
    /// Source tier that produced the resolution.
    pub source: LanguageSource,
}

/// Map a BCP 47 locale tag (e.g., `"en-US"`, `"es-ES"`, `"pt-BR"`) to its
/// ISO 639-1 base language code (e.g., `"en"`, `"es"`, `"pt"`).
///
/// The mapping is lossy: regional variants collapse to their base language.
/// Empty or unknown language parts fall back to `"en"`.
pub fn bcp47_to_iso639(bcp47: &str) -> String {
    let first = bcp47
        .split(['-', '_'])
        .next()
        .expect("split always yields at least one element");
    if first.is_empty() {
        "en".to_string()
    } else {
        first.to_lowercase()
    }
}

/// Map a raw locale string to a supported ISO 639-1 code, or `None` if the
/// input is empty or does not match any entry in `supported`.
///
/// This function is the testable core of system-locale detection: callers that
/// need deterministic results can pass a synthetic locale string instead of
/// querying the operating system.
#[must_use]
pub fn detect_system_locale_with(raw_locale: Option<&str>, supported: &[String]) -> Option<String> {
    let raw = raw_locale?;
    let iso_code = bcp47_to_iso639(raw);
    if iso_code.is_empty() {
        return None;
    }
    supported
        .iter()
        .find(|lang| lang.as_str() == iso_code)
        .cloned()
}

/// Detect the current system locale and map it to a supported ISO 639-1 code.
///
/// Returns `None` if the system locale cannot be determined or cannot be mapped
/// to any language in `supported`.
#[must_use]
pub fn detect_system_locale(supported: &[String]) -> Option<String> {
    detect_system_locale_with(sys_locale::get_locale().as_deref(), supported)
}

/// Resolve the active language using the priority chain.
///
/// `raw_locale_override` lets callers (especially tests) inject a synthetic
/// locale string; pass `None` in production to query the operating system.
#[must_use]
pub fn resolve_language_with(
    config: &Config,
    raw_locale_override: Option<&str>,
) -> ResolvedLanguage {
    if let Some(ref preferred) = config.user_preferred_language {
        if config.supported_languages.contains(preferred) {
            tracing::debug!("Language resolved from user preference: {}", preferred);
            return ResolvedLanguage {
                code: preferred.clone(),
                source: LanguageSource::UserPreference,
            };
        }
        tracing::warn!(
            "user_preferred_language '{}' is not in supported languages, ignoring",
            preferred
        );
    }

    let detected = match raw_locale_override {
        Some(raw) => detect_system_locale_with(Some(raw), &config.supported_languages),
        None => detect_system_locale(&config.supported_languages),
    };
    if let Some(code) = detected {
        tracing::debug!("Language resolved from system locale: {}", code);
        return ResolvedLanguage {
            code,
            source: LanguageSource::SystemLocale,
        };
    }

    tracing::debug!(
        "Language resolved from default fallback: {}",
        config.default_language
    );
    ResolvedLanguage {
        code: config.default_language.clone(),
        source: LanguageSource::DefaultFallback,
    }
}

/// Resolve the active language using the priority chain, querying the OS for
/// the current locale.
///
/// Order: user preference → system locale → default fallback.
#[must_use]
pub fn resolve_language(config: &Config) -> ResolvedLanguage {
    resolve_language_with(config, None)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "test code uses unwrap for concise assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_bcp47_simple_language() {
        assert_eq!(bcp47_to_iso639("en"), "en");
        assert_eq!(bcp47_to_iso639("es"), "es");
        assert_eq!(bcp47_to_iso639("pt"), "pt");
    }

    #[test]
    fn test_bcp47_with_region() {
        assert_eq!(bcp47_to_iso639("en-US"), "en");
        assert_eq!(bcp47_to_iso639("es-ES"), "es");
        assert_eq!(bcp47_to_iso639("es-MX"), "es");
        assert_eq!(bcp47_to_iso639("pt-BR"), "pt");
        assert_eq!(bcp47_to_iso639("pt-PT"), "pt");
    }

    #[test]
    fn test_bcp47_with_script() {
        assert_eq!(bcp47_to_iso639("zh-Hans-CN"), "zh");
        assert_eq!(bcp47_to_iso639("sr-Cyrl-RS"), "sr");
    }

    #[test]
    fn test_bcp47_underscore_separator() {
        assert_eq!(bcp47_to_iso639("en_US"), "en");
        assert_eq!(bcp47_to_iso639("pt_BR"), "pt");
    }

    #[test]
    fn test_bcp47_uppercase_normalized() {
        assert_eq!(bcp47_to_iso639("EN"), "en");
        assert_eq!(bcp47_to_iso639("PT-BR"), "pt");
    }

    #[test]
    fn test_bcp47_empty_falls_back() {
        assert_eq!(bcp47_to_iso639(""), "en");
    }

    #[test]
    fn test_bcp47_separator_only() {
        assert_eq!(bcp47_to_iso639("-"), "en");
        assert_eq!(bcp47_to_iso639("_"), "en");
    }

    #[test]
    fn test_bcp47_grandfathered_tags() {
        assert_eq!(bcp47_to_iso639("x-private"), "x");
        assert_eq!(bcp47_to_iso639("i-klingon"), "i");
    }

    #[test]
    fn test_detect_with_exact_match() {
        let supported = vec!["en".to_string(), "es".to_string()];
        assert_eq!(
            detect_system_locale_with(Some("en-US"), &supported),
            Some("en".to_string())
        );
        assert_eq!(
            detect_system_locale_with(Some("es-MX"), &supported),
            Some("es".to_string())
        );
    }

    #[test]
    fn test_detect_with_no_match_returns_none() {
        let supported = vec!["en".to_string()];
        assert_eq!(detect_system_locale_with(Some("fr-FR"), &supported), None);
    }

    #[test]
    fn test_detect_with_empty_supported_returns_none() {
        assert_eq!(detect_system_locale_with(Some("en-US"), &[]), None);
    }

    #[test]
    fn test_detect_with_none_input_returns_none() {
        let supported = vec!["en".to_string()];
        assert_eq!(detect_system_locale_with(None, &supported), None);
    }

    #[test]
    fn test_resolve_user_pref_takes_priority() {
        let config = Config {
            user_preferred_language: Some("es".to_string()),
            supported_languages: vec!["en".to_string(), "es".to_string()],
            default_language: "en".to_string(),
            ..Config::default()
        };
        let resolved = resolve_language_with(&config, Some("en-US"));
        assert_eq!(resolved.code, "es");
        assert_eq!(resolved.source, LanguageSource::UserPreference);
    }

    #[test]
    fn test_resolve_with_override_system_match() {
        let config = Config {
            user_preferred_language: None,
            supported_languages: vec!["en".to_string(), "es".to_string()],
            default_language: "en".to_string(),
            ..Config::default()
        };
        let resolved = resolve_language_with(&config, Some("es-ES"));
        assert_eq!(resolved.code, "es");
        assert_eq!(resolved.source, LanguageSource::SystemLocale);
    }

    #[test]
    fn test_resolve_with_override_no_match_falls_back() {
        let config = Config {
            user_preferred_language: None,
            supported_languages: vec!["en".to_string()],
            default_language: "en".to_string(),
            ..Config::default()
        };
        let resolved = resolve_language_with(&config, Some("fr-FR"));
        assert_eq!(resolved.code, "en");
        assert_eq!(resolved.source, LanguageSource::DefaultFallback);
    }

    #[test]
    fn test_resolve_user_pref_unsupported_falls_through_to_system() {
        let config = Config {
            user_preferred_language: Some("fr".to_string()),
            supported_languages: vec!["en".to_string(), "es".to_string()],
            default_language: "en".to_string(),
            ..Config::default()
        };
        let resolved = resolve_language_with(&config, Some("es-ES"));
        assert_eq!(resolved.code, "es");
        assert_eq!(resolved.source, LanguageSource::SystemLocale);
    }

    #[test]
    fn test_resolve_user_pref_unsupported_no_match_falls_to_default() {
        let config = Config {
            user_preferred_language: Some("fr".to_string()),
            supported_languages: vec!["en".to_string(), "es".to_string()],
            default_language: "en".to_string(),
            ..Config::default()
        };
        let resolved = resolve_language_with(&config, Some("de-DE"));
        assert_eq!(resolved.code, "en");
        assert_eq!(resolved.source, LanguageSource::DefaultFallback);
    }

    #[test]
    fn test_resolved_language_equality() {
        let a = ResolvedLanguage {
            code: "en".to_string(),
            source: LanguageSource::UserPreference,
        };
        let b = ResolvedLanguage {
            code: "en".to_string(),
            source: LanguageSource::UserPreference,
        };
        assert_eq!(a, b);
    }
}
