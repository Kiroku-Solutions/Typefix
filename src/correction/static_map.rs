//! Static error map for O(1) typo lookups
//!
//! Stores frequently made typos and their corrections for instant lookup.

use anyhow::Result;
use parking_lot::RwLock;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Thread-safe static error map
#[derive(Debug, Clone)]
pub struct StaticErrorMap {
    inner: Arc<RwLock<ErrorMapInner>>,
}

#[derive(Debug, Default)]
struct ErrorMapInner {
    /// Mapping from typo -> correction
    errors: HashMap<String, String>,
    /// Error frequency for ranking
    frequency: HashMap<String, u64>,
    /// User-learned errors
    user_errors: HashMap<String, String>,
    /// Language of this map
    language: String,
}

impl StaticErrorMap {
    /// Create a new empty error map
    pub fn new(language: &str) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ErrorMapInner {
                language: language.to_string(),
                ..Default::default()
            })),
        }
    }

    /// Maximum JSON file size in bytes (1MB) - prevents DoS from malicious files
    const MAX_JSON_SIZE: usize = 1024 * 1024;

    /// Load from JSON file with size limit
    pub fn from_json_file(path: &Path) -> Result<Self> {
        let metadata = std::fs::metadata(path)?;
        if metadata.len() as usize > Self::MAX_JSON_SIZE {
            anyhow::bail!(
                "JSON file too large: {} bytes (max {})",
                metadata.len(),
                Self::MAX_JSON_SIZE
            );
        }
        let content = std::fs::read_to_string(path)?;
        Self::from_json(&content)
    }

    /// Load from JSON string with size validation
    pub fn from_json(json: &str) -> Result<Self> {
        if json.len() > Self::MAX_JSON_SIZE {
            anyhow::bail!(
                "JSON too large: {} bytes (max {})",
                json.len(),
                Self::MAX_JSON_SIZE
            );
        }
        #[derive(Deserialize)]
        struct ErrorFile {
            language: String,
            errors: HashMap<String, String>,
        }

        let file: ErrorFile = serde_json::from_str(json)?;
        let map = Self::new(&file.language);

        {
            let mut inner = map.inner.write();
            inner.errors = file.errors;
            // Default frequency for static errors
            let keys: Vec<String> = inner.errors.keys().cloned().collect();
            for key in keys {
                inner.frequency.insert(key, 1000);
            }
        }

        Ok(map)
    }

    /// Look up a typo
    ///
    /// Checks user errors first (higher priority), then static errors.
    /// Returns the correction if found.
    pub fn lookup(&self, typo: &str) -> Option<String> {
        let inner = self.inner.read();
        let typo_lower = typo.to_lowercase();

        // Check user errors first (learned corrections)
        if let Some(correction) = inner.user_errors.get(&typo_lower) {
            return Some(correction.clone());
        }

        // Check static errors
        inner.errors.get(&typo_lower).cloned()
    }

    /// Add a user correction
    ///
    /// User corrections override static errors.
    pub fn learn(&self, typo: &str, correction: &str) {
        let typo_lower = typo.to_lowercase();
        let correction_lower = correction.to_lowercase();

        // Don't learn from itself
        if typo_lower == correction_lower {
            return;
        }

        {
            let mut inner = self.inner.write();
            inner
                .user_errors
                .insert(typo_lower.clone(), correction_lower);
            // Increment frequency
            *inner.frequency.entry(typo_lower).or_insert(0) += 1;
        }
    }

    /// Unlearn a correction (user marked it as wrong)
    pub fn unlearn(&self, typo: &str) {
        let mut inner = self.inner.write();
        let typo_lower = typo.to_lowercase();
        inner.user_errors.remove(&typo_lower);
    }

    /// Insert a static error correction
    pub fn insert_static(&self, typo: &str, correction: &str) {
        let mut inner = self.inner.write();
        inner
            .errors
            .insert(typo.to_lowercase(), correction.to_lowercase());
        inner.frequency.insert(typo.to_lowercase(), 1000);
    }

    /// Check if a word is a known typo
    pub fn is_known_typo(&self, word: &str) -> bool {
        let inner = self.inner.read();
        let word_lower = word.to_lowercase();
        inner.errors.contains_key(&word_lower) || inner.user_errors.contains_key(&word_lower)
    }

    /// Get frequency of a typo
    pub fn get_frequency(&self, typo: &str) -> u64 {
        let inner = self.inner.read();
        inner
            .frequency
            .get(&typo.to_lowercase())
            .copied()
            .unwrap_or(0)
    }

    /// Get all known typos
    pub fn all_typos(&self) -> Vec<(String, String)> {
        let inner = self.inner.read();
        let mut result: Vec<(String, String)> = inner
            .errors
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        // Add user errors (may override static)
        for (k, v) in &inner.user_errors {
            if !result.iter().any(|(key, _)| key == k) {
                result.push((k.clone(), v.clone()));
            }
        }

        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }

    /// Save user errors to file
    pub fn save_user_errors(&self, path: &Path) -> Result<()> {
        let inner = self.inner.read();
        let user_errors: HashMap<String, String> = inner.user_errors.clone();

        let json = serde_json::to_string_pretty(&user_errors)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load user errors from file
    pub fn load_user_errors(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(path)?;
        let errors: HashMap<String, String> = serde_json::from_str(&content)?;

        let mut inner = self.inner.write();
        for (typo, correction) in errors {
            inner
                .user_errors
                .insert(typo.to_lowercase(), correction.to_lowercase());
        }

        Ok(())
    }

    /// Clear all learned corrections
    pub fn clear_user_errors(&self) {
        let mut inner = self.inner.write();
        inner.user_errors.clear();
    }

    /// Get map statistics
    pub fn stats(&self) -> ErrorMapStats {
        let inner = self.inner.read();
        ErrorMapStats {
            static_errors: inner.errors.len(),
            user_errors: inner.user_errors.len(),
            language: inner.language.clone(),
        }
    }

    /// Get language
    pub fn language(&self) -> String {
        let inner = self.inner.read();
        inner.language.clone()
    }
}

/// Statistics about an error map
#[derive(Debug, Clone)]
pub struct ErrorMapStats {
    /// Number of static (loaded) error entries
    pub static_errors: usize,
    /// Number of user-learned error entries
    pub user_errors: usize,
    /// Language code (ISO 639-1) this map applies to
    pub language: String,
}

impl Default for StaticErrorMap {
    fn default() -> Self {
        Self::new("unknown")
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
    fn test_basic_lookup() {
        let map = StaticErrorMap::new("es");
        map.insert_static("qeu", "que");

        assert_eq!(map.lookup("qeu"), Some("que".to_string()));
        assert_eq!(map.lookup("QEU"), Some("que".to_string())); // Case insensitive
        assert_eq!(map.lookup("que"), None); // "que" is not a typo
    }

    #[test]
    fn test_learn() {
        let map = StaticErrorMap::new("es");
        map.learn("tengo", "tengo"); // Should be ignored (same)
        map.learn("mipropio", "mi propio");

        assert_eq!(map.lookup("mipropio"), Some("mi propio".to_string()));
    }

    #[test]
    fn test_user_override() {
        let map = StaticErrorMap::new("es");
        map.insert_static("qeu", "que");

        // User says "qeu" should actually be "qué"
        map.learn("qeu", "qué");

        // User correction takes priority
        assert_eq!(map.lookup("qeu"), Some("qué".to_string()));
    }

    #[test]
    fn test_unlearn() {
        let map = StaticErrorMap::new("es");
        // Learn a real correction (typo != correction)
        map.learn("testtypo", "test typo");
        assert_eq!(map.lookup("testtypo"), Some("test typo".to_string()));

        map.unlearn("testtypo");
        assert_eq!(map.lookup("testtypo"), None);
    }

    #[test]
    fn test_is_known_typo() {
        let map = StaticErrorMap::new("es");
        map.insert_static("qeu", "que");

        assert!(map.is_known_typo("qeu"));
        assert!(!map.is_known_typo("que"));
        assert!(!map.is_known_typo("unknown"));
    }

    #[test]
    fn test_frequency() {
        let map = StaticErrorMap::new("es");
        map.learn("test1", "correct1");
        map.learn("test1", "correct1"); // Learn twice
        map.learn("test2", "correct2");

        assert_eq!(map.get_frequency("test1"), 2);
        assert_eq!(map.get_frequency("test2"), 1);
    }

    #[test]
    fn test_json_parsing() {
        let json = r#"{
            "language": "en",
            "errors": {
                "teh": "the",
                "qeu": "que"
            }
        }"#;

        let map = StaticErrorMap::from_json(json).unwrap();
        assert_eq!(map.language(), "en");
        assert_eq!(map.lookup("teh"), Some("the".to_string()));
        assert_eq!(map.lookup("qeu"), Some("que".to_string()));
    }

    #[test]
    fn test_stats() {
        let map = StaticErrorMap::new("es");
        map.insert_static("qeu", "que");
        map.learn("mipalabra", "mi palabra");

        let stats = map.stats();
        assert_eq!(stats.static_errors, 1);
        assert_eq!(stats.user_errors, 1);
        assert_eq!(stats.language, "es");
    }

    #[test]
    fn test_all_typos() {
        let map = StaticErrorMap::new("en");
        map.insert_static("teh", "the");
        map.learn("mytypo", "my typo");

        let all = map.all_typos();
        assert!(all.len() >= 2);
    }

    #[test]
    fn test_clear_user_errors() {
        let map = StaticErrorMap::new("en");
        map.learn("typo1", "correct1");
        map.learn("typo2", "correct2");

        assert!(map.lookup("typo1").is_some());

        map.clear_user_errors();
        assert!(map.lookup("typo1").is_none());
    }
}
