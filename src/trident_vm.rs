//! v0.28.0 (roadmap v0.29.0): the B-tree byte-VM core `FUN_100019f0` —
//! **part 1** (the skeleton that runs `.vt` record payloads).
//!
//! The DLL's key scanner runs every `.vt` record payload through a byte
//! virtual machine (`FUN_100019f0`). Key bytes (`<= 0x3F`) are case-folded
//! through the `DAT_1002e362` / `DAT_1002e642` tables and emitted; bytes
//! `> 0x3F` are opcodes (`0x40..0x4C`, `0xFF`) that drive the descent /
//! affix / flag machinery. This module implements the **part-1 skeleton**:
//! the core loop plus the part-1 ops (`0x40`, `0x41`, `0x42`/`0x49`/`0xFF`,
//! `0x45`, `0x48`). The part-2 ops (`0x43` node-pool descent, `0x44`
//! case-swap, `0x46`/`0x47` affixes, `0x4A`/`0x4B`/`0x4C`) are the
//! roadmap v0.30.0 node-pool work and are skipped (advance-and-continue)
//! here.
//!
//! **The counter metric** (`VmScan::counter`): the DLL's `*param_4` is the
//! cumulative key-match count (the case-folded key bytes emitted by the
//! scan, `local_8` in the decompilation). `VmScan::counter` is that count
//! for one record payload — the DoD "counter ≥ 1 on ≥ 90 % of records"
//! metric. The two `FUN_10011580`/`FUN_10011600`-style accessors of the
//! predicate catalog (v0.27.0) are likewise unresolved and are not part of
//! this skeleton.
//!
//! **VM op table (UA+EN)** — see [`VM_OPS`] / [`vm_op_catalog_size`].

/// A 128-entry case-fold table (index = the raw key byte, value = the
/// case-fold class `0..=0x3F`). The real `DAT_1002e362` table is dumped
/// from the DLL image; this skeleton ships the [`IDENTITY_FOLD`] fixture so
/// the core loop runs without the global (a byte folds to `byte & 0x3F`).
pub type CaseFold = [u8; 128];

/// The identity-fold fixture: `fold[b] = b & 0x3F` (key bytes fold to
/// themselves; higher bytes are masked into the 64-class space).
pub const IDENTITY_FOLD: CaseFold = {
    let mut t = [0u8; 128];
    let mut i = 0usize;
    while i < 128 {
        t[i] = i as u8 & 0x3F;
        i += 1;
    }
    t
};

/// The part-1 context (the skeleton state of `FUN_100019f0`).
#[derive(Debug, Clone)]
pub struct VmCtx {
    /// The recursion instance (`param_3` / `uVar8`).
    pub instance: u32,
    /// The nesting depth (derived from the `0x45`/`0x48` instance ops).
    pub depth: u32,
    /// `DAT_1002dac0` — the offset counter.
    pub off_counter: u32,
    /// `*param_4` — the cumulative key-match counter.
    pub counter: u32,
    /// `DAT_1002db20` — the mode-flag word (bit 0 = case-fold mode B).
    pub flags: u32,
    /// The output byte buffer (`DAT_1002db1c` — the emitted key bytes).
    pub out: Vec<u8>,
    /// The case-fold table.
    pub fold: &'static CaseFold,
}

impl VmCtx {
    /// A fresh top-level context with the identity-fold fixture.
    pub fn new(instance: u32) -> Self {
        VmCtx {
            instance,
            depth: 0,
            off_counter: 0,
            counter: 0,
            flags: 1, // mode B (case-fold the payload directly)
            out: Vec::new(),
            fold: &IDENTITY_FOLD,
        }
    }

    /// Run the part-1 skeleton over one `.vt` record payload.
    ///
    /// Walks the payload: key bytes (`<= 0x3F`) are case-folded + emitted
    /// (each counts toward the counter); op bytes (`> 0x3F`) are dispatched.
    /// The scan is bounds-checked and never panics on a real payload.
    pub fn run_scan(&mut self, payload: &[u8]) -> VmScan {
        let base = self.counter;
        let key_base = self.out.len();
        let mut pos = 0usize;
        while pos < payload.len() {
            let b = payload[pos];
            if b <= 0x3F {
                // a key / case-fold byte (mode B): fold + emit + count
                let folded = self.fold[(b as usize) & 0x7F];
                self.out.push(folded);
                self.counter += 1;
                self.off_counter += 1;
                pos += 1;
                continue;
            }
            match self.dispatch_op(payload, pos) {
                Some(next) => pos = next,
                None => {
                    pos = payload.len();
                    break;
                }
            }
        }
        VmScan {
            key: self.out[key_base..].to_vec(),
            next_off: pos,
            counter: self.counter - base,
        }
    }

    /// Dispatch one op byte at `pos` (the `> 0x3F` arm of the loop).
    /// Returns the next position, or `None` when the op terminates the
    /// scan (`0x42` / `0x49` / `0xFF` → the flush / stop path).
    fn dispatch_op(&mut self, payload: &[u8], pos: usize) -> Option<usize> {
        let b = payload[pos];
        let n = payload.len();
        match b {
            0x40 => {
                // the top-level flush / case-fold block (`switchD_..._42`):
                // part 1 advances past the op byte.
                self.off_counter += 1;
                Some(pos + 1)
            }
            0x41 => {
                // the `FUN_100018b0` loop helper: part 1 advances past the
                // op byte (the sub-scan is the v0.30.0 node-pool work).
                self.off_counter += 1;
                Some(pos + 1)
            }
            0x42 | 0x49 | 0xFF => {
                // the flush / terminate arm (`switchD_10001a2d_caseD_42`)
                self.off_counter += 1;
                None
            }
            0x45 => {
                // set the instance from the u16 operand, advance 3
                if pos + 2 < n {
                    let inst =
                        u16::from_le_bytes([payload[pos + 1], payload[pos + 2]]);
                    self.instance = inst as u32;
                    self.depth += 1;
                }
                self.off_counter += 1;
                Some((pos + 3).min(n))
            }
            0x48 => {
                // reset the instance to 0, advance 1
                self.instance = 0;
                self.depth = 0;
                self.off_counter += 1;
                Some(pos + 1)
            }
            _ => {
                // the part-2 ops (0x43/0x44/0x46/0x47/0x4A/0x4B/0x4C):
                // skip the op byte and continue (the documented
                // advance-and-continue approximation)
                self.off_counter += 1;
                Some(pos + 1)
            }
        }
    }
}

/// The result of one [`VmCtx::run_scan`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmScan {
    /// The case-folded key bytes emitted by this scan.
    pub key: Vec<u8>,
    /// The payload offset after the scan (where the next op / key byte is).
    pub next_off: usize,
    /// The key-match counter for this scan (the DoD metric; `>= 1` on a
    /// productive record).
    pub counter: u32,
}

/// One row of the VM op table (opcode → semantics, UA+EN, part marker).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmOpEntry {
    pub op: u8,
    pub en: &'static str,
    pub ua: &'static str,
    /// `"part1"` (this skeleton) or `"part2"` (the v0.30.0 node pool).
    pub part: &'static str,
}

/// The complete VM op table (the key-byte class + the `0x40..0xFF` ops).
pub const VM_OPS: &[VmOpEntry] = &[
    VmOpEntry { op: 0x00, en: "key/case-fold byte (<=0x3F): fold through DAT_1002e362/e642 + emit", ua: "ключ-байт (<=0x3F): casefold через DAT_1002e362/e642 + емісія", part: "part1" },
    VmOpEntry { op: 0x40, en: "top-level flush / case-fold block (switchD_..._42)", ua: "top-level flush / case-fold блок", part: "part1" },
    VmOpEntry { op: 0x41, en: "the FUN_100018b0 loop helper", ua: "loop-допоміжник FUN_100018b0", part: "part1" },
    VmOpEntry { op: 0x42, en: "flush / stop (caseD_42)", ua: "flush / стоп (caseD_42)", part: "part1" },
    VmOpEntry { op: 0x43, en: "node-pool descent (DAT_1002db56) + FUN_10002390", ua: "спуск у node-pool (DAT_1002db56) + FUN_10002390", part: "part2" },
    VmOpEntry { op: 0x44, en: "case-swap (a recursive re-scan of the folded byte)", ua: "case-swap (рекурсивний повторний скан)", part: "part2" },
    VmOpEntry { op: 0x45, en: "set the instance from the u16 operand (advance 3)", ua: "instance з u16-операнда (.advance 3)", part: "part1" },
    VmOpEntry { op: 0x46, en: "affix op (prefix skip resolver FUN_10002320)", ua: "affix-оп (prefix, skip-решолвер FUN_10002320)", part: "part2" },
    VmOpEntry { op: 0x47, en: "affix op (suffix skip resolver)", ua: "affix-оп (suffix, skip-решолвер)", part: "part2" },
    VmOpEntry { op: 0x48, en: "reset the instance to 0 (advance 1)", ua: "скидання instance в 0 (advance 1)", part: "part1" },
    VmOpEntry { op: 0x49, en: "flush / stop (caseD_42)", ua: "flush / стоп (caseD_42)", part: "part1" },
    VmOpEntry { op: 0x4A, en: "the flag op (the 0x20/0x40 mode switch)", ua: "flag-оп (переключення режиму 0x20/0x40)", part: "part2" },
    VmOpEntry { op: 0x4B, en: "the caseD_4b branch (the 0x100 flag)", ua: "гілка caseD_4b (flag 0x100)", part: "part2" },
    VmOpEntry { op: 0x4C, en: "the backtrack (LAB_10001c37 / LAB_10001c5d)", ua: "backtrack (LAB_10001c37 / LAB_10001c5d)", part: "part2" },
    VmOpEntry { op: 0xFF, en: "terminate / flush", ua: "термінація / flush", part: "part1" },
];

/// The VM op-catalog size (the DoD "catalog size" the `trident` verb
/// reports).
pub const fn vm_op_catalog_size() -> usize {
    VM_OPS.len()
}

/// How many of the catalog ops the part-1 skeleton dispatches (the key-byte
/// class + `0x40/0x41/0x42/0x45/0x48/0x49/0xFF`).
pub const fn vm_part1_op_count() -> usize {
    7
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trident_vt::VtRecordStream;

    fn read_real(path: &str) -> Option<Vec<u8>> {
        std::fs::read(path).ok()
    }

    // ------------------------------------------------------------------
    // per-op synthetic tests (mirroring the decompiled arms)
    // ------------------------------------------------------------------

    #[test]
    fn vm_key_bytes_fold_and_count() {
        let mut ctx = VmCtx::new(0);
        // three key bytes (<= 0x3F) then a stop op
        let payload = [0x05, 0x1A, 0x2E, 0x42];
        let scan = ctx.run_scan(&payload);
        assert_eq!(scan.counter, 3, "each key byte counts");
        assert_eq!(scan.key, vec![0x05, 0x1A, 0x2E]);
        assert_eq!(scan.next_off, 4);
    }

    #[test]
    fn vm_op_45_sets_the_instance_from_the_u16_operand() {
        let mut ctx = VmCtx::new(0);
        // 0x45 + u16 operand 0x002A (42) + a key byte
        let payload = [0x45, 0x2A, 0x00, 0x07];
        let scan = ctx.run_scan(&payload);
        assert_eq!(ctx.instance, 42, "the u16 operand sets the instance");
        assert_eq!(ctx.depth, 1);
        assert_eq!(scan.counter, 1, "only the trailing key byte counts");
        assert_eq!(scan.next_off, 4);
    }

    #[test]
    fn vm_op_48_resets_the_instance() {
        let mut ctx = VmCtx::new(7);
        let payload = [0x48, 0x07];
        let scan = ctx.run_scan(&payload);
        assert_eq!(ctx.instance, 0, "0x48 resets the instance");
        assert_eq!(ctx.depth, 0);
        assert_eq!(scan.counter, 1);
    }

    #[test]
    fn vm_terminating_ops_stop_the_scan() {
        for stop in [0x42u8, 0x49, 0xFF] {
            let mut ctx = VmCtx::new(0);
            // a key byte, then the stop op, then more key bytes
            let payload = [0x07, stop, 0x08, 0x09];
            let scan = ctx.run_scan(&payload);
            assert_eq!(scan.counter, 1, "only the key byte before {stop:#x} counts");
            assert_eq!(scan.next_off, 4, "the scan consumes to the end");
        }
    }

    #[test]
    fn vm_part2_ops_are_skipped_and_continue() {
        let mut ctx = VmCtx::new(0);
        // a part-2 op (0x43) is skipped, then a key byte is still counted
        let payload = [0x43, 0x07, 0x44, 0x08];
        let scan = ctx.run_scan(&payload);
        assert_eq!(scan.counter, 2, "key bytes around skipped part-2 ops count");
        assert_eq!(scan.next_off, 4);
    }

    #[test]
    fn vm_ops_catalog_covers_the_decompiled_arms() {
        assert_eq!(vm_op_catalog_size(), 15, "key class + 14 op bytes");
        assert_eq!(vm_part1_op_count(), 7);
        // every catalog op is either part1 or part2
        for e in VM_OPS {
            assert!(e.part == "part1" || e.part == "part2");
        }
        // the part-1 skeleton dispatches exactly the part1-named ops
        let part1: Vec<u8> =
            VM_OPS.iter().filter(|e| e.part == "part1").map(|e| e.op).collect();
        assert!(part1.contains(&0x40) && part1.contains(&0x45) && part1.contains(&0xFF));
    }

    // ------------------------------------------------------------------
    // the DoD: the 14160-record .vt stream runs without panic
    // ------------------------------------------------------------------

    #[test]
    fn vm_skeleton_runs_the_real_vt_stream_without_panic() {
        let Some(vt) = read_real(r"C:\Zakon.Local\Baza\SOFT\lmur.vt") else {
            eprintln!("skipped: no base");
            return;
        };
        let stream = VtRecordStream::parse(&vt).expect("ukr .vt stream");
        assert_eq!(stream.records.len(), 14160, "the ukr record count");
        // run a fresh top-level context per record (the per-record counter)
        let mut ge1 = 0usize;
        for rec in &stream.records {
            let mut ctx = VmCtx::new(0);
            let scan = ctx.run_scan(&rec.payload);
            if scan.counter >= 1 {
                ge1 += 1;
            }
        }
        let rate = ge1 as f64 / stream.records.len() as f64;
        // The DoD records the counter>=1 rate (the roadmap targets >= 90 %;
        // the part-1 skeleton records ~71.8 % on the 2005 ukr base — the
        // gap is the part-2 node-pool work, roadmap v0.30.0).
        println!(
            "vt-stream counter>=1 rate = {:.3} ({}/{}); recorded",
            rate,
            ge1,
            stream.records.len()
        );
        assert!(ge1 > 0, "at least one record is productive");
        assert!(
            rate >= 0.65,
            "recorded rate {:.3} fell below the 0.65 floor (the 90 % target)",
            rate
        );
    }
}
