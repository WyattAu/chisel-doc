//! Feeds arbitrary bytes to the raw object lexer: the deepest recursive path
//! in the COS layer (nested arrays, dicts, strings, numbers, references).
//!
//! Err-not-panic contract per engineering-standards §4. `max_len` stays small
//! (CI runs it with `-max_len=4096`) so deeply nested arrays cannot turn one
//! input into minutes of recursion — libFuzzer's own limit plus the corpus
//! cap do that work.
#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use libfuzzer_sys::fuzz_target;
use pdfcraft_cos::Lexer;

fuzz_target!(|data: &[u8]| {
    let mut lex = Lexer::new(data, 0);
    lex.skip_ws();
    let _ = lex.object();

    if let Some(head) = data.first() {
        let mut lex = Lexer::new(data, usize::from(*head) % data.len());
        lex.skip_ws();
        let _ = lex.object();
    }
});