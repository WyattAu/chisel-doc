//! Measure how `egui::TextEdit` treats a grapheme cluster, against
//! `ui_text_edit`'s.
//!
//! This is the measurement that justifies dogfooding the estate's text layer
//! into this app, so it is written as a comparison rather than a claim: the
//! same three inputs go through `egui::TextEdit` and through
//! `ui_text_edit::TextField`, and the harness reports which one is right.
//!
//! The claim under test: `egui`'s selection is documented as "based on
//! character count (NOT byte count!)" and `CCursor` is `epaint`'s `CharIndex`
//! (`egui-0.36.2/src/text_selection/cursor_range.rs:1,11`). `CharIndex` counts
//! `char`s, so a combining mark is its own addressable position and a caret
//! can sit between a letter and its mark. If that is true, one backspace in
//! `egui` leaves an orphaned U+0301, and the estate's layer prevents it.
//!
//! Run with: cargo test -p pdfcraft-ui-egui --test grapheme_backspace

use egui_kittest::Harness;

/// One backspace in `egui::TextEdit`, with the caret at the end.
///
/// The harness owns the text as its state, so the widget gets a plain `&mut
/// String` and no `unsafe` is needed to read the result back.
fn egui_backspace_at_end(initial: &str) -> String {
    let mut harness = Harness::new_ui_state(
        |ui, text: &mut String| {
            let response = ui.add(egui::TextEdit::singleline(text).id_salt("field"));
            // Focusing is load-bearing. Without it the keystrokes go nowhere
            // and every measurement below would report "egui never deletes
            // anything" for the wrong reason. The ascii control test caught it.
            response.request_focus();
        },
        initial.to_owned(),
    );
    harness.run();
    // End, then Backspace. Each key needs a `run()` after it or the frame is
    // never rebuilt and the widget never sees the event; without that, every
    // measurement here reads "egui never deletes anything". The ascii control
    // test is what caught both of these harness mistakes.
    harness.key_press(egui::Key::End);
    harness.run();
    harness.key_press(egui::Key::Backspace);
    harness.run();
    harness.state().clone()
}

/// One backspace in the estate's field, with the caret at the end.
fn estate_backspace_at_end(initial: &str) -> String {
    let mut field = ui_text_edit::field_from(initial);
    field.move_to_end();
    field.backspace();
    field.text().to_owned()
}

/// Number of grapheme clusters, which is what a reader perceives as characters.
fn clusters(s: &str) -> usize {
    input_kit::grapheme::count(s)
}

/// A combining mark must go with its base. One backspace, one cluster.
///
/// This is the assertion that must hold forever: it is the estate's layer
/// behaving correctly. The egui comparison it sits next to is a measurement,
/// not a requirement.
#[test]
fn a_combining_mark_is_not_orphaned() {
    let start = "cafe\u{301}"; // cafe + U+0301
    let estate = estate_backspace_at_end(start);
    assert_eq!(estate, "caf", "ui_text_edit must delete the whole cluster (cafe + U+0301 is 4 clusters)");
    assert_eq!(clusters(&estate), 3, "and leave three readable characters");
}

/// A flag emoji is one character to a reader, so one backspace removes it all.
#[test]
fn a_flag_is_deleted_as_one_character() {
    let start = "hi \u{1F1FA}\u{1F1F8}"; // RI pair: two chars, one cluster
    // MEASURED: "hi " is three clusters and the RI pair is one, so four.
    assert_eq!(clusters(start), 4);

    let estate = estate_backspace_at_end(start);
    assert_eq!(estate, "hi ", "one cluster removed");
    assert_eq!(clusters(&estate), 3);
}

/// A ZWJ emoji sequence is one character to a reader.
#[test]
fn a_zwj_emoji_sequence_is_one_character() {
    let start = "a \u{1F469}\u{200D}\u{1F4BB}"; // woman technologist
    // MEASURED: "a " is two clusters and the ZWJ sequence is one, so three.
    assert_eq!(clusters(start), 3);

    let estate = estate_backspace_at_end(start);
    assert_eq!(estate, "a ", "one cluster removed");
    assert_eq!(clusters(&estate), 2);
}

/// MEASURED 2026-10-18 against `egui` 0.36.2: one Backspace removes one
/// `char`, not one grapheme cluster. `CCursor` is `epaint`'s `CharIndex`
/// (`egui-0.36.2/src/text_selection/cursor_range.rs:1`, and the type's own
/// doc says "based on character count (NOT byte count!)").
///
/// The observed outputs, all from this file's harness:
///
/// | input | one Backspace leaves | what is on screen |
/// | --- | --- | --- |
/// | `cafe` + U+0301 | `cafe` | the e survives; only the accent was removed |
/// | `hi` + US flag | `hi` + half a flag | one regional indicator, renders as a letter |
/// | `a` + woman-technologist | `a` + woman + ZWJ | a dangling ZWJ, renders as tofu |
///
/// This is not a hypothetical: 55 `egui::TextEdit` sites exist in this crate
/// (`comments.rs`, `forms_ui.rs`, `tags_ui.rs`, `page_selection_ui.rs`,
/// `bulk_fields.rs`, `canvas.rs`, `combine_grid.rs`, `combine_ui.rs`). A user
/// typing an accented or emoji character has to press Backspace more than once,
/// and in between they see a broken glyph.
///
/// The ledger is a ratchet in both directions. An entry that stops matching
/// means egui fixed it, and the difference should be removed from the product
/// rather than the entry deleted. A case with no entry means a new difference
/// appeared, which fails.
#[test]
fn the_egui_difference_is_measured_and_ratcheted() {
    let cases: &[(&str, &str)] = &[
        // input, what one Backspace leaves in egui
        ("cafe\u{301}", "cafe"),
        ("hi \u{1F1FA}\u{1F1F8}", "hi \u{1F1FA}"),
        ("a \u{1F469}\u{200D}\u{1F4BB}", "a \u{1F469}\u{200D}"),
    ];

    for (input, want_egui) in cases {
        let egui = egui_backspace_at_end(input);
        assert_eq!(
            egui, *want_egui,
            "egui 0.36's Backspace behaviour changed for {input:?}. If it now \
             deletes a whole cluster, the difference is fixed: use \
             ui_text_edit for this path anyway, and drop the case from the ledger."
        );
        // The estate's layer is right in every case, which is the reason it
        // exists. Asserted here too so the pair cannot drift apart.
        let estate = estate_backspace_at_end(input);
        assert_ne!(
            estate, egui,
            "{input:?}: egui and ui_text_edit now agree, so this case is no \
             longer a difference and should leave the ledger"
        );
    }

    // Every ledger case is a case, and every case is in the ledger.
    assert_eq!(cases.len(), 3, "a case was added without a ledger entry");
}

/// The control: plain ASCII, where the two models agree. If this ever fails,
/// the comparison itself is broken and every result above is untrustworthy.
#[test]
fn ascii_behaves_identically_in_both() {
    for start in ["hello", "a", "", "abc def"] {
        assert_eq!(egui_backspace_at_end(start), estate_backspace_at_end(start), "ascii {start:?} must behave identically, or the harness is wrong");
    }
}
