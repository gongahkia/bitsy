// Fuzzy matching algorithm (fzf-style)

/// Result of a fuzzy match
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    /// The matched item
    pub item: String,
    /// Match score (higher is better)
    pub score: i32,
    /// Indices of matched characters in the item
    pub matched_indices: Vec<usize>,
}

/// Fuzzy matcher with fzf-style scoring
#[derive(Debug)]
pub struct FuzzyMatcher {
    case_sensitive: bool,
}

impl FuzzyMatcher {
    pub fn new(case_sensitive: bool) -> Self {
        Self { case_sensitive }
    }

    /// Match a pattern against a candidate string
    /// Returns Some(FuzzyMatch) if the pattern matches, None otherwise
    pub fn fuzzy_match(&self, pattern: &str, candidate: &str) -> Option<FuzzyMatch> {
        if pattern.is_empty() {
            return Some(FuzzyMatch {
                item: candidate.to_string(),
                score: 0,
                matched_indices: vec![],
            });
        }

        let pattern_chars: Vec<char> = if self.case_sensitive {
            pattern.chars().collect()
        } else {
            pattern.to_lowercase().chars().collect()
        };

        let candidate_chars: Vec<char> = candidate.chars().collect();
        let candidate_lower: Vec<char> = if self.case_sensitive {
            candidate_chars.clone()
        } else {
            candidate.to_lowercase().chars().collect()
        };

        // Find all matching positions
        let mut matched_indices = Vec::new();
        let mut pattern_idx = 0;

        for (i, &ch) in candidate_lower.iter().enumerate() {
            if pattern_idx < pattern_chars.len() && ch == pattern_chars[pattern_idx] {
                matched_indices.push(i);
                pattern_idx += 1;
            }
        }

        // Check if all pattern characters were matched
        if pattern_idx != pattern_chars.len() {
            return None;
        }

        // Calculate score
        let score = self.calculate_score(&candidate_chars, &matched_indices);

        Some(FuzzyMatch {
            item: candidate.to_string(),
            score,
            matched_indices,
        })
    }

    /// Calculate match score using fzf-style heuristics
    fn calculate_score(&self, candidate: &[char], matched_indices: &[usize]) -> i32 {
        if matched_indices.is_empty() {
            return 0;
        }

        let mut score: i32 = 0;

        // Base score for each matched character
        score += (matched_indices.len() * 16) as i32;

        // Bonus for matches at the start
        if matched_indices[0] == 0 {
            score += 32;
        }

        // Bonus for consecutive matches
        let mut prev_idx: Option<usize> = None;
        for &idx in matched_indices {
            if let Some(prev) = prev_idx {
                if idx == prev + 1 {
                    // Consecutive match bonus
                    score += 24;
                } else {
                    // Gap penalty
                    let gap = (idx - prev - 1) as i32;
                    score -= gap.min(8);
                }
            }
            prev_idx = Some(idx);
        }

        // Bonus for word boundary matches (after /, _, -, ., or space, or uppercase after lowercase)
        for &idx in matched_indices {
            if idx == 0 {
                continue;
            }
            let prev_char = candidate[idx - 1];
            let curr_char = candidate[idx];

            // Check for word boundary
            if prev_char == '/' || prev_char == '_' || prev_char == '-'
                || prev_char == '.' || prev_char == ' ' {
                score += 16;
            }
            // CamelCase boundary
            else if prev_char.is_lowercase() && curr_char.is_uppercase() {
                score += 16;
            }
        }

        // Bonus for shorter candidates (prefer shorter matches)
        let length_penalty = (candidate.len() as i32 - matched_indices.len() as i32) / 4;
        score -= length_penalty;

        score
    }

    /// Match pattern against multiple candidates and return sorted results
    pub fn fuzzy_match_all(&self, pattern: &str, candidates: &[String]) -> Vec<FuzzyMatch> {
        let mut matches: Vec<FuzzyMatch> = candidates
            .iter()
            .filter_map(|c| self.fuzzy_match(pattern, c))
            .collect();

        // Sort by score (highest first)
        matches.sort_by(|a, b| b.score.cmp(&a.score));
        matches
    }
}

impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self::new(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_pattern() {
        let matcher = FuzzyMatcher::new(false);
        let result = matcher.fuzzy_match("", "hello");
        assert!(result.is_some());
        assert_eq!(result.unwrap().score, 0);
    }

    #[test]
    fn test_exact_match() {
        let matcher = FuzzyMatcher::new(false);
        let result = matcher.fuzzy_match("hello", "hello");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.matched_indices, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_subsequence_match() {
        let matcher = FuzzyMatcher::new(false);
        let result = matcher.fuzzy_match("hlo", "hello");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.matched_indices, vec![0, 2, 4]);
    }

    #[test]
    fn test_no_match() {
        let matcher = FuzzyMatcher::new(false);
        let result = matcher.fuzzy_match("xyz", "hello");
        assert!(result.is_none());
    }

    #[test]
    fn test_case_insensitive() {
        let matcher = FuzzyMatcher::new(false);
        let result = matcher.fuzzy_match("HeLLo", "hello");
        assert!(result.is_some());
    }

    #[test]
    fn test_case_sensitive() {
        let matcher = FuzzyMatcher::new(true);
        let result = matcher.fuzzy_match("HeLLo", "hello");
        assert!(result.is_none());
    }

    #[test]
    fn test_word_boundary_bonus() {
        let matcher = FuzzyMatcher::new(false);
        // "src" should score higher in "src/main.rs" than "asrc"
        let result1 = matcher.fuzzy_match("src", "src/main.rs");
        let result2 = matcher.fuzzy_match("src", "asrc");
        assert!(result1.is_some());
        assert!(result2.is_some());
        assert!(result1.unwrap().score > result2.unwrap().score);
    }

    #[test]
    fn test_consecutive_bonus() {
        let matcher = FuzzyMatcher::new(false);
        // Consecutive matches should score higher
        let result1 = matcher.fuzzy_match("ab", "ab");
        let result2 = matcher.fuzzy_match("ab", "a_b");
        assert!(result1.is_some());
        assert!(result2.is_some());
        assert!(result1.unwrap().score > result2.unwrap().score);
    }

    #[test]
    fn test_match_all_sorted() {
        let matcher = FuzzyMatcher::new(false);
        let candidates = vec![
            "src/main.rs".to_string(),
            "some_random_file.txt".to_string(),
            "main.rs".to_string(),
        ];
        let results = matcher.fuzzy_match_all("main", &candidates);
        assert_eq!(results.len(), 2);
        // main.rs should score higher than src/main.rs due to length
        assert_eq!(results[0].item, "main.rs");
    }

    #[test]
    fn test_camel_case_bonus() {
        let matcher = FuzzyMatcher::new(false);
        let result = matcher.fuzzy_match("fm", "FuzzyMatcher");
        assert!(result.is_some());
        // Should match at word boundaries
        let m = result.unwrap();
        assert!(m.score > 0);
    }

    #[test]
    fn test_path_matching() {
        let matcher = FuzzyMatcher::new(false);
        let result = matcher.fuzzy_match("buf", "src/buffer.rs");
        assert!(result.is_some());
    }
}
