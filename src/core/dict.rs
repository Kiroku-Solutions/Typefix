use anyhow::{Context, Result};
use fst::automaton::Levenshtein;
use fst::{IntoStreamer, Map, Streamer};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use crate::core::encoder::{decode_accents, encode_accents};

#[cfg(not(target_arch = "wasm32"))]
use memmap2::Mmap;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub enum DictData {
    Mmap(std::sync::Arc<Mmap>),
    Bytes(std::sync::Arc<[u8]>),
}

#[cfg(not(target_arch = "wasm32"))]
impl AsRef<[u8]> for DictData {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Mmap(m) => m.as_ref(),
            Self::Bytes(b) => b.as_ref(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub type DictData = std::sync::Arc<[u8]>;

/// Dictionary based on Finite State Transducers (FST) for high performance and minimal memory footprint.
#[derive(Clone)]
pub struct Dict {
    map: Map<DictData>,
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
    #[cfg(target_arch = "wasm32")]
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        let arc_bytes: std::sync::Arc<[u8]> = bytes.into();
        let map = Map::new(arc_bytes).context("Failed to load FST map from bytes")?;
        let word_count = map.len();
        Ok(Self { map, word_count })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        let arc_bytes: std::sync::Arc<[u8]> = bytes.into();
        let data = DictData::Bytes(arc_bytes);
        let map = Map::new(data).context("Failed to load FST map from bytes")?;
        let word_count = map.len();
        Ok(Self { map, word_count })
    }

    /// Load a dictionary from an FST file
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_fst_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let data = DictData::Mmap(std::sync::Arc::new(mmap));
        let map = Map::new(data).context("Failed to load FST map from file")?;
        let word_count = map.len();
        Ok(Self { map, word_count })
    }

    /// Load a dictionary from an FST file (WASM fallback - shouldn't happen, but just in case it compiles)
    #[cfg(target_arch = "wasm32")]
    pub fn from_fst_file<P: AsRef<Path>>(_path: P) -> Result<Self> {
        anyhow::bail!("Filesystem operations not supported in WASM")
    }


    /// Search for an exact word match and return its frequency
    pub fn search(&self, word: &str) -> Option<u64> {
        let encoded = encode_accents(word);
        self.map.get(encoded.as_bytes())
    }

    /// Check if a word exists in the dictionary
    pub fn contains(&self, word: &str) -> bool {
        let encoded = encode_accents(word);
        self.map.contains_key(encoded.as_bytes())
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
        
        let encoded_word = encode_accents(word);
        let lev = match Levenshtein::new(&encoded_word, fst_distance as u32) {
            Ok(l) => l,
            Err(_) => return Vec::new(),
        };

        let mut stream = self.map.search(lev).into_stream();
        let mut results = Vec::new();

        while let Some((k, v)) = stream.next() {
            if let Ok(matched_encoded) = std::str::from_utf8(k) {
                // Decode the matched word back to its UTF-8 accented form
                let matched_word = decode_accents(matched_encoded);
                
                // We calculate exact damerau distance to sort properly on the original UTF-8 characters
                let dist = damerau_distance(word, &matched_word, max_distance);
                if dist <= max_distance {
                    // Overlap filter (Gibberish prevention)
                    if has_sufficient_overlap(word, &matched_word) {
                        results.push((matched_word, dist, v));
                    }
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

/// Check if two words share at least 60% of their characters
fn has_sufficient_overlap(word: &str, candidate: &str) -> bool {
    let mut word_counts = std::collections::HashMap::new();
    for c in word.chars() {
        *word_counts.entry(c).or_insert(0) += 1;
    }
    
    let mut overlap = 0;
    for c in candidate.chars() {
        if let Some(count) = word_counts.get_mut(&c) {
            if *count > 0 {
                overlap += 1;
                *count -= 1;
            }
        }
    }
    
    let max_len = word.chars().count().max(candidate.chars().count());
    if max_len == 0 {
        return false;
    }
    
    // For very short words (1-2 chars), avoid aggressive filtering
    if max_len <= 2 {
        return overlap > 0;
    }
    
    (overlap as f32 / max_len as f32) >= 0.6
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
        let mut last_col: Option<usize> = None;
        for (j, c2) in s2_chars.iter().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            let prev = matrix[i][j] + cost;
            let del = matrix[i][j + 1] + 1;
            let ins = matrix[i + 1][j] + 1;

            let mut trans = usize::MAX;
            if let Some(&prev_col) = last_row.get(c2) {
                if let Some(l_col) = last_col {
                    if prev_col < i && l_col < j {
                        trans = matrix[prev_col][l_col] + (i - prev_col - 1) + 1 + (j - l_col - 1);
                    }
                }
            }

            matrix[i + 1][j + 1] = prev.min(del).min(ins).min(trans);
            if cost == 0 {
                last_col = Some(j);
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
        
        // Encode accents before sorting
        for entry in &mut entries {
            entry.word = encode_accents(&entry.word);
        }

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
