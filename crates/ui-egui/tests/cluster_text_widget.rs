//! The wrapper, end to end through a real egui harness.
//!
//! `cluster_text.rs` unit-tests the decision function over every caret
//! position in every hostile string. This file proves the decision survives
//! the round trip: the widget is built, focused, a real Backspace key event is
//! delivered by the harness, and the resulting text is read back.
//!
//! Without this, the unit tests could all pass while the wrapper never fires,
//! which is the same failure mode the ascii control in
//! `grapheme_backspace.rs` caught earlier: an unfocused widget, or a missing
//! `run()`, makes every measurement read "nothing was deleted".

use egui_kittest::Harness;
use pdfcraft_ui_egui::cluster_text::cluster_text_edit_singleline;

/// Press Backspace once at the end of `initial`, through the real widget.
fn backspace_via_wrapper(initial: &str) -> String {
    let mut harness = Harness::new_ui_state(
        |ui, text: &mut String| {
            let response = cluster_text_edit_singleline(ui, text).id_salt("probe").show();
            response.request_focus();
        },
        initial.to_owned(),
    );
    harness.run();
    harness.key_press(egui::Key::End);
    harness.run();
    harness.key_press(egui::Key::Backspace);
    harness.run();
    harness.state().clone()
}

/// The same key through a bare `egui::TextEdit`, for the comparison.
fn backspace_via_egui(initial: &str) -> String {
    let mut harness = Harness::new_ui_state(
        |ui, text: &mut String| {
            let response = ui.add(egui::TextEdit::singleline(text).id_salt("probe"));
            response.request_focus();
        },
        initial.to_owned(),
    );
    harness.run();
    harness.key_press(egui::Key::End);
    harness.run();
    harness.key_press(egui::Key::Backspace);
    harness.run();
    harness.state().clone()
}

/// A combining mark goes with its base. One Backspace, one character.
#[test]
fn the_wrapper_deletes_a_whole_character() {
    assert_eq!(backspace_via_wrapper("cafe\u{301}"), "caf");
    assert_eq!(backspace_via_wrapper("hi \u{1F1FA}\u{1F1F8}"), "hi ");
    assert_eq!(backspace_via_wrapper("a \u{1F469}\u{200D}\u{1F4BB}"), "a ");
}

/// Plain text must be untouched: the wrapper only fires when egui would be
/// wrong, and this is the control that proves it.
#[test]
fn the_wrapper_leaves_plain_text_to_egui() {
    for start in ["hello", "a", "", "abc def", "1234"] {
        assert_eq!(
            backspace_via_wrapper(start),
            backspace_via_egui(start),
            "plain {start:?}: the wrapper changed what egui already did correctly"
        );
    }
}

/// Two fields in one panel must not share an id.
///
/// Regression test. The wrapper originally used a fixed `Id`, so every field in
/// a panel shared one `TextEditState` and one accessibility node, and all but the
/// last field silently vanished from the a11y tree. It was caught by
/// `tests/accessibility.rs` rather than by any test of the wrapper itself,
/// because the wrapper is perfectly correct in isolation and only wrong in a
/// panel.
#[test]
fn two_fields_in_one_panel_get_distinct_ids() {
    let mut harness = Harness::new_ui(|ui| {
        let mut a = String::from("one");
        let mut b = String::from("two");
        let first = cluster_text_edit_singleline(ui, &mut a).id_salt("a").show();
        let second = cluster_text_edit_singleline(ui, &mut b).id_salt("b").show();
        assert_ne!(first.id, second.id, "two fields shared an id");
    });
    harness.run();

    // And with no salt at all, two fields in a row still get distinct ids,
    // because the default advances egui's auto-id counter.
    let mut harness = Harness::new_ui(|ui| {
        let mut a = String::from("one");
        let mut b = String::from("two");
        let first = cluster_text_edit_singleline(ui, &mut a).show();
        let second = cluster_text_edit_singleline(ui, &mut b).show();
        assert_ne!(
            first.id, second.id,
            "two unsalted fields shared an id, so one would be missing from \
             the accessibility tree"
        );
    });
    harness.run();
}

/// The point of the whole file: the same keystroke, the same harness, two
/// different answers, and the wrapper's is the right one.
///
/// This doubles as a ratchet. If it ever fails because egui stopped splitting
/// clusters, the workaround can go.
#[test]
fn the_wrapper_and_bare_egui_differ_exactly_where_egui_is_wrong() {
    for start in ["cafe\u{301}", "hi \u{1F1FA}\u{1F1F8}", "a \u{1F469}\u{200D}\u{1F4BB}"] {
        let wrapped = backspace_via_wrapper(start);
        let bare = backspace_via_egui(start);
        assert_ne!(
            wrapped, bare,
            "{start:?}: the wrapper agreed with egui, so it did not fire; if \
             egui fixed this, remove the workaround"
        );
        assert!(
            wrapped.len() < start.len(),
            "{start:?}: the wrapper deleted nothing"
        );
    }
}