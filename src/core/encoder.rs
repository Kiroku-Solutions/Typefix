/// Encode Latin accented characters into single-byte uppercase ASCII characters.
/// This allows the FST Levenshtein automaton (which operates on bytes) to treat
/// an accent difference as a single byte substitution rather than a multi-byte
/// insertion/substitution, effectively preventing a missing accent from costing
/// 2 or more edit distances.
pub fn encode_accents(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    for c in s.chars() {
        let mapped = match c {
            'á' => 'A',
            'é' => 'E',
            'í' => 'I',
            'ó' => 'O',
            'ú' => 'U',
            'ñ' => 'N',
            'ü' => 'W',
            'ã' => 'B',
            'õ' => 'C',
            'ç' => 'D',
            'â' => 'F',
            'ê' => 'G',
            'ô' => 'J',
            'à' => 'L',
            _ => c,
        };
        res.push(mapped);
    }
    res
}

/// Decode single-byte uppercase ASCII characters back into their original
/// Latin accented characters for user-facing output.
pub fn decode_accents(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    for c in s.chars() {
        let mapped = match c {
            'A' => 'á',
            'E' => 'é',
            'I' => 'í',
            'O' => 'ó',
            'U' => 'ú',
            'N' => 'ñ',
            'W' => 'ü',
            'B' => 'ã',
            'C' => 'õ',
            'D' => 'ç',
            'F' => 'â',
            'G' => 'ê',
            'J' => 'ô',
            'L' => 'à',
            _ => c,
        };
        res.push(mapped);
    }
    res
}

/// Strip Latin accented characters into their unaccented ASCII equivalents.
pub fn strip_accents(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    for c in s.chars() {
        let mapped = match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' => 'u',
            'ñ' => 'n',
            'ü' => 'u',
            'ã' => 'a',
            'õ' => 'o',
            'ç' => 'c',
            'â' => 'a',
            'ê' => 'e',
            'ô' => 'o',
            'à' => 'a',
            _ => c,
        };
        res.push(mapped);
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoding_decoding() {
        let original = "investigación";
        let encoded = encode_accents(original);
        assert_eq!(encoded, "investigaciOn");
        let decoded = decode_accents(&encoded);
        assert_eq!(decoded, original);
    }
}
