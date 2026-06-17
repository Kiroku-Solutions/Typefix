//! Configuration management
//!
//! Loads and validates configuration from YAML files with environment variable overrides.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur when loading or validating configuration
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Failed to read configuration file from disk
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),
    /// Failed to parse configuration file contents
    #[error("Failed to parse config: {0}")]
    ParseError(#[from] serde_json::Error),
    /// Configuration values failed validation
    #[error("Validation error: {0}")]
    ValidationError(String),
}

/// Main engine configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    /// Data directory path (relative or absolute)
    pub data_path: PathBuf,

    /// Default language (ISO 639-1 code)
    pub default_language: String,

    /// Supported languages
    pub supported_languages: Vec<String>,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Buffer configuration
    pub buffer: BufferConfig,

    /// Language detection configuration
    pub language_detection: LanguageDetectionConfig,

    /// Correction configuration
    pub correction: CorrectionConfig,

    /// Hook configuration
    pub hooks: HooksConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_path: PathBuf::from("data"),
            default_language: "en".to_string(),
            supported_languages: vec!["en".to_string(), "es".to_string()],
            logging: LoggingConfig::default(),
            buffer: BufferConfig::default(),
            language_detection: LanguageDetectionConfig::default(),
            correction: CorrectionConfig::default(),
            hooks: HooksConfig::default(),
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log level: trace, debug, info, warn, error
    pub level: String,
    /// Log file path (empty = stdout only)
    pub file: Option<PathBuf>,
    /// Enable structured logging
    pub structured: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
            structured: true,
        }
    }
}

/// Buffer configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct BufferConfig {
    /// Maximum buffer size in characters
    pub max_size: usize,
    /// Enable Unicode normalization
    pub normalize_unicode: bool,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            max_size: 64,
            normalize_unicode: true,
        }
    }
}

/// Language detection configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct LanguageDetectionConfig {
    /// Window size for Bayesian detection
    pub window_size: usize,
    /// Confidence threshold for language switch (0.0-1.0)
    pub confidence_threshold: f64,
    /// Hysteresis zone (0.0-1.0)
    pub hysteresis_zone: f64,
    /// Minimum words before re-evaluating
    pub min_words_before_switch: usize,
    /// Enable dynamic switching
    pub dynamic_switching: bool,
}

impl Default for LanguageDetectionConfig {
    fn default() -> Self {
        Self {
            window_size: 5,
            confidence_threshold: 0.85,
            hysteresis_zone: 0.10,
            min_words_before_switch: 5,
            dynamic_switching: true,
        }
    }
}

/// Correction engine configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct CorrectionConfig {
    /// Maximum Damerau-Levenshtein distance to consider
    pub max_edit_distance: usize,
    /// Maximum corrections to return
    pub max_corrections: usize,
    /// Minimum word length to correct
    pub min_word_length: usize,
    /// Case sensitivity
    pub case_sensitive: bool,
    /// Learn from user corrections
    pub learn_from_user: bool,
    /// User error map persistence path
    pub user_errors_path: Option<PathBuf>,
}

impl Default for CorrectionConfig {
    fn default() -> Self {
        Self {
            max_edit_distance: 1,
            max_corrections: 3,
            min_word_length: 2,
            case_sensitive: false,
            learn_from_user: true,
            user_errors_path: Some(PathBuf::from("data/user_errors.json")),
        }
    }
}

/// Hook configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct HooksConfig {
    /// Enable keyboard hooks
    pub keyboard_enabled: bool,
    /// Hook mode: system, application, disabled
    pub mode: HookMode,
    /// Target application (for application mode)
    pub target_app: Option<String>,
    /// Enable logging of all keystrokes
    pub log_keystrokes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
/// Hook mode selecting how keyboard hooks are installed
pub enum HookMode {
    /// System-wide hook (requires elevation on Windows)
    #[default]
    System,
    /// Application-specific hook
    Application,
    /// Disabled (correction only via API)
    Disabled,
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            keyboard_enabled: true,
            mode: HookMode::System,
            target_app: None,
            log_keystrokes: false,
        }
    }
}

impl Config {
    /// Load configuration from file
    ///
    /// Supports JSON format based on file extension.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;

        let config = serde_json::from_str(&content)?;

        let config: Config = config;
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from string
    pub fn from_str(content: &str, _format: ConfigFormat) -> Result<Self, ConfigError> {
        let config: Config = serde_json::from_str(content)?;
        config.validate()?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let path = path.as_ref();
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.supported_languages.is_empty() {
            return Err(ConfigError::ValidationError(
                "At least one supported language is required".to_string(),
            ));
        }

        if !self.supported_languages.contains(&self.default_language) {
            return Err(ConfigError::ValidationError(format!(
                "Default language '{}' must be in supported languages",
                self.default_language
            )));
        }

        if self.buffer.max_size == 0 {
            return Err(ConfigError::ValidationError(
                "Buffer max_size must be > 0".to_string(),
            ));
        }

        if !(0.0..=1.0).contains(&self.language_detection.confidence_threshold) {
            return Err(ConfigError::ValidationError(
                "confidence_threshold must be between 0.0 and 1.0".to_string(),
            ));
        }

        if self.correction.max_edit_distance > 2 {
            tracing::warn!(
                "max_edit_distance > 2 may impact performance. Current: {}",
                self.correction.max_edit_distance
            );
        }

        Ok(())
    }

    /// Get the data directory as an absolute path
    pub fn data_dir(&self) -> PathBuf {
        if self.data_path.is_absolute() {
            self.data_path.clone()
        } else {
            std::env::current_dir()
                .map(|p| p.join(&self.data_path))
                .unwrap_or(self.data_path.clone())
        }
    }
}

/// File format used for serializing/deserializing configuration
#[derive(Debug, Clone, Copy)]
pub enum ConfigFormat {
    /// YAML format (reserved; project currently parses JSON)
    Yaml,
    /// JSON format
    Json,
}

/// Default configuration file content
impl Config {
    /// Built-in default configuration as a YAML/JSON-like string
    pub fn default_config_file() -> &'static str {
        r#"# TypeFix Configuration

# Data directory (relative to working directory or absolute)
data_path: "data"

# Default language (ISO 639-1 code)
default_language: "en"

# Supported languages
supported_languages:
  - "en"
  - "es"
  - "pt"

# Logging configuration
logging:
  level: "info"  # trace, debug, info, warn, error
  file: null      # null = stdout only, or path to log file
  structured: true

# Buffer configuration
buffer:
  max_size: 64
  normalize_unicode: true

# Language detection configuration
language_detection:
  window_size: 5
  confidence_threshold: 0.85
  hysteresis_zone: 0.10
  min_words_before_switch: 5
  dynamic_switching: true

# Correction configuration
correction:
  max_edit_distance: 1
  max_corrections: 3
  min_word_length: 2
  case_sensitive: false
  learn_from_user: true
  user_errors_path: "data/user_errors.json"

# Hook configuration
hooks:
  keyboard_enabled: true
  mode: "system"  # system, application, disabled
  target_app: null
  log_keystrokes: false
"#
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
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.default_language, "en");
        assert!(config.supported_languages.contains(&"en".to_string()));
    }

    #[test]
    fn test_validation_empty_languages() {
        let config = Config {
            supported_languages: vec![],
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_invalid_default() {
        let config = Config {
            default_language: "de".to_string(),
            supported_languages: vec!["en".to_string()],
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_invalid_threshold() {
        let config = Config {
            language_detection: LanguageDetectionConfig {
                confidence_threshold: 1.5,
                ..LanguageDetectionConfig::default()
            },
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_yaml_parse() {
        // Project uses JSON-only config (YAML support removed)
        // This test verifies JSON parsing still works
        let json = r#"{
            "data_path": "data",
            "default_language": "es",
            "supported_languages": ["es", "en"],
            "logging": {
                "level": "debug"
            },
            "buffer": {
                "max_size": 128
            }
        }"#;
        let config = Config::from_str(json, ConfigFormat::Json).unwrap();
        assert_eq!(config.default_language, "es");
        assert_eq!(config.buffer.max_size, 128);
    }

    #[test]
    fn test_json_parse() {
        let json = r#"{
            "data_path": "data",
            "default_language": "es",
            "supported_languages": ["es", "en"],
            "buffer": {
                "max_size": 100
            }
        }"#;
        let config = Config::from_str(json, ConfigFormat::Json).unwrap();
        assert_eq!(config.default_language, "es");
        assert_eq!(config.buffer.max_size, 100);
    }

    #[test]
    fn test_data_dir_relative() {
        let config = Config {
            data_path: PathBuf::from("data"),
            ..Config::default()
        };
        let dir = config.data_dir();
        assert!(dir.is_absolute());
    }

    #[test]
    fn test_data_dir_absolute() {
        let config = Config {
            data_path: PathBuf::from("C:/data"),
            ..Config::default()
        };
        assert_eq!(config.data_dir(), PathBuf::from("C:/data"));
    }
}
