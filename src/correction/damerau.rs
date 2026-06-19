//! Damerau-Levenshtein distance algorithm
//!
//! Calculates the minimum edit distance between two strings including
//! transpositions. Optimized for distance = 1 (most common case).

use std::collections::HashMap;

/// Damerau-Levenshtein distance calculator
#[derive(Debug, Clone)]
pub struct DamerauLevenshtein {
    /// Matrix pool to avoid allocations (pool of 2D matrices)
    matrix_pool: Vec<Vec<Vec<usize>>>,
}

impl DamerauLevenshtein {
    /// Create a new calculator
    pub fn new() -> Self {
        Self {
            matrix_pool: Vec::new(),
        }
    }

    /// Calculate distance between two strings
    ///
    /// Returns the minimum number of operations needed to transform
    /// `s1` into `s2`, where operations are:
    /// - Insertion
    /// - Deletion
    /// - Substitution
    /// - Transposition of adjacent characters
    ///
    /// # Arguments
    /// * `s1` - First string
    /// * `s2` - Second string
    /// * `max_distance` - Early exit threshold
    ///
    /// # Returns
    /// Distance if <= max_distance, otherwise max_distance + 1
    pub fn distance(&mut self, s1: &str, s2: &str, max_distance: usize) -> usize {
        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();
        let len1 = s1_chars.len();
        let len2 = s2_chars.len();

        // Early exit for identical strings
        if len1 == len2 {
            let mut identical = true;
            for i in 0..len1 {
                if s1_chars[i] != s2_chars[i] {
                    identical = false;
                    break;
                }
            }
            if identical {
                return 0;
            }
        }

        // Early exit conditions
        if s1.is_empty() {
            return len2.min(max_distance + 1);
        }
        if s2.is_empty() {
            return len1.min(max_distance + 1);
        }

        // Length difference check
        if (len1 as i64 - len2 as i64).unsigned_abs() as usize > max_distance {
            return max_distance + 1;
        }

        // Optimized path for distance = 1
        if max_distance == 1 {
            return self.distance_one(&s1_chars, &s2_chars);
        }

        // General case with matrix
        self.distance_general(&s1_chars, &s2_chars, max_distance)
    }

    /// Optimized distance = 1 check
    #[inline]
    fn distance_one(&mut self, s1: &[char], s2: &[char]) -> usize {
        let len1 = s1.len();
        let len2 = s2.len();

        // Length difference
        if (len1 as i64 - len2 as i64).unsigned_abs() > 1 {
            return 2; // Can't be distance 1
        }

        // Same length: check for single substitution or transposition
        if len1 == len2 {
            let mut diff_count = 0;
            let mut swap_pos = None;

            for i in 0..len1 {
                if s1[i] != s2[i] {
                    diff_count += 1;
                    if diff_count > 2 {
                        return 2;
                    }
                    // Check if it's a transposition
                    if i + 1 < len1
                        && s1[i] == s2[i + 1]
                        && s1[i + 1] == s2[i]
                        && swap_pos.is_none()
                    {
                        swap_pos = Some(i);
                    }
                }
            }

            match diff_count {
                0 => 0,                       // Same
                1 => 1,                       // Single substitution
                2 if swap_pos.is_some() => 1, // Transposition
                _ => 2,
            }
        } else if len1 + 1 == len2 {
            // s1 is shorter - check for single insertion
            self.is_single_insertion(s1, s2)
        } else {
            // s2 is shorter - check for single deletion
            self.is_single_deletion(s1, s2)
        }
    }

    /// Check if s2 can be made from s1 with one insertion
    #[inline]
    fn is_single_insertion(&self, short: &[char], long: &[char]) -> usize {
        let mut i = 0;
        let mut j = 0;

        while i < short.len() && j < long.len() {
            if short[i] == long[j] {
                i += 1;
                j += 1;
            } else {
                // Skip character in long
                j += 1;
                if j - i > 1 {
                    return 2;
                }
            }
        }

        1 // One insertion
    }

    /// Check if s1 can be made from s2 with one deletion
    #[inline]
    fn is_single_deletion(&self, long: &[char], short: &[char]) -> usize {
        self.is_single_insertion(short, long)
    }

    /// General Damerau-Levenshtein with matrix
    fn distance_general(&mut self, s1: &[char], s2: &[char], max_dist: usize) -> usize {
        let len1 = s1.len();
        let len2 = s2.len();

        if (len1 + 1).saturating_mul(len2 + 1) > 1_000_000 {
            return max_dist + 1;
        }

        // Get or create matrix
        let mut matrix = self.get_matrix(len1 + 1, len2 + 1);

        // Initialize first row and column
        for (i, row) in matrix.iter_mut().enumerate().take(len1 + 1) {
            row[0] = i;
        }
        for (j, val) in matrix[0].iter_mut().enumerate().take(len2 + 1) {
            *val = j;
        }

        // Track last row where each character was seen
        let mut last: HashMap<char, usize> = HashMap::new();

        for i in 1..=len1 {
            let mut last_match = 0usize;
            let c1 = s1[i - 1];

            for j in 1..=len2 {
                let c2 = s2[j - 1];
                let last_match_tmp = last.get(&c2).copied().unwrap_or(0);

                let cost = if c1 == c2 { 0 } else { 1 };

                let deletion = matrix[i - 1][j] + 1;
                let insertion = matrix[i][j - 1] + 1;
                let substitution = matrix[i - 1][j - 1] + cost;

                let mut transposition = usize::MAX;
                if last_match_tmp != 0 && last_match != 0 {
                    transposition = matrix[last_match_tmp - 1][last_match - 1]
                        + (i - last_match_tmp - 1)
                        + 1
                        + (j - last_match - 1);
                }

                matrix[i][j] = deletion.min(insertion).min(substitution).min(transposition);

                if cost == 0 {
                    last_match = j;
                }
            }

            last.insert(c1, i);
        }

        let result = matrix[len1][len2].min(max_dist + 1);

        // Return matrix to pool for reuse (prevents allocations)
        self.return_matrix(matrix);

        result
    }

    /// Get or create matrix from pool
    fn get_matrix(&mut self, rows: usize, cols: usize) -> Vec<Vec<usize>> {
        // Try to reuse a matrix from pool
        if let Some(idx) = self
            .matrix_pool
            .iter()
            .position(|m| !m.is_empty() && m[0].len() >= cols && m.len() >= rows)
        {
            let mut m = self.matrix_pool.remove(idx);
            for row in m.iter_mut().take(rows) {
                row.resize(cols, 0);
            }
            return m;
        }

        // Create new matrix (will be returned to pool after use)
        vec![vec![0usize; cols]; rows]
    }

    /// Return matrix to pool for reuse
    fn return_matrix(&mut self, matrix: Vec<Vec<usize>>) {
        if self.matrix_pool.len() < 4 {
            // Pool max size: 4 matrices
            let mut m = matrix;
            for row in &mut m {
                row.clear();
            }
            m.clear();
            self.matrix_pool.push(m);
        }
        // If pool full, matrix is dropped (no leak, just not reused)
    }
}

impl Default for DamerauLevenshtein {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate Damerau-Levenshtein distance in one shot
///
/// Convenience function for simple use cases.
pub fn damerau_distance(s1: &str, s2: &str, max_dist: usize) -> usize {
    let mut calc = DamerauLevenshtein::new();
    calc.distance(s1, s2, max_dist)
}

/// Check if two strings are within max distance
pub fn is_within_distance(s1: &str, s2: &str, max_dist: usize) -> bool {
    damerau_distance(s1, s2, max_dist) <= max_dist
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_strings() {
        let mut calc = DamerauLevenshtein::new();
        assert_eq!(calc.distance("hello", "hello", 2), 0);
        assert_eq!(calc.distance("hola", "hola", 2), 0);
    }

    #[test]
    fn test_single_transposition() {
        let mut calc = DamerauLevenshtein::new();
        // Common typo: qeu -> que
        assert_eq!(calc.distance("qeu", "que", 1), 1);
        assert_eq!(calc.distance("hte", "the", 1), 1);
        assert_eq!(calc.distance("adn", "and", 1), 1);
        assert_eq!(calc.distance("teh", "the", 1), 1);
    }

    #[test]
    fn test_single_insertion() {
        let mut calc = DamerauLevenshtein::new();
        assert_eq!(calc.distance("hola", "holaa", 1), 1);
        assert_eq!(calc.distance("", "a", 1), 1);
    }

    #[test]
    fn test_single_deletion() {
        let mut calc = DamerauLevenshtein::new();
        assert_eq!(calc.distance("holaa", "hola", 1), 1);
        assert_eq!(calc.distance("a", "", 1), 1);
    }

    #[test]
    fn test_single_substitution() {
        let mut calc = DamerauLevenshtein::new();
        assert_eq!(calc.distance("hola", "holb", 1), 1);
        assert_eq!(calc.distance("cat", "bat", 1), 1);
    }

    #[test]
    fn test_distance_two() {
        let mut calc = DamerauLevenshtein::new();
        assert_eq!(calc.distance("cat", "dog", 2), 3);
        assert_eq!(calc.distance("hello", "helo", 2), 1);
    }

    #[test]
    fn test_early_exit() {
        let mut calc = DamerauLevenshtein::new();
        // Should early exit when distance exceeds threshold
        assert_eq!(calc.distance("abc", "xyz", 1), 2); // 3, but capped at 2
    }

    #[test]
    fn test_empty_strings() {
        let mut calc = DamerauLevenshtein::new();
        assert_eq!(calc.distance("", "", 2), 0);
        assert_eq!(calc.distance("abc", "", 2), 3);
        assert_eq!(calc.distance("", "abc", 2), 3);
    }

    #[test]
    fn test_unicode() {
        let mut calc = DamerauLevenshtein::new();
        assert_eq!(calc.distance("café", "café", 1), 0);
        assert_eq!(calc.distance("naïve", "naive", 1), 1);
        assert_eq!(calc.distance("日本語", "日本", 1), 1);
    }

    #[test]
    fn test_emoji() {
        let mut calc = DamerauLevenshtein::new();
        assert_eq!(calc.distance("😀", "😀", 1), 0);
        assert_eq!(calc.distance("😀😁", "😀", 1), 1);
    }

    #[test]
    fn test_case_sensitivity() {
        let mut calc = DamerauLevenshtein::new();
        assert_eq!(calc.distance("Hello", "hello", 1), 1); // Case is considered substitution
    }

    #[test]
    fn test_multiple_transpositions() {
        let mut calc = DamerauLevenshtein::new();
        // caab -> abca has multiple ops
        assert_eq!(calc.distance("caab", "abca", 2), 3);
    }

    #[test]
    fn test_convenience_functions() {
        assert!(is_within_distance("qeu", "que", 1));
        assert!(!is_within_distance("qeu", "casa", 1));
        assert_eq!(damerau_distance("test", "test", 2), 0);
    }

    #[test]
    fn test_special_chars() {
        let mut calc = DamerauLevenshtein::new();
        assert_eq!(calc.distance("hello!", "hello", 1), 1);
        assert_eq!(calc.distance("hello!", "hello!", 1), 0);
        assert_eq!(calc.distance("don't", "dont", 1), 1);
    }

    #[test]
    fn test_whitespace() {
        let mut calc = DamerauLevenshtein::new();
        assert_eq!(calc.distance("hello world", "helloworld", 1), 1);
        assert_eq!(calc.distance("hello world", "hello  world", 1), 1);
    }

    #[test]
    fn test_length_difference_limit() {
        let mut calc = DamerauLevenshtein::new();
        // "abc" and "abcdef" can't be distance 1
        assert_eq!(calc.distance("abc", "abcdef", 1), 2);
        assert_eq!(calc.distance("abcdef", "abc", 1), 2);
    }
}
