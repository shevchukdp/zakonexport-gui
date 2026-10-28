//! v0.23.0: the doc-number join — the native u16 postings vs the
//! current base docnums (the roadmap payoff item).
//!
//! **Empirical findings (this base):**
//! - the `.vt` u16 posting stream spans a 2005-era document space
//!   (the full `lmur.vt` dense stream: values up to 65535, ~46k
//!   distinct ids, many in the 20000–26000 range; the clean
//!   per-record u16s start above zero at 257);
//! - the current base the `fields <docnum>` verb renders is the
//!   docloc.db-based base (`docloc.db` 3065877 B → 613173 docs,
//!   `doc\doc.mb` container) — **not** the legacy `text\*.mb` FTS
//!   containers (2363 files, the ftsearch oracle's input).
//!
//! Because the u16 space (≤ 65535) fits numerically inside the
//! current 613173-doc range, `DocJoin::probe` classifies this base
//! as **Identity**: every u16 id is a candidate current docnum.
//! What ships is the probe (`DocJoin::probe` — the u16 space stats
//! + the current doc count + the `JoinKind` classification) and the
//! **openable subset** (`DocJoin::openable` — the u16s within
//! `[1..=doc_count]`). Whether those u16 ids *are* the docloc
//! docnums (vs a shared-but-offset numbering) is the v0.24.0
//! oracle cross-check (tsearch vs ftsearch); the 2005 → current
//! mapping-table hunt (`.m` 0x29D7 section) is only needed if the
//! probe ever reports `Bounded`.

use std::collections::HashSet;

/// The classified join between the native u16 postings and the
/// current base docnums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    /// `max u16 <= doc_count` — the u16s are candidate current
    /// docnums (the cheapest outcome; the case in this base:
    /// 65535 ≤ 613173).
    Identity,
    /// `max u16 > doc_count` — the u16s span a space larger than the
    /// current docnums; only the subset within `[1..=doc_count]` is
    /// openable via the current base.
    Bounded,
    /// No u16 stream or doc count was available to probe against.
    Unknown,
}

/// The doc-number join probe result.
#[derive(Debug, Clone)]
pub struct DocJoin {
    /// The minimum u16 in the posting stream (0 when empty).
    pub min_u16: u16,
    /// The maximum u16 in the posting stream (0 when empty).
    pub max_u16: u16,
    /// The count of distinct u16 ids in the stream.
    pub distinct_u16: usize,
    /// The total u16 count of the stream.
    pub total_u16: usize,
    /// The current base doc count (0 when the base could not load).
    pub doc_count: u32,
    /// The classified join kind.
    pub kind: JoinKind,
}

impl DocJoin {
    /// Probe the join kind from the full `.vt` u16 stream
    /// (`TridentIndex.vt_u16`) + the current base doc count
    /// (`render::Base::doc_count`).
    pub fn probe(vt_u16: &[u16], doc_count: u32) -> DocJoin {
        if vt_u16.is_empty() || doc_count == 0 {
            return DocJoin {
                min_u16: 0,
                max_u16: 0,
                distinct_u16: 0,
                total_u16: vt_u16.len(),
                doc_count,
                kind: JoinKind::Unknown,
            };
        }
        let mut min_u16 = u16::MAX;
        let mut max_u16 = 0u16;
        let mut set = HashSet::new();
        for &v in vt_u16 {
            if v < min_u16 {
                min_u16 = v;
            }
            if v > max_u16 {
                max_u16 = v;
            }
            set.insert(v);
        }
        let kind = if (max_u16 as u32) <= doc_count {
            JoinKind::Identity
        } else {
            JoinKind::Bounded
        };
        DocJoin {
            min_u16,
            max_u16,
            distinct_u16: set.len(),
            total_u16: vt_u16.len(),
            doc_count,
            kind,
        }
    }

    /// The openable subset of a posting list: the u16s within
    /// `[1..=doc_count]` — the ids `fields <docnum>` can open in the
    /// current base (when the numbering is shared; the full 2005 →
    /// current mapping is the follow-up RE step).
    pub fn openable(&self, u16s: &[u16]) -> Vec<u16> {
        u16s.iter()
            .copied()
            .filter(|v| *v >= 1 && (*v as u32) <= self.doc_count)
            .collect()
    }

    /// True when every u16 in the stream is a current docnum
    /// (the identity outcome).
    pub fn is_identity(&self) -> bool {
        self.kind == JoinKind::Identity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_classifies_identity_when_the_u16_space_fits() {
        let j = DocJoin::probe(&[1, 2, 3, 5, 5], 10);
        assert_eq!(j.kind, JoinKind::Identity);
        assert_eq!(j.min_u16, 1);
        assert_eq!(j.max_u16, 5);
        assert_eq!(j.distinct_u16, 4);
        assert_eq!(j.total_u16, 5);
        assert!(j.is_identity());
        // the openable subset is the whole list (all within 1..=10)
        assert_eq!(j.openable(&[1, 2, 3, 5]), vec![1, 2, 3, 5]);
    }

    #[test]
    fn probe_classifies_bounded_when_the_u16_space_exceeds_docs() {
        let j = DocJoin::probe(&[1, 2363, 2364, 65535], 2363);
        assert_eq!(j.kind, JoinKind::Bounded);
        assert_eq!(j.max_u16, 65535);
        // boundary behavior: 2363 is openable, 2364 is not
        assert_eq!(j.openable(&[1, 2363, 2364, 65535]), vec![1, 2363]);
    }

    #[test]
    fn probe_is_unknown_without_a_stream_or_doc_count() {
        assert_eq!(DocJoin::probe(&[], 2363).kind, JoinKind::Unknown);
        assert_eq!(DocJoin::probe(&[1], 0).kind, JoinKind::Unknown);
        // the openable subset is empty when no doc count is known
        let j = DocJoin::probe(&[1], 0);
        assert!(j.openable(&[1]).is_empty());
    }

    #[test]
    fn openable_drops_zero_and_out_of_range() {
        let j = DocJoin::probe(&[1], 100);
        assert_eq!(
            j.openable(&[0, 1, 100, 101, 65535]),
            vec![1, 100],
            "0 is not a docnum; >100 is out of the current range"
        );
    }

    #[test]
    fn probe_the_real_lmur_vt_stream() {
        let Some(idx) = crate::trident::TridentIndex::open(std::path::Path::new(
            r"C:\Zakon.Local\Baza",
        )) else {
            eprintln!("skipped: no base");
            return;
        };
        // the join target is the current docloc-based base (what
        // `fields <docnum>` renders), not the legacy text\*.mb set
        let doc_count = crate::render::Base::load(std::path::Path::new(
            r"C:\Zakon.Local\Baza",
        ))
        .ok()
        .map(|b| b.doc_count())
        .unwrap_or(0);
        if doc_count == 0 {
            eprintln!("skipped: no current base");
            return;
        }
        let j = DocJoin::probe(&idx.vt_u16, doc_count);
        // the 2005-era u16 space (max 65535) fits inside the current
        // docnum range -> the probe reports the identity outcome
        assert_eq!(j.kind, JoinKind::Identity, "the u16 space fits the docs");
        assert!(j.max_u16 as u32 <= doc_count, "the max u16 is a valid docnum");
        // the dense stream carries the NUL/zero padding, so its min is 0
        assert_eq!(j.min_u16, 0, "the dense stream has zero padding");
        assert!(
            j.distinct_u16 > 10_000,
            "the stream references a large document space"
        );
        // the openable subset is the whole clean u16 set (all within range)
        let all: Vec<u16> = idx
            .vt_stream
            .as_ref()
            .map(|s| s.records.iter().flat_map(|r| r.u16_words.iter().copied()).collect())
            .unwrap_or_default();
        let open = j.openable(&all);
        assert!(
            !open.is_empty(),
            "at least some u16 ids fall in the current doc range"
        );
        assert!(
            open.iter().all(|v| u32::from(*v) <= doc_count),
            "openable ids stay within the current range"
        );
        // every non-zero clean u16 is openable in this identity base
        let nonzero_clean: Vec<u16> = all.iter().copied().filter(|&v| v != 0).collect();
        assert_eq!(open.len(), nonzero_clean.len(), "identity: all non-zero u16s open");
    }
}
