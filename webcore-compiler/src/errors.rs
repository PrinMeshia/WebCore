//! Utility functions for error reporting

/// Find the closest match using Levenshtein distance (threshold: 3 edits).
#[cfg(test)]
pub fn find_closest_match<'a>(
    needle: &str,
    haystack: impl Iterator<Item = &'a str>,
) -> Option<String> {
    let mut best_match = None;
    let mut best_distance = usize::MAX;

    for candidate in haystack {
        let distance = levenshtein_distance(needle, candidate);
        if distance < best_distance && distance <= 3 {
            best_distance = distance;
            best_match = Some(candidate.to_string());
        }
    }

    best_match
}

/// Calculate Levenshtein distance between two strings.
#[cfg(test)]
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for (i, row) in matrix.iter_mut().enumerate().take(a_len + 1) {
        row[0] = i;
    }
    for (j, cell) in matrix[0].iter_mut().enumerate().take(b_len + 1) {
        *cell = j;
    }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = usize::from(a_chars[i - 1] != b_chars[j - 1]);
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a_len][b_len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("Button", "Button"), 0);
        assert_eq!(levenshtein_distance("Buton", "Button"), 1);
        assert_eq!(levenshtein_distance("Btton", "Button"), 1);
        assert_eq!(levenshtein_distance("abc", "xyz"), 3);
    }

    #[test]
    fn test_find_closest_match() {
        let candidates = vec!["Button", "Card", "Input", "Modal"];
        let result = find_closest_match("Buton", candidates.iter().map(|s| *s));
        assert_eq!(result, Some("Button".to_string()));
    }

    #[test]
    fn test_find_closest_no_match() {
        let candidates = vec!["Button", "Card"];
        let result = find_closest_match("XYZ", candidates.iter().map(|s| *s));
        assert_eq!(result, None);
    }
}
