//! A text input whose Backspace and Delete remove whole characters.
//!
//! # Why this exists
//!
//! `egui::TextEdit` addresses text by `char`, not by grapheme cluster.
//! `CCursor` is `epaint`'s `CharIndex`, and the type's own documentation says
//! the selection is "based on character count (NOT byte count!)"
//! (`egui-0.36.2/src/text_selection/cursor_range.rs:11`). A `char` is a
//! Unicode scalar value, so an `e` followed by U+0301 is two of them, a flag is
//! two regional indicators, and a ZWJ emoji sequence is several plus the
//! joiner.
//!
//! Measured through this crate's harness
//! (`crates/ui-egui/tests/grapheme_backspace.rs`), one Backspace at the end:
//!
//! | input | egui leaves | what the user sees |
//! | --- | --- | --- |
//! | `cafe` + U+0301 | `cafe` | the e survives; only the accent was removed |
//! | `hi` + US flag | `hi` + one RI | half a flag, rendered as a letter |
//! | `a` + woman-technologist | `a` + woman + ZWJ | a dangling ZWJ, rendered as tofu |
//!
//! So the user has to press Backspace more than once, and in between they see a
//! broken glyph.
//!
//! # What it does
//!
//! [`cluster_text_edit_singleline`] and [`cluster_text_edit_multiline`] wrap
//! `egui::TextEdit` and own Backspace and Delete in cluster units. Everything
//! else stays egui's: rendering, scrolling, click-to-position, drag selection,
//! clipboard, undo, IME. The wrapper does not reimplement a text widget; it
//! corrects two keys.
//!
//! # How
//!
//! Before the widget runs, the keystroke is inspected. If the caret sits
//! *inside* a grapheme cluster, the event is removed from the input so egui
//! never sees it, the correct cluster is deleted here, and egui's cursor is
//! repositioned afterwards. When the caret is already on a boundary the event
//! is left alone and egui handles it as it always did, which is the common case
//! and costs one `is_char_boundary` check.
//!
//! [ADR-0004](https://github.com/WyattAu/engineering-standards/blob/main/docs/adr/0004-gui-toolchain-semantics-layer-not-toolkit.md)
//! calls this "consume the bricks, own the wall": egui draws the field, this
//! crate decides what a character is.

use egui::text::{CCursor, CCursorRange};
use egui::widgets::text_edit::TextEditState;
use egui::{AsIdSalt, Event, Id, Key, Response, Ui};

/// What the wrapper decided to do about a keystroke this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intercept {
    /// Let egui handle it. The common case, and the cheap one.
    Pass,
    /// The caret was inside a cluster. The event has been removed from the
    /// input and this is the byte range to delete instead.
    Delete { from: usize, to: usize },
}

/// A pending cluster-correct deletion, taken before a widget ran and applied
/// after it. See [`take_cluster_intercept`].
pub type ClusterAction = Intercept;

/// Byte offset of egui's caret, given in egui's `char` units.
///
/// egui's `CharIndex` counts characters from the start, so index `n` means
/// "after `n` characters". Getting this off by one silently deletes the
/// neighbouring character, so it is derived rather than looked up.
fn caret_byte(text: &str, caret_char: usize) -> usize {
    if caret_char == 0 {
        return 0;
    }
    match text.char_indices().nth(caret_char - 1) {
        Some((byte, ch)) => byte + ch.len_utf8(),
        // A caret past the end.
        None => text.len(),
    }
}

/// Number of whole clusters before `byte`.
///
/// When `byte` is on a cluster boundary this is the index of the cluster that
/// starts there, which is what a forward Delete should remove.
fn clusters_before(text: &str, byte: usize) -> usize {
    input_kit::grapheme::count(&text[..byte.min(text.len())])
}

/// Decide whether the key would split a cluster, and if so which bytes to remove.
///
/// `caret_char` is egui's caret in `char` units. `from_end` selects Backspace
/// (remove the character before the caret) or Delete (remove the one at it).
///
/// The rule is the same either way: take the whole grapheme cluster adjacent to
/// the caret on the deleting side, and only intercept when that cluster is
/// more than one `char`. A single-`char` cluster is exactly what egui would
/// have deleted, so it is left alone: that is the common case, it costs one
/// comparison, and egui keeps its own undo entry and cursor behaviour for it.
fn intercept_delete(text: &str, caret_char: usize, from_end: bool) -> Intercept {
    let caret = caret_byte(text, caret_char);
    let on_boundary = input_kit::grapheme::index(text, caret).is_some();

    // The index of the cluster the caret sits in, rounded down when the caret
    // is inside one.
    let here = if on_boundary {
        clusters_before(text, caret)
    } else {
        input_kit::grapheme::nearest_index(text, caret).unwrap_or(0)
    };

    // Which cluster comes out depends on which side of the caret it is on.
    let cluster = if from_end {
        // The cluster ending at the caret. When the caret is inside one, that
        // cluster comes out whole.
        let end_cluster = if on_boundary { here } else { here + 1 };
        if end_cluster == 0 {
            return Intercept::Pass; // nothing before the caret
        }
        end_cluster - 1
    } else {
        // The cluster starting at the caret. When the caret is inside one, the
        // whole cluster comes out rather than just its tail.
        if caret >= text.len() {
            return Intercept::Pass; // nothing after the caret
        }
        here
    };

    let from = input_kit::grapheme::byte_offset(text, cluster).unwrap_or(caret);
    let to = input_kit::grapheme::byte_offset(text, cluster + 1).unwrap_or(text.len());

    if from > to || to > text.len() {
        return Intercept::Pass;
    }

    let unit = &text[from..to];
    if unit.chars().count() <= 1 {
        // One char: egui's own delete is already correct.
        return Intercept::Pass;
    }

    // Both keys remove the whole cluster, whatever side the caret is on. When
    // the caret is inside the cluster there is no "the part before it" to keep:
    // the user sees one character, so deleting it removes all of it.
    Intercept::Delete { from, to }
}

/// The `char` index of `byte` in `text`, for putting egui's cursor back.
fn char_index_of(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].chars().count()
}

/// Take the Backspace/Delete keystroke out of the input if it splits a cluster.
///
/// Returns the interception and the event that was consumed, if any. The event
/// is removed from `ctx` before the widget runs so egui cannot act on it.
fn take_intercept(ctx: &egui::Context, id: Id, text: &str) -> Option<Intercept> {
    let state = TextEditState::load(ctx, id)?;
    let range: CCursorRange = state.cursor.char_range()?;
    let caret_char = range.primary.index.0;

    let mut decision = Intercept::Pass;
    let mut removed = false;

    ctx.input_mut(|i| {
        i.events.retain(|ev| match ev {
            Event::Key {
                key,
                pressed: true,
                repeat,
                ..
            } if *key == Key::Backspace || *key == Key::Delete => {
                // An auto-repeat is a held key, not a new intent; let egui
                // handle repeats so its own repeat logic still works.
                if *repeat {
                    return true;
                }
                let from_end = *key == Key::Backspace;
                match intercept_delete(text, caret_char, from_end) {
                    Intercept::Pass => true,
                    Intercept::Delete { from, to } => {
                        decision = Intercept::Delete { from, to };
                        removed = true;
                        false
                    }
                }
            }
            _ => true,
        });
    });

    if removed {
        Some(decision)
    } else {
        None
    }
}

/// Apply an interception to the text and put egui's cursor back where it belongs.
fn apply_intercept(ctx: &egui::Context, id: Id, text: &mut String, action: Intercept) {
    let Intercept::Delete { from, to } = action else {
        return;
    };
    // The bounds came from this frame's text, and nothing has run since, so the
    // slice is valid; the guard is here because a silent panic in a text field
    // is worse than a missed deletion.
    if from > to || to > text.len() || !text.is_char_boundary(from) || !text.is_char_boundary(to) {
        return;
    }
    text.replace_range(from..to, "");

    // egui counts `char`s, so the cursor goes back in `char` units.
    let caret_char = char_index_of(text, from);
    if let Some(mut state) = TextEditState::load(ctx, id) {
        let caret = CCursor::new(caret_char);
        state.cursor.set_char_range(Some(CCursorRange::one(caret)));
        state.store(ctx, id);
    }
}

/// A single-line text input whose Backspace and Delete remove whole characters.
///
/// A drop-in replacement for `egui::TextEdit::singleline`, with the same
/// builder methods available through [`with_text_edit`].
pub fn cluster_text_edit_singleline<'a>(
    ui: &'a mut Ui,
    text: &'a mut String,
) -> ClusterTextEdit<'a> {
    // The default id must be per-widget, not a constant. A fixed id makes every
    // field in a panel share one `TextEditState` and one accessibility node, so
    // all but the last field vanish from the a11y tree. `egui`'s own default is
    // `ui.next_auto_id()`, which advances the auto counter; mirror that so a
    // panel of fields behaves the same whether they are wrapped or not.
    let id = ui.next_auto_id();
    ui.skip_ahead_auto_ids(1);
    ClusterTextEdit {
        ui,
        text,
        multiline: false,
        id,
        hint: String::new(),
        width: None,
        desired_rows: None,
        font: None,
        background: None,
        text_color: None,
        margin: None,
        password: false,
        char_limit: None,
    }
}

/// The builder state for a cluster-aware text input.
pub struct ClusterTextEdit<'a> {
    ui: &'a mut Ui,
    text: &'a mut String,
    multiline: bool,
    id: Id,
    hint: String,
    width: Option<f32>,
    desired_rows: Option<usize>,
    font: Option<egui::FontId>,
    background: Option<egui::Color32>,
    text_color: Option<egui::Color32>,
    margin: Option<egui::Margin>,
    password: bool,
    char_limit: Option<usize>,
}

impl ClusterTextEdit<'_> {
    /// Set the hint text shown while the field is empty.
    #[must_use]
    pub fn hint_text(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    /// Set the desired width in points.
    #[must_use]
    pub fn desired_width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Set the number of rows a multiline field wants to show.
    #[must_use]
    pub fn desired_rows(mut self, rows: usize) -> Self {
        self.desired_rows = Some(rows);
        self
    }

    /// Give the widget a stable id, matching `egui::TextEdit::id_salt`.
    #[must_use]
    pub fn id_salt(mut self, salt: impl AsIdSalt) -> Self {
        self.id = self.id.with(salt);
        self
    }

    /// Set the font used to render and size the text.
    #[must_use]
    pub fn font(mut self, font: egui::FontId) -> Self {
        self.font = Some(font);
        self
    }

    /// Set the background colour behind the text.
    #[must_use]
    pub fn background_color(mut self, color: egui::Color32) -> Self {
        self.background = Some(color);
        self
    }

    /// Set the text colour.
    #[must_use]
    pub fn text_color(mut self, color: egui::Color32) -> Self {
        self.text_color = Some(color);
        self
    }

    /// Set the margin between the frame and the text.
    #[must_use]
    pub fn margin(mut self, margin: egui::Margin) -> Self {
        self.margin = Some(margin);
        self
    }

    /// Mask the text, for password entry.
    #[must_use]
    pub fn password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    /// Cap the text at `limit` characters, as `egui` does.
    #[must_use]
    pub fn char_limit(mut self, limit: usize) -> Self {
        self.char_limit = Some(limit);
        self
    }

    /// Show the field, correcting Backspace and Delete to whole characters.
    pub fn show(self) -> Response {
        let id = self.id;
        let ctx = self.ui.ctx().clone();

        // Decide before the widget runs: if the caret is inside a cluster, the
        // keystroke never reaches egui.
        let action = take_intercept(&ctx, id, self.text);

        let mut builder = if self.multiline {
            egui::TextEdit::multiline(self.text)
        } else {
            egui::TextEdit::singleline(self.text)
        };
        if !self.hint.is_empty() {
            builder = builder.hint_text(&self.hint);
        }
        if let Some(width) = self.width {
            builder = builder.desired_width(width);
        }
        if let Some(rows) = self.desired_rows {
            builder = builder.desired_rows(rows);
        }
        if let Some(font) = self.font {
            builder = builder.font(font);
        }
        if let Some(background) = self.background {
            builder = builder.background_color(background);
        }
        if let Some(color) = self.text_color {
            builder = builder.text_color(color);
        }
        if let Some(margin) = self.margin {
            builder = builder.margin(margin);
        }
        if self.password {
            builder = builder.password(true);
        }
        if let Some(limit) = self.char_limit {
            builder = builder.char_limit(limit);
        }
        builder = builder.id(id);

        let response = self.ui.add(builder);

        // Then apply our own deletion and put the cursor back.
        if let Some(action) = action {
            apply_intercept(&ctx, id, self.text, action);
        }
        response
    }

}

/// Remove a Backspace or Delete keystroke from `ctx`'s input when it would
/// split a grapheme cluster.
///
/// Call this before the widget runs and [`apply_cluster_action`] after. It is
/// the seam for widgets this crate does not build, which is every widget that
/// is not a plain `egui::TextEdit`: the PDF form fields in `forms_ui.rs` lay
/// the editor out inside a fixed rect and need the galley, so they cannot use
/// [`ClusterTextEdit`] but can still get cluster-correct deletion.
#[must_use]
pub fn take_cluster_intercept(ctx: &egui::Context, id: Id, text: &str) -> Option<ClusterAction> {
    take_intercept(ctx, id, text)
}

/// Apply a [`ClusterAction`] taken by [`take_cluster_intercept`].
pub fn apply_cluster_action(
    ctx: &egui::Context,
    id: Id,
    text: &mut String,
    action: ClusterAction,
) {
    apply_intercept(ctx, id, text, action);
}

/// A multiline text input whose Backspace and Delete remove whole characters.
///
/// A drop-in replacement for `egui::TextEdit::multiline`.
pub fn cluster_text_edit_multiline<'a>(
    ui: &'a mut Ui,
    text: &'a mut String,
) -> ClusterTextEdit<'a> {
    let mut field = cluster_text_edit_singleline(ui, text);
    field.multiline = true;
    field
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delete_backspace(text: &str, caret_char: usize) -> Intercept {
        intercept_delete(text, caret_char, true)
    }

    fn delete_forward(text: &str, caret_char: usize) -> Intercept {
        intercept_delete(text, caret_char, false)
    }

    #[test]
    fn ascii_is_left_to_egui() {
        assert_eq!(delete_backspace("hello", 5), Intercept::Pass);
        assert_eq!(delete_forward("hello", 2), Intercept::Pass);
        assert_eq!(delete_backspace("", 0), Intercept::Pass);
        assert_eq!(delete_forward("", 0), Intercept::Pass);
        assert_eq!(delete_forward("hello", 5), Intercept::Pass);
    }

    /// The case the wrapper exists for: the caret sits after a base letter and
    /// its combining mark, and one Backspace must remove both.
    #[test]
    fn backspace_at_the_end_of_a_combined_character_takes_both() {
        // "cafe" + U+0301: 5 chars, 6 bytes, 4 clusters.
        let text = "cafe\u{301}";
        assert_eq!(text.chars().count(), 5);
        assert_eq!(input_kit::grapheme::count(text), 4);
        match delete_backspace(text, 5) {
            Intercept::Delete { from, to } => {
                assert_eq!(&text[from..to], "e\u{301}", "one whole character");
                assert_eq!(to, text.len());
            }
            Intercept::Pass => panic!("the combining mark should have been caught"),
        }
    }

    /// Anywhere inside the combined character gives the same answer: whether
    /// the caret is between the base and its mark, or on the mark itself.
    #[test]
    fn backspace_anywhere_inside_a_combined_character_takes_all_of_it() {
        let text = "cafe\u{301}";
        // byte 3 is before the e, 4 is after the e, 5 is after the mark.
        for caret_char in [4, 5] {
            match delete_backspace(text, caret_char) {
                Intercept::Delete { from, to } => {
                    assert_eq!(
                        &text[from..to], "e\u{301}",
                        "caret at char {caret_char} split the character"
                    );
                }
                Intercept::Pass => panic!("caret at char {caret_char} was passed through"),
            }
        }
    }

    #[test]
    fn backspace_mid_string_takes_the_whole_character() {
        let text = "a\u{1F1FA}\u{1F1F8}b"; // a, flag, b
        // After the flag (a + RI + RI = 3 chars).
        match delete_backspace(text, 3) {
            Intercept::Delete { from, to } => {
                assert_eq!(&text[from..to], "\u{1F1FA}\u{1F1F8}");
                assert_eq!(&text[..from], "a");
                assert_eq!(&text[to..], "b");
            }
            Intercept::Pass => panic!("half a flag should not survive"),
        }
    }

    /// Forward Delete at the start of a flag takes the whole flag, and leaves the
    /// space in front of it alone.
    #[test]
    fn forward_delete_takes_the_whole_cluster_from_its_start() {
        // MEASURED: "hi " is three bytes, each regional indicator is four, so
        // the flag spans bytes 3..=10 and the text is 11 bytes.
        let text = "hi \u{1F1FA}\u{1F1F8}";
        assert_eq!(text.len(), 11);
        // Caret after "hi " (three chars), so byte 3, the start of the flag.
        match delete_forward(text, 3) {
            Intercept::Delete { from, to } => {
                assert_eq!(from, 3, "keeps the space, which is before the caret");
                assert_eq!(&text[from..to], "\u{1F1FA}\u{1F1F8}");
                assert_eq!(&text[..from], "hi ");
            }
            Intercept::Pass => panic!("half a flag should not survive"),
        }
    }

    /// Forward Delete with the caret between a base and its mark removes the
    /// whole character, because the user sees one character, not two.
    #[test]
    fn forward_delete_inside_a_combined_character_takes_all_of_it() {
        let text = "cafe\u{301}";
        // Caret after the e (four chars), so byte 4: inside the cluster.
        match delete_forward(text, 4) {
            Intercept::Delete { from, to } => {
                assert_eq!(from, 3, "the base letter goes with its mark");
                assert_eq!(to, 6);
                assert_eq!(&text[from..to], "e\u{301}");
            }
            Intercept::Pass => panic!("a dangling mark should have been caught"),
        }
    }

    /// egui's caret is a char index meaning "after n chars". A conversion that
    /// is off by one silently deletes the neighbour, so it is pinned here.
    #[test]
    fn the_caret_conversion_is_not_off_by_one() {
        let flag = "hi \u{1F1FA}\u{1F1F8}";
        assert_eq!(caret_byte(flag, 0), 0);
        assert_eq!(caret_byte(flag, 1), 1, "after h");
        assert_eq!(caret_byte(flag, 2), 2, "after i");
        assert_eq!(caret_byte(flag, 3), 3, "after the space");
        assert_eq!(caret_byte(flag, 4), 7, "after the first regional indicator");
        assert_eq!(caret_byte(flag, 5), 11, "after both");
        // Multi-byte: "a" + U+0301 is two chars, three bytes.
        assert_eq!(caret_byte("a\u{301}", 2), 3);
        assert_eq!(caret_byte("a\u{301}", 1), 1, "between the base and its mark");
    }

    /// The wrapper must never hand back a range that is not whole clusters.
    #[test]
    fn every_intercept_is_a_whole_number_of_clusters() {
        for text in [
            "cafe\u{301}",
            "hi \u{1F1FA}\u{1F1F8}",
            "a\u{1F469}\u{200D}\u{1F4BB}b",
            "hello",
            "\u{1F1FA}\u{1F1F8}",
        ] {
            let chars = text.chars().count();
            for caret_char in 0..=chars {
                for from_end in [true, false] {
                    let decision = intercept_delete(text, caret_char, from_end);
                    let Intercept::Delete { from, to } = decision else {
                        continue;
                    };
                    assert!(from <= to, "{text:?} at {caret_char}: reversed range");
                    assert!(to <= text.len(), "{text:?} at {caret_char}: past the end");
                    assert!(
                        text.is_char_boundary(from) && text.is_char_boundary(to),
                        "{text:?} at {caret_char}: not on a char boundary"
                    );
                    // The removed span must be exactly one cluster.
                    let removed = &text[from..to];
                    assert_eq!(
                        input_kit::grapheme::count(removed),
                        1,
                        "{text:?} at {caret_char}: removed {removed:?}, which is not one cluster"
                    );
                    // And the rest must still be well-formed.
                    let remainder = format!("{}{}", &text[..from], &text[to..]);
                    assert_eq!(
                        input_kit::grapheme::count(&remainder),
                        input_kit::grapheme::count(text) - 1,
                        "{text:?} at {caret_char}: the remainder changed shape"
                    );
                }
            }
        }
    }

    #[test]
    fn char_index_conversion() {
        let text = "a\u{1F600}b";
        assert_eq!(char_index_of(text, 0), 0);
        assert_eq!(char_index_of(text, 1), 1);
        assert_eq!(char_index_of(text, 5), 2);
        assert_eq!(char_index_of(text, 6), 3);
        // A byte offset past the end clamps rather than panicking.
        assert_eq!(char_index_of(text, 99), 3);
    }
}