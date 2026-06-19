use anyhow::{Context, Result};
use fst::automaton::Levenshtein;
use fst::{IntoStreamer, Map, Streamer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// Dictionary based on Finite State Transducers (FST) for high performance and minimal memory footprint.
#[derive(Clone)]
pub struct Dict {
    map: Map<Vec<u8>>,
    word_count: usize,
}

// Map does not implement Debug nicely without scanning, so we provide a custom implementation
impl std::fmt::Debug for Dict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dict")
            .field("word_count", &self.word_count)
            .finish()
    }
}

impl Dict {
    /// Load a dictionary from pre-compiled FST bytes
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        let map = Map::new(bytes).context("Failed to load FST map from bytes")?;
        let word_count = map.len();
        Ok(Self { map, word_count })
    }

    /// Load a dictionary from an FST file
    pub fn from_fst_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Self::from_bytes(bytes)
    }

    /// Search for an exact word match and return its frequency
    pub fn search(&self, word: &str) -> Option<u64> {
        self.map.get(word.as_bytes())
    }

    /// Check if a word exists in the dictionary
    pub fn contains(&self, word: &str) -> bool {
        self.map.contains_key(word.as_bytes())
    }

    /// Get the number of words in the dictionary
    pub fn len(&self) -> usize {
        self.word_count
    }

    /// Check if the dictionary is empty
    pub fn is_empty(&self) -> bool {
        self.word_count == 0
    }

    /// Find words within maximum edit distance (Levenshtein)
    /// Note: FST uses standard Levenshtein, not Damerau-Levenshtein. We will do a post-filter
    /// with our Damerau-Levenshtein function if needed, or just rely on Levenshtein.
    pub fn find_similar(
        &self,
        word: &str,
        max_distance: usize,
        limit: usize,
    ) -> Vec<(String, usize, u64)> {
        // Build Levenshtein automaton. 
        // A Damerau-Levenshtein transposition counts as 2 standard Levenshtein edits.
        // Therefore, we must search the FST with at least distance 2 to catch transpositions 
        // when max_distance is 1. We cap FST distance at 2 because distance 3 is very slow.
        let fst_distance = if max_distance == 1 { 2 } else { max_distance };
        
        let lev = match Levenshtein::new(word, fst_distance as u32) {
            Ok(l) => l,
            Err(_) => return Vec::new(),
        };

        let mut stream = self.map.search(lev).into_stream();
        let mut results = Vec::new();

        while let Some((k, v)) = stream.next() {
            if let Ok(matched_word) = std::str::from_utf8(k) {
                // We calculate exact damerau distance to sort properly
                let dist = damerau_distance(word, matched_word, max_distance);
                if dist <= max_distance {
                    results.push((matched_word.to_string(), dist, v));
                }
            }
        }

        // Sort by distance (ascending), then frequency (descending)
        results.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| b.2.cmp(&a.2)));

        if limit > 0 && results.len() > limit {
            results.truncate(limit);
        }

        results
    }
}

/// Calculate Damerau-Levenshtein distance between two strings
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

/// JSON compilation helper
impl Dict {
    /// Compiles a legacy JSON dictionary into a binary FST file
    pub fn compile_json_to_fst(json_path: &Path, fst_path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(json_path)?;
        
        #[derive(Deserialize)]
        struct DictFile {
            words: Vec<DictEntry>,
        }

        #[derive(Deserialize)]
        struct DictEntry {
            word: String,
            #[serde(default)]
            frequency: u64,
        }

        let dict_file: DictFile = serde_json::from_str(&content)?;
        
        // FST requires lexicographically sorted keys
        let mut entries = dict_file.words;
        entries.sort_by(|a, b| a.word.cmp(&b.word));

        // Ensure unique keys
        entries.dedup_by(|a, b| a.word == b.word);

        let mut builder = fst::MapBuilder::memory();
        for entry in entries {
            // Frequency defaults to 1 if not present, but let's handle 0
            let freq = if entry.frequency == 0 { 1 } else { entry.frequency };
            builder.insert(entry.word.as_bytes(), freq)?;
        }

        let bytes = builder.into_inner()?;
        let mut file = File::create(fst_path)?;
        file.write_all(&bytes)?;

        Ok(())
    }
}
