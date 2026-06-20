fn damerau_distance(s1: &str, s2: &str, max_dist: usize) -> usize {
    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();
    let len1 = s1_chars.len();
    let len2 = s2_chars.len();

    let mut matrix = vec![vec![0usize; len2 + 1]; len1 + 1];

    for (i, row) in matrix.iter_mut().enumerate().take(len1 + 1) {
        row[0] = i;
    }
    for (j, val) in matrix[0].iter_mut().enumerate().take(len2 + 1) {
        *val = j;
    }

    let mut last_row: std::collections::HashMap<char, usize> = std::collections::HashMap::new();

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
    matrix[len1][len2]
}

fn main() {
    println!("hwo -> how: {}", damerau_distance("hwo", "how", 3));
    println!("hwo -> who: {}", damerau_distance("hwo", "who", 3));
    println!("hwo -> cho: {}", damerau_distance("hwo", "cho", 3));
}
