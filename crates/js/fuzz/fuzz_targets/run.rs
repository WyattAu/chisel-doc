//! Feeds arbitrary text to the Acrobat-JavaScript sandbox: the script engine
//! that runs form field code, button actions and document scripts from any
//! PDF a user opens.
//!
//! The untrusted surface is the script text, so the harness fuzzes that and
//! derives the event/document strings from it too — hostile names reach the
//! field/document lookup paths. Fields and document scripts are passed empty:
//! the engine's own object model (`this`, `app`, `event`) is fully reachable
//! without them, and empty inputs keep one iteration cheap.
//!
//! `Limits` is tightened well below `Limits::default()` (1M loop iterations)
//! so a fuzz run stays fast per engineering-standards §4 — the engine's own
//! caps are the resource guard; without file, network or timer access there
//! is nothing else to amplify.
#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use libfuzzer_sys::fuzz_target;
use pdfcraft_js::{DocInfo, Event, Limits, run};

const LIMITS: Limits = Limits { loop_iterations: 10_000, recursion: 32 };

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let script = text.chars().take(4_096).collect::<String>();
    let name: String = script.chars().take(32).collect();

    let event = Event {
        name: name.clone(),
        kind: "Field".into(),
        target: Some(name.clone()),
        value: name.clone(),
        change: String::new(),
        will_commit: false,
    };
    let doc = DocInfo { file_name: name, num_pages: 1, page: 0, info: Vec::new() };

    let _outcome = run(&script, &event, &doc, &[], &[], LIMITS);
});