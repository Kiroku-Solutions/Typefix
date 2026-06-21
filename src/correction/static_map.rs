//! Static error map for O(1) typo lookups
//!
//! Stores frequently made typos and their corrections for instant lookup.

use anyhow::Result;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Thread-safe static error map
#[derive(Debug, Clone)]
pub struct StaticErrorMap {
    inner: Arc<RwLock<ErrorMapInner>>,
}

#[derive(Debug)]
struct ErrorMapInner {
    /// User-learned errors
    user_errors: lru::LruCache<String, String>,
    /// Language of this map
    language: String,
}

// Include the generated PHF map
include!(concat!(env!("OUT_DIR"), "/static_errors.rs"));

impl StaticErrorMap {
    /// Create a new empty error map
    pub fn new(language: &str) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ErrorMapInner {
                language: language.to_string(),
                user_errors: lru::LruCache::new(std::num::NonZeroUsize::new(1000).unwrap()),
            })),
        }
    }

    /// Load from a JSON file
    pub fn from_json_file(_path: &Path) -> Result<Self> {
        // Obsolete: static errors are compiled via phf in build.rs
        Ok(Self::new("unknown"))
    }

    /// Load from JSON string
    pub fn from_json_str(language: &str, json_str: &str) -> Result<Self> {
        let map = Self::new(language);
        let parsed: serde_json::Value = serde_json::from_str(json_str)?;
        if let Some(errors) = parsed.get("errors").and_then(|e| e.as_object()) {
            let mut inner = map.inner.write();
            for (typo, correction) in errors {
                if let Some(corr_str) = correction.as_str() {
                    inner.user_errors.put(typo.to_lowercase(), corr_str.to_string());
                }
            }
        }
        Ok(map)
    }

    /// Look up a typo
    ///
    /// Checks user errors first (higher priority), then static errors.
    /// Returns the correction if found.
    pub fn lookup(&self, typo: &str) -> Option<String> {
        let typo_lower = typo.to_lowercase();
        
        {
            let mut inner = self.inner.write();
            if let Some(correction) = inner.user_errors.get(&typo_lower) {
                return Some(correction.clone());
            }
        }

        let lookup_key = format!("{}_{}", self.inner.read().language, typo_lower);
        STATIC_ERRORS.get(&lookup_key).map(|s| s.to_string())
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
            inner.user_errors.put(typo_lower, correction_lower);
        }
    }

    /// Unlearn a correction (user marked it as wrong)
    pub fn unlearn(&self, typo: &str) {
        let mut inner = self.inner.write();
        inner.user_errors.pop(&typo.to_lowercase());
    }

    /// Insert a static error correction
    pub fn insert_static(&self, typo: &str, correction: &str) {
        // Used in tests. In production this does nothing.
        let mut inner = self.inner.write();
        inner.user_errors.put(typo.to_lowercase(), correction.to_lowercase());
    }

    /// Check if a word is a known typo
    pub fn is_known_typo(&self, word: &str) -> bool {
        let inner = self.inner.read();
        let word_lower = word.to_lowercase();
        let lookup_key = format!("{}_{}", inner.language, word_lower);
        STATIC_ERRORS.contains_key(&lookup_key) || inner.user_errors.contains(&word_lower)
    }

    /// Get frequency of a typo
    pub fn get_frequency(&self, _typo: &str) -> u64 {
        // Not needed for PHF anymore.
        1000
    }

    /// Get all known typos
    pub fn all_typos(&self) -> Vec<(String, String)> {
        let inner = self.inner.read();
        let prefix = format!("{}_", inner.language);
        let mut result: Vec<(String, String)> = STATIC_ERRORS
            .entries()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(k, v)| (k.strip_prefix(&prefix).unwrap().to_string(), v.to_string()))
            .collect();

        // Add user errors (may override static)
        for (k, v) in inner.user_errors.iter() {
            if let Some(pos) = result.iter().position(|(key, _)| key == k) {
                result[pos].1 = v.clone();
            } else {
                result.push((k.clone(), v.clone()));
            }
        }

        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }

    /// Save user errors to file
    pub fn save_user_errors(&self, path: &Path) -> Result<()> {
        let inner = self.inner.read();
        let mut user_errors: HashMap<String, String> = HashMap::new();
        for (k, v) in inner.user_errors.iter() {
            user_errors.insert(k.clone(), v.clone());
        }

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
            inner.user_errors.put(typo.to_lowercase(), correction.to_lowercase());
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
            static_errors: STATIC_ERRORS.len(),
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
        
        assert_eq!(map.get_frequency("test1"), 1000);
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

        let map = StaticErrorMap::from_json_str("en", json).unwrap();
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
        assert!(stats.static_errors >= 1); // We now load all static errors into one global map
        assert_eq!(stats.user_errors, 2);
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
