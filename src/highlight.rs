//! Query-term highlighting for export formats (md/html/rtf/docx).
//!
//! A `Hl` instance is the set of case-folded query terms to mark in the
//! exported text. Writers mark the matched ranges with their own markup:
//! `<mark>` (md), `<mark class="hl">` (html), `\highlight2` (rtf),
//! `<w:highlight w:val="yellow"/>` (docx). An empty `Hl` = highlighting off
//! (the hot path: `is_empty()` is checked first everywhere).

/// Case-folded query terms. Build with [`Hl::from_query`]; empty = off.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hl {
    /// lowercased terms, longest-first (longest wins at equal starts)
    terms: Vec<Vec<char>>,
}

impl Hl {
    /// Build the highlighter from a raw query: whitespace-split, lowercase,
    /// drop 1-char tokens (they mark too much), dedup.
    pub fn from_query(q: &str) -> Hl {
        let mut terms: Vec<String> = Vec::new();
        for w in q.split_whitespace() {
            let t: String = w.to_lowercase();
            if t.chars().count() >= 2 && !terms.contains(&t) {
                terms.push(t);
            }
        }
        terms.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()));
        let terms = terms
            .into_iter()
            .map(|t| t.chars().collect::<Vec<char>>())
            .collect();
        Hl { terms }
    }

    /// No terms — the writer skips all highlight work.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Non-overlapping case-insensitive match ranges (in CHARs, not bytes),
    /// left-to-right: a match consumes its range (the scan resumes after
    /// it); at equal starts the longest term wins.
    pub fn matches(&self, s: &str) -> Vec<(usize, usize)> {
        if self.terms.is_empty() {
            return Vec::new();
        }
        // one lowercased copy + the char-start -> lowered-position map
        // (to_lowercase can change char counts, e.g. U+0130)
        let mut lc: Vec<char> = Vec::new();
        let mut map: Vec<usize> = Vec::new();
        for ch in s.chars() {
            map.push(lc.len());
            for c in ch.to_lowercase() {
                lc.push(c);
            }
        }
        let total = s.chars().count();
        let mut out: Vec<(usize, usize)> = Vec::new();
        let mut i = 0usize;
        while i < total {
            let li = map[i];
            let mut best: Option<usize> = None; // end char index
            for t in &self.terms {
                if t[0] == lc[li] && lc[li..].starts_with(t.as_slice()) {
                    let end = i + t.len();
                    if best.map_or(true, |e| end > e) {
                        best = Some(end);
                    }
                }
            }
            if let Some(end) = best {
                out.push((i, end));
                i = end;
            } else {
                i += 1;
            }
        }
        out
    }

    /// A PARTITION of `s` into `(piece, is_highlight)` pairs: the pieces
    /// concatenate to `s` exactly, matched ranges are the highlight pieces.
    /// With no matches: one `(s, false)` entry.
    pub fn segments<'a>(&self, s: &'a str) -> Vec<(&'a str, bool)> {
        let m = self.matches(s);
        if m.is_empty() {
            return vec![(s, false)];
        }
        // char -> byte bounds (match ranges are in chars)
        let mut b: Vec<usize> = Vec::with_capacity(s.chars().count() + 1);
        for (o, _) in s.char_indices() {
            b.push(o);
        }
        b.push(s.len());
        let mut out: Vec<(&str, bool)> = Vec::new();
        let mut prev = 0usize;
        for (a, e) in m {
            let a0 = b[a];
            let e0 = b[e];
            if a0 > prev {
                out.push((&s[prev..a0], false));
            }
            out.push((&s[a0..e0], true));
            prev = e0;
        }
        if prev < s.len() {
            out.push((&s[prev..], false));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_query_drops_single_chars_and_folds() {
        let h = Hl::from_query(" Закон УКРАЇНИ а ");
        assert!(!h.is_empty());
        // 1-char «а» is dropped; both longer terms are folded to lowercase
        assert_eq!(h.terms.len(), 2);
        assert!(h.terms.iter().any(|t| t == "закон".chars().collect::<Vec<char>>().as_slice()));
        assert!(h
            .terms
            .iter()
            .any(|t| t == "україни".chars().collect::<Vec<char>>().as_slice()));
    }

    #[test]
    fn matches_case_insensitive_and_non_overlapping() {
        let h = Hl::from_query("закон");
        let s = "Закон України. закон і ЗАКОН";
        let m = h.matches(s);
        assert_eq!(m.len(), 3);
        assert_eq!(m[0], (0, 5));
    }

    #[test]
    fn matches_longest_wins_and_multi_terms() {
        let h = Hl::from_query("закон закони");
        let s = "закони і закон";
        let m = h.matches(s);
        // "закони" (longest) first, then "закон" after it
        assert_eq!(m.len(), 2);
        assert_eq!(m[0], (0, 6));
        assert_eq!(m[1], (9, 14));
    }

    #[test]
    fn segments_cover_the_string() {
        let h = Hl::from_query("закон");
        let s = "Закон і закон";
        let segs = h.segments(s);
        // partition: pieces concatenate to s exactly
        let joined: String = segs.iter().map(|(p, _)| p.to_string()).collect();
        assert_eq!(joined, s);
        // exactly the two matches are the highlight pieces
        let matched: String = segs
            .iter()
            .filter(|(_, is_hl)| *is_hl)
            .map(|(p, _)| p.to_string())
            .collect();
        assert_eq!(matched, "Законзакон");
    }

    #[test]
    fn empty_hl_no_matches() {
        let h = Hl::default();
        assert!(h.is_empty());
        assert!(h.matches("any text").is_empty());
        assert_eq!(h.segments("abc"), vec![("abc", false)]);
    }

    #[test]
    fn no_match_gives_single_segment() {
        let h = Hl::from_query("немає");
        let segs = h.segments("звичайний текст");
        assert_eq!(segs, vec![("звичайний текст", false)]);
    }
}
