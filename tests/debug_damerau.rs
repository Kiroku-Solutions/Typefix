fn damerau_distance(s1: &str, s2: &str, max_dist: usize) -> usize {
    if s1.is_empty() { return s2.chars().count().min(max_dist + 1); }
    if s2.is_empty() { return s1.chars().count().min(max_dist + 1); }

    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();
    let len1 = s1_chars.len();
    let len2 = s2_chars.len();

    if (len1 as i64 - len2 as i64).unsigned_abs() as usize > max_dist {
        return max_dist + 1;
    }

    let mut d = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 { d[i][0] = i; }
    for j in 0..=len2 { d[0][j] = j; }

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if s1_chars[i - 1] == s2_chars[j - 1] { 0 } else { 1 };

            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);

            if i > 1 && j > 1 && s1_chars[i - 1] == s2_chars[j - 2] && s1_chars[i - 2] == s2_chars[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + cost);
            }
        }
    }

    d[len1][len2]
}

#[test]
fn test_damerau() {
    let dist = damerau_distance("qeu", "que", 1);
    println!("DAMERAU DISTANCE FOR QEU -> QUE IS: {}", dist);
    let dist_qu = damerau_distance("qeu", "qu", 1);
    println!("DAMERAU DISTANCE FOR QEU -> QU IS: {}", dist_qu);
}
