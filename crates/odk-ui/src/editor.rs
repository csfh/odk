//! The editing model of ADR 0038 (`docs/spec/odk-text-input.md`): the text, the selection, an
//! IME composition and the undo history. Offsets are UTF-8 bytes, always on grapheme-cluster
//! boundaries; GPUI's UTF-16 ranges are converted at the edge of the input
//! ([`Editor::range_to_utf16`]). No GPUI here, so it tests without a window.

use std::ops::Range;

use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

/// How many steps the undo history keeps (ODK's own).
const MAX_HISTORY: usize = 1000;

/// Where the text is selected: the `anchor` stays put and the `head` moves. A caret is a
/// selection whose two ends are the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    fn caret(at: usize) -> Self {
        Self {
            anchor: at,
            head: at,
        }
    }

    pub(crate) fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    pub(crate) fn reversed(&self) -> bool {
        self.head < self.anchor
    }
}

/// What made an edit, which decides whether the next one joins it in one undo step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Characters typed at the caret.
    Typing,
    /// Backspace over single graphemes.
    DeleteBack,
    /// Delete over single graphemes.
    DeleteForward,
    /// Everything else: a paste, a cut, a word, a selection, an IME commit.
    Other,
}

#[derive(Debug, Clone)]
struct Edit {
    at: usize,
    old: String,
    new: String,
    before: Selection,
    after: Selection,
    kind: Kind,
}

/// An IME composition in progress: where it began, what the text there was, and the selection
/// before it. While it runs the pre-edit text is in the text but not in the history; one step
/// is recorded when the composition ends.
#[derive(Debug, Clone)]
struct Composition {
    at: usize,
    old: String,
    before: Selection,
}

#[derive(Debug, Clone)]
pub(crate) struct Editor {
    text: String,
    selection: Selection,
    marked: Option<Range<usize>>,
    composition: Option<Composition>,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// Whether the next edit may join the last step: false once the selection has moved.
    joins: bool,
    /// A masked editor never shows where its words are: word movement goes to the ends.
    masked: bool,
}

impl Editor {
    /// An editor holding `text`, with the caret at its end.
    pub(crate) fn new(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            selection: Selection::caret(text.len()),
            marked: None,
            composition: None,
            undo: Vec::new(),
            redo: Vec::new(),
            joins: false,
            masked: false,
        }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn selection(&self) -> Selection {
        self.selection
    }

    pub(crate) fn selected_range(&self) -> Range<usize> {
        self.selection.range()
    }

    pub(crate) fn selected_text(&self) -> &str {
        &self.text[self.selection.range()]
    }

    pub(crate) fn cursor(&self) -> usize {
        self.selection.head
    }

    pub(crate) fn marked_range(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    pub(crate) fn is_composing(&self) -> bool {
        self.composition.is_some()
    }

    pub(crate) fn set_masked(&mut self, masked: bool) {
        self.masked = masked;
    }

    /// Replaces the whole text from outside, keeping the selection where it still fits and
    /// forgetting the history, since its steps no longer describe this text.
    pub(crate) fn set_text(&mut self, text: &str) {
        self.text = text.to_owned();
        self.selection = Selection {
            anchor: self.snap(self.selection.anchor),
            head: self.snap(self.selection.head),
        };
        self.marked = None;
        self.composition = None;
        self.undo.clear();
        self.redo.clear();
        self.joins = false;
    }

    // Offsets

    /// `offset` clamped to the text and moved back onto a grapheme boundary.
    fn snap(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        GraphemeCursor::new(offset, self.text.len(), true)
            .is_boundary(&self.text, 0)
            .map(|on| {
                if on {
                    offset
                } else {
                    self.prev_boundary(offset)
                }
            })
            .unwrap_or(offset)
    }

    fn prev_boundary(&self, offset: usize) -> usize {
        GraphemeCursor::new(offset, self.text.len(), true)
            .prev_boundary(&self.text, 0)
            .ok()
            .flatten()
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        GraphemeCursor::new(offset, self.text.len(), true)
            .next_boundary(&self.text, 0)
            .ok()
            .flatten()
            .unwrap_or(self.text.len())
    }

    /// The start of the line `offset` is on.
    pub(crate) fn line_start_of(&self, offset: usize) -> usize {
        self.text[..offset].rfind('\n').map_or(0, |i| i + 1)
    }

    /// The end of the line `offset` is on, before its line break.
    pub(crate) fn line_end_of(&self, offset: usize) -> usize {
        self.text[offset..]
            .find('\n')
            .map_or(self.text.len(), |i| offset + i)
    }

    /// Where the word before `offset` starts.
    fn word_start_before(&self, offset: usize) -> usize {
        let mut to = 0;
        for (at, segment) in self.text.split_word_bound_indices() {
            if at >= offset {
                break;
            }
            if is_word(segment) {
                to = at;
            }
        }
        to
    }

    /// Where the word after `offset` ends.
    fn word_end_after(&self, offset: usize) -> usize {
        for (at, segment) in self.text.split_word_bound_indices() {
            let end = at + segment.len();
            if end > offset && is_word(segment) {
                return end;
            }
        }
        self.text.len()
    }

    pub(crate) fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .encode_utf16()
            .count()
    }

    pub(crate) fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut units = 0;
        for (at, c) in self.text.char_indices() {
            if units >= offset {
                return at;
            }
            units += c.len_utf16();
        }
        self.text.len()
    }

    pub(crate) fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    pub(crate) fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    // Selecting

    fn select(&mut self, selection: Selection) {
        self.selection = selection;
        self.joins = false;
    }

    /// Selects from `anchor` to `head`, each moved onto a grapheme boundary.
    pub(crate) fn set_selection(&mut self, anchor: usize, head: usize) {
        let selection = Selection {
            anchor: self.snap(anchor),
            head: self.snap(head),
        };
        self.select(selection);
    }

    /// Moves the head to `to`, and the anchor with it unless `extend` keeps it.
    pub(crate) fn move_to(&mut self, to: usize, extend: bool) {
        let to = self.snap(to);
        let anchor = if extend { self.selection.anchor } else { to };
        self.select(Selection { anchor, head: to });
    }

    pub(crate) fn left(&mut self, extend: bool) {
        let s = self.selection;
        if !extend && !s.is_empty() {
            self.move_to(s.range().start, false);
        } else {
            self.move_to(self.prev_boundary(s.head), extend);
        }
    }

    pub(crate) fn right(&mut self, extend: bool) {
        let s = self.selection;
        if !extend && !s.is_empty() {
            self.move_to(s.range().end, false);
        } else {
            self.move_to(self.next_boundary(s.head), extend);
        }
    }

    pub(crate) fn word_left(&mut self, extend: bool) {
        let to = if self.masked {
            0
        } else {
            self.word_start_before(self.selection.head)
        };
        self.move_to(to, extend);
    }

    pub(crate) fn word_right(&mut self, extend: bool) {
        let to = if self.masked {
            self.text.len()
        } else {
            self.word_end_after(self.selection.head)
        };
        self.move_to(to, extend);
    }

    pub(crate) fn line_start(&mut self, extend: bool) {
        self.move_to(self.line_start_of(self.selection.head), extend);
    }

    pub(crate) fn line_end(&mut self, extend: bool) {
        self.move_to(self.line_end_of(self.selection.head), extend);
    }

    pub(crate) fn text_start(&mut self, extend: bool) {
        self.move_to(0, extend);
    }

    pub(crate) fn text_end(&mut self, extend: bool) {
        self.move_to(self.text.len(), extend);
    }

    pub(crate) fn select_all(&mut self) {
        self.select(Selection {
            anchor: 0,
            head: self.text.len(),
        });
    }

    /// Selects the word, or the run of spaces or marks, at `offset`.
    pub(crate) fn select_word_at(&mut self, offset: usize) {
        let offset = self.snap(offset);
        let mut found = offset..offset;
        for (at, segment) in self.text.split_word_bound_indices() {
            found = at..at + segment.len();
            if offset < found.end {
                break;
            }
        }
        self.select(Selection {
            anchor: found.start,
            head: found.end,
        });
    }

    /// Selects the line `offset` is on, without its line break.
    pub(crate) fn select_line_at(&mut self, offset: usize) {
        self.select(Selection {
            anchor: self.line_start_of(offset),
            head: self.line_end_of(offset),
        });
    }

    // Editing

    /// Inserts `text` over the selection as a step of its own, unlike typing, which joins runs.
    pub(crate) fn paste(&mut self, text: &str) {
        let range = self.selection.range();
        self.replace(range, text, Kind::Other);
    }

    /// What the platform asks for: replaces `range` (by default the pre-edit text, else the
    /// selection) with `text`. A composition ends here, as one step.
    pub(crate) fn replace_text(&mut self, range: Option<Range<usize>>, text: &str) {
        if let Some(marked) = self.marked.clone() {
            let inside = range.clone().unwrap_or_else(|| marked.clone());
            if inside.start >= marked.start && inside.end <= marked.end {
                self.splice_marked(inside, text);
                self.commit_composition();
                return;
            }
            self.commit_composition();
        }
        let range = range.unwrap_or_else(|| self.selection.range());
        let kind = if range.is_empty() && !text.is_empty() {
            Kind::Typing
        } else {
            Kind::Other
        };
        self.replace(range, text, kind);
    }

    /// Puts pre-edit text `text` in place of `range` (by default the earlier pre-edit text,
    /// else the selection), marked, with `selected` (offsets inside `text`) selected. It is
    /// not a step of the history until the composition ends.
    pub(crate) fn mark_text(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) {
        if let Some(marked) = self.marked.clone()
            && let Some(range) = &range
            && !(range.start >= marked.start && range.end <= marked.end)
        {
            self.commit_composition();
        }
        let range = range
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.range());
        if self.composition.is_none() {
            self.composition = Some(Composition {
                at: range.start,
                old: self.text[range.clone()].to_owned(),
                before: self.selection,
            });
            self.marked = Some(range.clone());
        }
        self.splice_marked(range.clone(), text);
        let start = range.start;
        self.selection = match selected {
            Some(s) => Selection {
                anchor: self.snap(start + s.start),
                head: self.snap(start + s.end),
            },
            None => Selection::caret(start + text.len()),
        };
        if text.is_empty() {
            self.commit_composition();
        }
    }

    /// Replaces `range`, inside the pre-edit text, with `text`, and moves the caret after it.
    fn splice_marked(&mut self, range: Range<usize>, text: &str) {
        let marked = self.marked.clone().unwrap_or(range.clone());
        self.text.replace_range(range.clone(), text);
        self.marked = Some(marked.start..marked.end - range.len() + text.len());
        self.selection = Selection::caret(range.start + text.len());
    }

    /// Ends the composition, keeping what it made as one step of the history.
    pub(crate) fn commit_composition(&mut self) {
        let Some(composition) = self.composition.take() else {
            return;
        };
        let end = self
            .marked
            .take()
            .map_or(composition.at, |m| m.end.max(composition.at));
        let new = self.text[composition.at..end].to_owned();
        if new != composition.old {
            self.record(Edit {
                at: composition.at,
                old: composition.old,
                new,
                before: composition.before,
                after: self.selection,
                kind: Kind::Other,
            });
        }
    }

    pub(crate) fn backspace(&mut self) {
        let s = self.selection;
        if !s.is_empty() {
            self.replace(s.range(), "", Kind::Other);
        } else if s.head > 0 {
            self.replace(self.prev_boundary(s.head)..s.head, "", Kind::DeleteBack);
        }
    }

    pub(crate) fn delete(&mut self) {
        let s = self.selection;
        if !s.is_empty() {
            self.replace(s.range(), "", Kind::Other);
        } else if s.head < self.text.len() {
            self.replace(s.head..self.next_boundary(s.head), "", Kind::DeleteForward);
        }
    }

    pub(crate) fn backspace_word(&mut self) {
        let s = self.selection;
        if !s.is_empty() {
            self.replace(s.range(), "", Kind::Other);
            return;
        }
        let from = if self.masked {
            0
        } else {
            self.word_start_before(s.head)
        };
        self.replace(from..s.head, "", Kind::Other);
    }

    pub(crate) fn delete_word(&mut self) {
        let s = self.selection;
        if !s.is_empty() {
            self.replace(s.range(), "", Kind::Other);
            return;
        }
        let to = if self.masked {
            self.text.len()
        } else {
            self.word_end_after(s.head)
        };
        self.replace(s.head..to, "", Kind::Other);
    }

    /// Deletes the selection and returns it, for a cut.
    pub(crate) fn cut(&mut self) -> String {
        let cut = self.selected_text().to_owned();
        let range = self.selection.range();
        self.replace(range, "", Kind::Other);
        cut
    }

    fn replace(&mut self, range: Range<usize>, new: &str, kind: Kind) {
        if range.is_empty() && new.is_empty() {
            return;
        }
        let before = self.selection;
        let old = self.text[range.clone()].to_owned();
        self.text.replace_range(range.clone(), new);
        self.selection = Selection::caret(range.start + new.len());
        self.record(Edit {
            at: range.start,
            old,
            new: new.to_owned(),
            before,
            after: self.selection,
            kind,
        });
    }

    /// Adds `edit` to the history, joining it to the last step when they are one run of
    /// typing or deleting that the selection has not left.
    fn record(&mut self, edit: Edit) {
        self.redo.clear();
        if self.joins
            && let Some(last) = self.undo.last_mut()
            && last.kind == edit.kind
        {
            let joined = match edit.kind {
                Kind::Typing => {
                    last.at + last.new.len() == edit.at
                        && !last.new.ends_with(char::is_whitespace)
                        && {
                            last.new.push_str(&edit.new);
                            true
                        }
                }
                Kind::DeleteBack => {
                    edit.at + edit.old.len() == last.at && {
                        last.at = edit.at;
                        last.old.insert_str(0, &edit.old);
                        true
                    }
                }
                Kind::DeleteForward => {
                    edit.at == last.at && {
                        last.old.push_str(&edit.old);
                        true
                    }
                }
                Kind::Other => false,
            };
            if joined {
                last.after = edit.after;
                return;
            }
        }
        self.undo.push(edit);
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
        }
        self.joins = true;
    }

    /// Undoes the last step. Nothing happens while a composition is running.
    pub(crate) fn undo(&mut self) -> bool {
        if self.composition.is_some() {
            return false;
        }
        let Some(edit) = self.undo.pop() else {
            return false;
        };
        self.text
            .replace_range(edit.at..edit.at + edit.new.len(), &edit.old);
        self.selection = edit.before;
        self.redo.push(edit);
        self.joins = false;
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        if self.composition.is_some() {
            return false;
        }
        let Some(edit) = self.redo.pop() else {
            return false;
        };
        self.text
            .replace_range(edit.at..edit.at + edit.old.len(), &edit.new);
        self.selection = edit.after;
        self.undo.push(edit);
        self.joins = false;
        true
    }
}

/// Whether a segment of Unicode word segmentation is a word and not spaces or marks.
fn is_word(segment: &str) -> bool {
    segment.chars().any(char::is_alphanumeric)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An editor holding `text` with the caret at `at`.
    fn at(text: &str, at: usize) -> Editor {
        let mut e = Editor::new(text);
        e.move_to(at, false);
        e
    }

    fn typed(e: &mut Editor, text: &str) {
        for c in text.chars() {
            e.replace_text(None, &c.to_string());
        }
    }

    // Graphemes

    #[test]
    fn the_caret_moves_over_whole_graphemes() {
        // e with a combining accent, a family of three joined emoji, a flag, a Japanese word.
        let text = "e\u{301}👨‍👩‍👧🇩🇰日本";
        let mut e = Editor::new(text);
        let mut stops = vec![e.cursor()];
        while e.cursor() > 0 {
            e.left(false);
            stops.push(e.cursor());
        }
        let lens: Vec<usize> = stops.windows(2).map(|w| w[0] - w[1]).collect();
        assert_eq!(lens, [3, 3, 8, 18, 3], "日, 本, flag, family, accented e");
        e.text_start(false);
        e.right(false);
        assert_eq!(e.cursor(), 3, "the accented e is one step");
    }

    #[test]
    fn backspace_removes_a_whole_grapheme() {
        let mut e = Editor::new("a👨‍👩‍👧");
        e.backspace();
        assert_eq!(e.text(), "a");
        let mut e = Editor::new("e\u{301}");
        e.backspace();
        assert_eq!(e.text(), "");
    }

    #[test]
    fn a_selection_snaps_onto_grapheme_boundaries() {
        let mut e = Editor::new("e\u{301}x");
        e.set_selection(1, 2);
        assert_eq!(
            e.selected_range(),
            0..0,
            "inside the accented e both ends snap back"
        );
        e.set_selection(0, 100);
        assert_eq!(e.selected_range(), 0..4, "past the end clamps");
    }

    // UTF-16

    #[test]
    fn utf16_offsets_count_surrogate_pairs() {
        let e = Editor::new("a😀b日");
        assert_eq!(e.offset_to_utf16(0), 0);
        assert_eq!(e.offset_to_utf16(1), 1);
        assert_eq!(e.offset_to_utf16(5), 3, "after the emoji, two units");
        assert_eq!(e.offset_to_utf16(e.text().len()), 5);
        assert_eq!(e.offset_from_utf16(3), 5);
        assert_eq!(e.offset_from_utf16(5), e.text().len());
        assert_eq!(e.offset_from_utf16(99), e.text().len());
        assert_eq!(e.range_from_utf16(&(1..3)), 1..5);
        assert_eq!(e.range_to_utf16(&(1..5)), 1..3);
    }

    // Words

    #[test]
    fn words_move_by_unicode_word_boundaries() {
        let mut e = Editor::new("foo  bar, baz");
        e.text_start(false);
        e.word_right(false);
        assert_eq!(e.cursor(), 3);
        e.word_right(false);
        assert_eq!(e.cursor(), 8, "past the spaces, to the end of bar");
        e.word_right(false);
        assert_eq!(e.cursor(), 13, "past the comma, to the end of baz");
        e.word_left(false);
        assert_eq!(e.cursor(), 10);
        e.word_left(false);
        assert_eq!(e.cursor(), 5);
        e.word_left(false);
        assert_eq!(e.cursor(), 0);
        let e = Editor::new("bar.baz");
        let mut e = e;
        e.text_start(false);
        e.word_right(false);
        assert_eq!(
            e.cursor(),
            7,
            "a dot between letters keeps one word, as in UAX 29"
        );
    }

    #[test]
    fn a_masked_editor_jumps_to_the_ends_instead_of_words() {
        let mut e = at("one two", 4);
        e.set_masked(true);
        e.word_left(false);
        assert_eq!(e.cursor(), 0);
        e.word_right(false);
        assert_eq!(e.cursor(), 7);
        e.move_to(3, false);
        e.backspace_word();
        assert_eq!(e.text(), " two");
    }

    #[test]
    fn a_double_click_selects_a_word() {
        let mut e = Editor::new("one two\nthree");
        e.select_word_at(5);
        assert_eq!(e.selected_text(), "two");
        e.select_word_at(3);
        assert_eq!(e.selected_text(), " ", "on the space between words");
        e.select_word_at(e.text().len());
        assert_eq!(e.selected_text(), "three", "at the very end, the last word");
    }

    #[test]
    fn home_and_end_go_to_the_ends_of_the_line() {
        let mut e = at("ab\ncd", 4);
        e.line_start(false);
        assert_eq!(e.cursor(), 3);
        e.line_end(false);
        assert_eq!(e.cursor(), 5);
        e.move_to(1, false);
        e.line_end(true);
        assert_eq!(e.selected_range(), 1..2, "Shift keeps the anchor");
        e.text_end(false);
        e.text_start(true);
        assert_eq!(e.selected_range(), 0..5);
    }

    #[test]
    fn left_and_right_collapse_a_selection_to_its_side() {
        let mut e = Editor::new("hello");
        e.set_selection(1, 4);
        e.left(false);
        assert_eq!((e.cursor(), e.selection().is_empty()), (1, true));
        e.set_selection(1, 4);
        e.right(false);
        assert_eq!((e.cursor(), e.selection().is_empty()), (4, true));
        e.set_selection(4, 1);
        assert!(e.selection().reversed());
        e.right(true);
        assert_eq!(
            e.selected_range(),
            2..4,
            "the head moved, the anchor stayed"
        );
    }

    // Undo

    #[test]
    fn a_run_of_typing_is_one_step_up_to_a_space() {
        let mut e = Editor::new("");
        typed(&mut e, "hello world");
        assert_eq!(e.text(), "hello world");
        assert!(e.undo());
        assert_eq!(e.text(), "hello ", "the second word was one step");
        assert!(e.undo());
        assert_eq!(e.text(), "", "the first word and its space were one step");
        assert!(!e.undo());
        assert!(e.redo());
        assert_eq!(e.text(), "hello ");
        assert!(e.redo());
        assert_eq!(e.text(), "hello world");
        assert!(!e.redo());
    }

    #[test]
    fn moving_the_caret_ends_a_run() {
        let mut e = Editor::new("");
        typed(&mut e, "ab");
        e.left(false);
        e.right(false);
        typed(&mut e, "cd");
        e.undo();
        assert_eq!(e.text(), "ab");
        e.undo();
        assert_eq!(e.text(), "");
    }

    #[test]
    fn deleting_backwards_and_forwards_each_make_one_step() {
        let mut e = Editor::new("abcdef");
        e.backspace();
        e.backspace();
        assert_eq!(e.text(), "abcd");
        e.undo();
        assert_eq!(e.text(), "abcdef");
        assert_eq!(e.cursor(), 6, "the caret is where it was before");
        e.move_to(1, false);
        e.delete();
        e.delete();
        assert_eq!(e.text(), "adef");
        e.undo();
        assert_eq!(e.text(), "abcdef");
        assert_eq!(e.cursor(), 1);
    }

    #[test]
    fn a_paste_a_cut_and_a_word_are_steps_of_their_own() {
        let mut e = Editor::new("");
        typed(&mut e, "ab");
        e.paste("XY");
        typed(&mut e, "c");
        assert_eq!(e.text(), "abXYc");
        e.undo();
        assert_eq!(e.text(), "abXY");
        e.undo();
        assert_eq!(e.text(), "ab");
        e.select_all();
        assert_eq!(e.cut(), "ab");
        assert_eq!(e.text(), "");
        e.undo();
        assert_eq!(e.text(), "ab");
        assert_eq!(e.selected_range(), 0..2, "the cut text is selected again");
    }

    #[test]
    fn typing_over_a_selection_is_not_joined_to_the_typing_before_it() {
        let mut e = Editor::new("");
        typed(&mut e, "abc");
        e.set_selection(0, 3);
        e.replace_text(None, "x");
        assert_eq!(e.text(), "x");
        e.undo();
        assert_eq!(e.text(), "abc");
    }

    #[test]
    fn a_new_edit_forgets_what_could_be_redone() {
        let mut e = Editor::new("");
        typed(&mut e, "ab");
        e.undo();
        typed(&mut e, "c");
        assert!(!e.redo());
        assert_eq!(e.text(), "c");
    }

    #[test]
    fn the_history_is_bounded() {
        let mut e = Editor::new("");
        for _ in 0..MAX_HISTORY + 10 {
            e.paste("x");
        }
        let mut steps = 0;
        while e.undo() {
            steps += 1;
        }
        assert_eq!(steps, MAX_HISTORY);
        assert_eq!(e.text().len(), 10);
    }

    #[test]
    fn setting_the_text_from_outside_clears_the_history() {
        let mut e = Editor::new("");
        typed(&mut e, "abc");
        e.set_text("é");
        assert_eq!(e.text(), "é");
        assert!(!e.undo());
        assert_eq!(e.cursor(), 2, "the selection was clamped onto the new text");
    }

    // IME

    #[test]
    fn a_composition_is_one_step_when_it_ends() {
        let mut e = Editor::new("a");
        e.mark_text(None, "に", Some(3..3));
        assert_eq!((e.text(), e.marked_range()), ("aに", Some(1..4)));
        e.mark_text(None, "にほ", Some(6..6));
        assert_eq!((e.text(), e.marked_range()), ("aにほ", Some(1..7)));
        assert!(e.is_composing());
        assert!(!e.undo(), "nothing is undone mid-composition");
        e.replace_text(None, "日本");
        assert_eq!(
            (e.text(), e.marked_range(), e.is_composing()),
            ("a日本", None, false)
        );
        assert_eq!(e.cursor(), 7);
        e.undo();
        assert_eq!(e.text(), "a", "the whole composition was one step");
        e.redo();
        assert_eq!(e.text(), "a日本");
    }

    #[test]
    fn a_composition_over_a_selection_restores_it_on_undo() {
        let mut e = Editor::new("hello");
        e.set_selection(1, 4);
        e.mark_text(None, "x", None);
        assert_eq!(e.text(), "hxo");
        e.commit_composition();
        assert_eq!(e.text(), "hxo");
        assert!(!e.is_composing());
        e.undo();
        assert_eq!(e.text(), "hello");
        assert_eq!(e.selected_range(), 1..4);
    }

    #[test]
    fn a_cancelled_composition_leaves_no_step() {
        let mut e = Editor::new("a");
        e.mark_text(None, "に", None);
        e.mark_text(None, "", None);
        assert_eq!(e.text(), "a");
        assert!(!e.is_composing());
        assert!(!e.undo(), "nothing was recorded");
    }

    #[test]
    fn the_selection_inside_pre_edit_text_follows_the_ime() {
        let mut e = Editor::new("");
        e.mark_text(None, "abc", Some(1..2));
        assert_eq!(e.selected_range(), 1..2);
        assert_eq!(e.marked_range(), Some(0..3));
    }

    #[test]
    fn typing_before_any_composition_still_joins_runs() {
        let mut e = Editor::new("");
        e.replace_text(None, "a");
        e.replace_text(None, "b");
        e.undo();
        assert_eq!(e.text(), "");
    }
}
