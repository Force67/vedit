//! Boyer-Moore search implementation for document text searching

/// Boyer-Moore searcher for efficient string searching
pub struct BoyerMooreSearcher {
    pattern: Vec<u8>,
    bc_skips: [usize; 256], // Bad character table
    gs_skips: Vec<usize>,   // Good suffix table
}

impl BoyerMooreSearcher {
    /// Create a new Boyer-Moore searcher for the given pattern
    pub fn new(pattern: &[u8]) -> Self {
        let pattern_len = pattern.len();

        // Initialize bad character table with pattern length (worst case)
        let mut bc_skips = [pattern_len; 256];

        // Build bad character table
        for (i, &byte) in pattern.iter().enumerate() {
            if i < pattern_len - 1 {
                bc_skips[byte as usize] = pattern_len - 1 - i;
            }
        }

        // Build good suffix table
        let gs_skips = Self::build_good_suffix_table(pattern);

        Self {
            pattern: pattern.to_vec(),
            bc_skips,
            gs_skips,
        }
    }

    /// Build the good suffix table for the pattern
    fn build_good_suffix_table(pattern: &[u8]) -> Vec<usize> {
        let pattern_len = pattern.len();
        let mut gs_skips = vec![0; pattern_len];

        if pattern_len == 1 {
            return gs_skips;
        }

        // Find suffixes and build the good suffix table
        let mut suffixes = vec![0; pattern_len];
        Self::find_suffixes(pattern, &mut suffixes);

        // Initialize with pattern length (worst case)
        for i in 0..pattern_len {
            gs_skips[i] = pattern_len;
        }

        // Fill good suffix table
        let mut j = 0;
        for i in (0..pattern_len - 1).rev() {
            if suffixes[i] == i + 1 {
                while j < pattern_len - 1 - i {
                    if gs_skips[j] == pattern_len {
                        gs_skips[j] = pattern_len - 1 - i;
                    }
                    j += 1;
                }
            }
        }

        for i in 0..pattern_len - 1 {
            gs_skips[pattern_len - 1 - suffixes[i]] = pattern_len - 1 - i;
        }

        gs_skips
    }

    /// Find suffixes for good suffix table
    fn find_suffixes(pattern: &[u8], suffixes: &mut [usize]) {
        let pattern_len = pattern.len();
        suffixes[pattern_len - 1] = pattern_len;
        let mut g = pattern_len - 1;
        let mut f = 0;

        for i in (0..pattern_len - 1).rev() {
            if i > g && suffixes[i + pattern_len - 1 - f] < i - g {
                suffixes[i] = suffixes[i + pattern_len - 1 - f];
            } else {
                if i < g {
                    g = i;
                }
                f = i;

                while g > 0 && pattern[g] == pattern[g + pattern_len - 1 - f] {
                    g -= 1;
                }
                suffixes[i] = f - g;
            }
        }
    }

    /// Search for the pattern in the given text
    /// Returns a vector of starting indices where the pattern was found
    pub fn find_all(&self, text: &[u8]) -> Vec<usize> {
        let pattern_len = self.pattern.len();
        let text_len = text.len();
        let mut matches = Vec::new();

        if pattern_len == 0 || pattern_len > text_len {
            return matches;
        }

        let mut i = 0;
        while i <= text_len - pattern_len {
            let mut j = pattern_len - 1;

            // Compare from right to left
            while j > 0 && text[i + j] == self.pattern[j] {
                j -= 1;
            }

            if j == 0 && text[i] == self.pattern[0] {
                // Match found
                matches.push(i);
                i += 1; // Move to next position to find overlapping matches
            } else {
                // No match, skip ahead using appropriate table
                let skip_char = text[i + j];
                let bc_skip = self.bc_skips[skip_char as usize];
                let gs_skip = self.gs_skips[j];

                i += std::cmp::max(1, std::cmp::max(bc_skip, gs_skip));
            }
        }

        matches
    }

    /// Find the first occurrence of the pattern
    /// Returns the starting index or None if not found
    pub fn find_first(&self, text: &[u8]) -> Option<usize> {
        self.find_all(text).first().copied()
    }

    /// Check if the pattern exists in the text
    pub fn contains(&self, text: &[u8]) -> bool {
        self.find_first(text).is_some()
    }
}

/// Convenience function to search for a pattern in text
pub fn search_pattern(text: &str, pattern: &str) -> Vec<usize> {
    if pattern.is_empty() {
        return Vec::new();
    }

    let searcher = BoyerMooreSearcher::new(pattern.as_bytes());
    searcher.find_all(text.as_bytes())
}

/// Convenience function to find first occurrence of a pattern
pub fn find_pattern(text: &str, pattern: &str) -> Option<usize> {
    if pattern.is_empty() {
        return None;
    }

    let searcher = BoyerMooreSearcher::new(pattern.as_bytes());
    searcher.find_first(text.as_bytes())
}

/// Convenience function to check if pattern exists in text
pub fn contains_pattern(text: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }

    let searcher = BoyerMooreSearcher::new(pattern.as_bytes());
    searcher.contains(text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_search() {
        let text = "hello world hello universe";
        let pattern = "hello";

        let matches = search_pattern(text, pattern);
        assert_eq!(matches, vec![0, 12]);
    }

    #[test]
    fn test_single_char_search() {
        let text = "abracadabra";
        let pattern = "a";

        let matches = search_pattern(text, pattern);
        assert_eq!(matches, vec![0, 3, 5, 7, 10]);
    }

    #[test]
    fn test_no_match() {
        let text = "hello world";
        let pattern = "xyz";

        let matches = search_pattern(text, pattern);
        assert_eq!(matches, Vec::<usize>::new());
    }

    #[test]
    fn test_empty_pattern() {
        let text = "hello world";
        let pattern = "";

        let matches = search_pattern(text, pattern);
        assert_eq!(matches, Vec::<usize>::new());
    }

    #[test]
    fn test_pattern_longer_than_text() {
        let text = "hello";
        let pattern = "hello world";

        let matches = search_pattern(text, pattern);
        assert_eq!(matches, Vec::<usize>::new());
    }

    #[test]
    fn test_overlapping_matches() {
        let text = "aaaa";
        let pattern = "aa";

        let matches = search_pattern(text, pattern);
        assert_eq!(matches, vec![0, 1, 2]);
    }

    #[test]
    fn test_contains() {
        assert!(contains_pattern("hello world", "world"));
        assert!(!contains_pattern("hello world", "xyz"));
    }

    #[test]
    fn test_find_first() {
        assert_eq!(find_pattern("hello world hello", "hello"), Some(0));
        assert_eq!(find_pattern("hello world hello", "world"), Some(6));
        assert_eq!(find_pattern("hello world", "xyz"), None);
    }

    #[test]
    fn test_unicode_search() {
        let text = "héllo wörld héllo";
        let pattern = "héllo";

        let matches = search_pattern(text, pattern);
        // Unicode characters count as 4 bytes each for é and ö, so positions differ
        assert_eq!(matches, vec![0, 14]);
    }
}
