//! Trie data structure for efficient word lookups and prefix searches
//!
//! This implementation is immutable after construction, supporting concurrent
//! reads via Arc/RwLock at the engine level.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Trie node - stores children and optional word metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    is_end: bool,
    frequency: u64,
    word: Option<String>,
}

impl TrieNode {
    #[allow(
        dead_code,
        reason = "constructor reserved for future trie builders, used in tests"
    )]
    fn new() -> Self {
        Self::default()
    }

    #[allow(
        dead_code,
        reason = "constructor used in tests for pre-populated trie nodes"
    )]
    fn with_word(word: String, frequency: u64) -> Self {
        let mut node = Self::new();
        node.is_end = true;
        node.frequency = frequency;
        node.word = Some(word);
        node
    }
}

/// Trie structure for efficient prefix and exact-match lookups
///
/// # Type Parameters
/// * `K` - Key type for storing additional metadata at word endings
///
/// # Guarantees
/// * Immutable after construction - safe for concurrent reads
/// * O(m) lookup where m = word length
/// * O(m) prefix search where m = prefix length
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Trie {
    root: TrieNode,
    word_count: usize,
}

impl Trie {
    /// Create a new empty Trie
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a word with associated frequency
    ///
    /// # Arguments
    /// * `word` - The word to insert (lowercase recommended)
    /// * `frequency` - Word frequency for ranking corrections
    ///
    /// # Example
    /// ```
    /// use typefix::core::Trie;
    /// let mut trie = Trie::new();
    /// trie.insert("hola", 1000);
    /// assert!(trie.search("hola").is_some());
    /// ```
    pub fn insert(&mut self, word: &str, frequency: u64) {
        let mut current = &mut self.root;
        for ch in word.chars() {
            current.children.entry(ch).or_default();
            match current.children.get_mut(&ch) {
                Some(node) => current = node,
                None => return,
            }
        }
        if !current.is_end {
            self.word_count += 1;
        }
        current.is_end = true;
        current.frequency = frequency;
        current.word = Some(word.to_string());
    }

    /// Insert a word with default frequency of 1
    pub fn insert_word(&mut self, word: &str) {
        self.insert(word, 1);
    }

    /// Search for an exact word match
    ///
    /// # Returns
    /// `Some(frequency)` if word exists, `None` otherwise
    pub fn search(&self, word: &str) -> Option<u64> {
        let mut current = &self.root;
        for ch in word.chars() {
            match current.children.get(&ch) {
                Some(node) => current = node,
                None => return None,
            }
        }
        if current.is_end {
            Some(current.frequency)
        } else {
            None
        }
    }

    /// Check if a word exists in the Trie
    pub fn contains(&self, word: &str) -> bool {
        self.search(word).is_some()
    }

    /// Check if any words start with the given prefix
    pub fn starts_with(&self, prefix: &str) -> bool {
        let mut current = &self.root;
        for ch in prefix.chars() {
            match current.children.get(&ch) {
                Some(node) => current = node,
                None => return false,
            }
        }
        true
    }

    /// Get all words that start with the given prefix
    ///
    /// # Arguments
    /// * `prefix` - The prefix to search for
    /// * `max_results` - Maximum number of results to return (0 = unlimited)
    ///
    /// # Returns
    /// Vector of (word, frequency) tuples sorted by frequency descending
    pub fn get_all_with_prefix(&self, prefix: &str, max_results: usize) -> Vec<(String, u64)> {
        let mut current = &self.root;
        for ch in prefix.chars() {
            match current.children.get(&ch) {
                Some(node) => current = node,
                None => return Vec::new(),
            }
        }

        let mut results = Vec::new();
        self._collect_words(current, prefix, &mut results);

        // Sort by frequency descending
        results.sort_by_key(|b| std::cmp::Reverse(b.1));

        if max_results > 0 && results.len() > max_results {
            results.truncate(max_results);
        }

        results
    }

    /// Recursive helper to collect all words from a node
    fn _collect_words(&self, node: &TrieNode, prefix: &str, results: &mut Vec<(String, u64)>) {
        if node.is_end {
            if let Some(ref word) = node.word {
                results.push((word.clone(), node.frequency));
            }
        }
        for (ch, child) in &node.children {
            let new_prefix = format!("{}{}", prefix, ch);
            self._collect_words(child, &new_prefix, results);
        }
    }

    /// Get the number of words in the Trie
    pub fn len(&self) -> usize {
        self.word_count
    }

    /// Check if the Trie is empty
    pub fn is_empty(&self) -> bool {
        self.word_count == 0
    }

    /// Get all words in the Trie
    pub fn all_words(&self) -> Vec<(String, u64)> {
        let mut results = Vec::new();
        self._collect_words(&self.root, "", &mut results);
        results.sort_by_key(|b| std::cmp::Reverse(b.1));
        results
    }

    /// Find words within maximum edit distance (Damerau-Levenshtein)
    ///
    /// # Arguments
    /// * `word` - The word to search for
    /// * `max_distance` - Maximum allowed distance (typically 1 or 2)
    /// * `limit` - Maximum number of results
    ///
    /// # Returns
    /// Vector of (word, distance, frequency) tuples sorted by distance, then frequency
    pub fn find_similar(
        &self,
        word: &str,
        max_distance: usize,
        limit: usize,
    ) -> Vec<(String, usize, u64)> {
        let all_words = self.all_words();
        let mut results: Vec<(String, usize, u64)> = all_words
            .into_iter()
            .filter_map(|(w, freq)| {
                let dist = damerau_distance(word, &w, max_distance);
                if dist <= max_distance {
                    Some((w, dist, freq))
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| b.2.cmp(&a.2)));

        if limit > 0 && results.len() > limit {
            results.truncate(limit);
        }

        results
    }
}

/// Calculate Damerau-Levenshtein distance between two strings
///
/// This is a simplified version for use within the Trie module.
/// The full implementation with optimizations is in the correction module.
fn damerau_distance(s1: &str, s2: &str, max_dist: usize) -> usize {
    if s1.is_empty() {
        return s2.chars().count().min(max_dist + 1);
    }
    if s2.is_empty() {
        return s1.chars().count().min(max_dist + 1);
    }

    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();
    let len1 = s1_chars.len();
    let len2 = s2_chars.len();

    // Early exit if length difference exceeds max_dist
    if (len1 as i64 - len2 as i64).unsigned_abs() as usize > max_dist {
        return max_dist + 1;
    }

    let mut matrix = vec![vec![0usize; len2 + 1]; len1 + 1];

    for (i, row) in matrix.iter_mut().enumerate().take(len1 + 1) {
        row[0] = i;
    }
    for (j, val) in matrix[0].iter_mut().enumerate().take(len2 + 1) {
        *val = j;
    }

    let mut last_row: HashMap<char, usize> = HashMap::new();

    for (i, c1) in s1_chars.iter().enumerate() {
        let mut last_col = 0;
        for (j, c2) in s2_chars.iter().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            let prev = matrix[i][j] + cost;
            let del = matrix[i][j + 1] + 1;
            let ins = matrix[i + 1][j] + 1;

            let mut trans = usize::MAX;
            if let Some(&prev_col) = last_row.get(c2) {
                if prev_col < i && last_col < j {
                    trans =
                        matrix[prev_col][last_col] + (i - prev_col - 1) + 1 + (j - last_col - 1);
                }
            }

            matrix[i + 1][j + 1] = prev.min(del).min(ins).min(trans);
            if cost == 0 {
                last_col = j;
            }
        }
        last_row.insert(*c1, i);
    }

    matrix[len1][len2].min(max_dist + 1)
}

/// Load Trie from JSON file
impl Trie {
    /// Load Trie from a JSON file with format:
    /// ```json
    /// {
    ///   "language": "es",
    ///   "version": "1.0",
    ///   "words": [
    ///     {"word": "hola", "frequency": 1000},
    ///     {"word": "mundo", "frequency": 500}
    ///   ]
    /// }
    /// ```
    pub fn from_json_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_json(&content)
    }

    /// Load Trie from JSON string
    pub fn from_json(json: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct DictFile {
            words: Vec<DictEntry>,
        }

        #[derive(Deserialize)]
        struct DictEntry {
            word: String,
            #[serde(default = "default_frequency")]
            frequency: u64,
        }

        fn default_frequency() -> u64 {
            1
        }

        let dict_file: DictFile = serde_json::from_str(json)?;
        let mut trie = Trie::new();

        for entry in dict_file.words {
            trie.insert(&entry.word, entry.frequency);
        }

        Ok(trie)
    }

    /// Serialize Trie to JSON string
    pub fn to_json(&self) -> Result<String> {
        #[derive(Serialize)]
        struct DictEntry {
            word: String,
            frequency: u64,
        }

        #[derive(Serialize)]
        struct DictFile<'a> {
            language: &'a str,
            version: &'a str,
            words: Vec<DictEntry>,
        }

        let words: Vec<DictEntry> = self
            .all_words()
            .into_iter()
            .map(|(w, f)| DictEntry {
                word: w,
                frequency: f,
            })
            .collect();

        let dict_file = DictFile {
            language: "unknown",
            version: "1.0",
            words,
        };

        serde_json::to_string_pretty(&dict_file).map_err(Into::into)
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
    fn test_insert_and_search() {
        let mut trie = Trie::new();
        trie.insert("hola", 100);
        trie.insert("mundo", 50);
        trie.insert("hola", 200); // Update frequency

        assert_eq!(trie.search("hola"), Some(200));
        assert_eq!(trie.search("mundo"), Some(50));
        assert_eq!(trie.search("noexiste"), None);
    }

    #[test]
    fn test_starts_with() {
        let mut trie = Trie::new();
        trie.insert("hola", 100);
        trie.insert("hola mundo", 50);
        trie.insert("adios", 25);

        assert!(trie.starts_with("ho"));
        assert!(trie.starts_with("hola"));
        assert!(trie.starts_with("ad"));
        assert!(!trie.starts_with("xyz"));
    }

    #[test]
    fn test_get_all_with_prefix() {
        let mut trie = Trie::new();
        trie.insert("hola", 100);
        trie.insert("hola mundo", 50);
        trie.insert("hola amigos", 75);
        trie.insert("adios", 25);

        let results = trie.get_all_with_prefix("hola", 10);
        assert_eq!(results.len(), 3); // hola, hola mundo, hola amigos
        assert_eq!(results[0].0, "hola"); // Highest frequency first
        assert_eq!(results[0].1, 100); // Frequency preserved
    }

    #[test]
    fn test_len_and_empty() {
        let mut trie = Trie::new();
        assert!(trie.is_empty());
        assert_eq!(trie.len(), 0);

        trie.insert("hola", 100);
        assert!(!trie.is_empty());
        assert_eq!(trie.len(), 1);
    }

    #[test]
    fn test_damerau_distance() {
        // Transposition
        assert_eq!(damerau_distance("qeu", "que", 1), 1);
        // Insertion
        assert_eq!(damerau_distance("hola", "holaa", 1), 1);
        // Deletion
        assert_eq!(damerau_distance("holaa", "hola", 1), 1);
        // Substitution
        assert_eq!(damerau_distance("hola", "holb", 1), 1);
        // No match
        assert!(damerau_distance("hola", "xyz", 1) > 1);
    }

    #[test]
    fn test_json_serialize() {
        let mut trie = Trie::new();
        trie.insert("hola", 100);
        trie.insert("mundo", 50);

        let json = trie.to_json().unwrap();
        assert!(json.contains("hola"));
        assert!(json.contains("100"));
    }

    #[test]
    fn test_json_deserialize() {
        let json = r#"{
            "words": [
                {"word": "hola", "frequency": 100},
                {"word": "mundo", "frequency": 50}
            ]
        }"#;

        let trie = Trie::from_json(json).unwrap();
        assert!(trie.contains("hola"));
        assert!(trie.contains("mundo"));
        assert_eq!(trie.search("hola"), Some(100));
    }

    #[test]
    fn test_unicode() {
        let mut trie = Trie::new();
        trie.insert("café", 100);
        trie.insert("naïve", 50);
        trie.insert("日本語", 25);

        assert!(trie.contains("café"));
        assert!(trie.contains("naïve"));
        assert!(trie.contains("日本語"));
    }

    #[test]
    fn test_emoji_boundary() {
        let mut trie = Trie::new();
        trie.insert("😀", 100);
        trie.insert("hola 😀 mundo", 50);

        assert!(trie.contains("😀"));
        assert!(trie.starts_with("hola"));
    }

    #[test]
    fn test_find_similar() {
        let mut trie = Trie::new();
        trie.insert("hola", 100);
        trie.insert("bola", 50);
        trie.insert("pola", 30);
        trie.insert("jola", 10);

        let results = trie.find_similar("hola", 1, 10);
        // "bola" and "pola" are distance 1, "jola" is distance 1
        assert!(results.iter().any(|(w, d, _)| w == "bola" && *d == 1));
    }
}
