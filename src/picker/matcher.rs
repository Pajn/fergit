use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// An item that survived the current query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranked {
    /// Position in the original item list.
    pub index: usize,
    pub score: u32,
    /// Character positions in the label that the query matched, for
    /// highlighting.
    pub highlights: Vec<u32>,
}

/// Scores labels against a query.
pub struct Ranker {
    matcher: Matcher,
    haystack: Vec<char>,
    highlights: Vec<u32>,
}

impl Ranker {
    pub fn new() -> Self {
        // Paths are the common case, and this weights separators accordingly.
        Ranker {
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
            haystack: Vec::new(),
            highlights: Vec::new(),
        }
    }

    /// Rank `labels`, best first. An empty query keeps every item in the order
    /// it was given, which is the order the repository reported.
    pub fn rank(&mut self, query: &str, labels: &[String]) -> Vec<Ranked> {
        if query.is_empty() {
            return labels
                .iter()
                .enumerate()
                .map(|(index, _)| Ranked {
                    index,
                    score: 0,
                    highlights: Vec::new(),
                })
                .collect();
        }

        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let mut ranked: Vec<Ranked> = labels
            .iter()
            .enumerate()
            .filter_map(|(index, label)| {
                self.haystack.clear();
                self.highlights.clear();
                let text = Utf32Str::new(label, &mut self.haystack);
                let score = pattern.indices(text, &mut self.matcher, &mut self.highlights)?;
                self.highlights.sort_unstable();
                self.highlights.dedup();
                Some(Ranked {
                    index,
                    score,
                    highlights: self.highlights.clone(),
                })
            })
            .collect();

        // Equal scores keep their original order, so the list does not shuffle
        // as you type.
        ranked.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.index.cmp(&b.index)));
        ranked
    }
}

impl Default for Ranker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn an_empty_query_keeps_every_item_in_order() {
        let mut ranker = Ranker::new();
        let ranked = ranker.rank("", &labels(&["b", "a", "c"]));
        assert_eq!(
            ranked.iter().map(|r| r.index).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn drops_items_that_do_not_match() {
        let mut ranker = Ranker::new();
        let ranked = ranker.rank("zzz", &labels(&["src/main.rs", "README.md"]));
        assert!(ranked.is_empty());
    }

    #[test]
    fn matches_across_gaps_and_reports_where() {
        let mut ranker = Ranker::new();
        let ranked = ranker.rank("mrs", &labels(&["src/main.rs"]));
        assert_eq!(ranked.len(), 1);
        assert!(!ranked[0].highlights.is_empty());
        let matched: String = "src/main.rs"
            .chars()
            .enumerate()
            .filter(|(i, _)| ranked[0].highlights.contains(&(*i as u32)))
            .map(|(_, c)| c)
            .collect();
        assert_eq!(matched, "mrs");
    }

    #[test]
    fn ranks_the_closer_match_first() {
        let mut ranker = Ranker::new();
        let ranked = ranker.rank("main", &labels(&["src/domain/other.rs", "src/main.rs"]));
        assert_eq!(ranked.first().map(|r| r.index), Some(1));
    }
}
