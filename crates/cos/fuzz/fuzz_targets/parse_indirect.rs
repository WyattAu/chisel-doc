//! Feeds arbitrary bytes to the indirect-object parser at several offsets.
//!
//! The COS object layer parses untrusted bytes from any PDF a user opens, so
//! this asserts Err-not-panic across the whole input space: `parse_indirect`
//! returning `Ok` or `Err` are both fine; unwinding, aborting or overflowing
//! the stack is a finding (fuzz policy: engineering-standards §4).
//!
//! Stream `/Length` resolution is fuzzed too — the resolver supplies no
//! length (deferred), zero, a value derived from the input, and `i64::MAX`
//! (the stream-runs-to-end path) — because that callback reaches different
//! stream-length branches on the parser.
#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use libfuzzer_sys::fuzz_target;
use pdfcraft_cos::{ObjRef, parse_indirect};

fuzz_target!(|data: &[u8]| {
    let Some(head) = data.first().copied() else { return };

    let resolve = |_ref: ObjRef| -> Option<i64> {
        match head & 0x3 {
            0 => None,
            1 => Some(0),
            2 => Some(i64::MAX),
            _ => Some(i64::from(head) * 1_000),
        }
    };

    // Whole buffer, then an offset derived from the input's first byte: the
    // fuzzer converges on prefixes that only parse mid-buffer.
    let _ = parse_indirect(data, 0, &resolve);
    let off = usize::from(head) % data.len();
    let _ = parse_indirect(data, off, &resolve);
});