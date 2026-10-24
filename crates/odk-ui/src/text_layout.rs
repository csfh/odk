//! Where the text of a field is on screen (ADR 0038): the lines GPUI shaped, cut into rows
//! where they wrap, and the geometry between offsets in the text and points on the screen.
//!
//! An `Input` has one row; a `Textarea` has as many as its text and width need. Offsets in and
//! out are bytes of the editor's text. Inside, they are bytes of what is drawn (`Shown`), which
//! differs for a masked field and for the tabs and line breaks an `Input` draws as spaces.
//! Where a row wraps, the offset on the break belongs to the row that starts there.

use std::ops::Range;

use gpui::{App, Bounds, Pixels, Point, TextAlign, Window, WrappedLine, point, px, size};
use unicode_segmentation::UnicodeSegmentation;

/// What a masked field shows for each grapheme of its text.
pub(crate) const MASK: &str = "•";

/// The text as drawn, and where each grapheme boundary of the editor's text falls in it.
pub(crate) struct Shown {
    pub(crate) text: String,
    /// `(offset in the editor's text, offset in `text`)` at every grapheme boundary, the end of
    /// the text last.
    boundaries: Vec<(usize, usize)>,
}

impl Shown {
    /// `masked` draws a dot for each grapheme; line breaks stay breaks in a `multiline` field.
    /// Otherwise the text is drawn as it is, but for tabs, which become a space, and line
    /// breaks in a one-line field, which do too.
    pub(crate) fn new(text: &str, masked: bool, multiline: bool) -> Self {
        let mut shown = String::with_capacity(text.len());
        let mut boundaries = Vec::with_capacity(text.len() + 1);
        for (at, grapheme) in text.grapheme_indices(true) {
            boundaries.push((at, shown.len()));
            if masked {
                shown.push_str(if multiline && grapheme == "\n" {
                    "\n"
                } else {
                    MASK
                });
            } else {
                for c in grapheme.chars() {
                    shown.push(match c {
                        '\n' if multiline => '\n',
                        '\n' | '\r' | '\t' => ' ',
                        c => c,
                    });
                }
            }
        }
        boundaries.push((text.len(), shown.len()));
        Self {
            text: shown,
            boundaries,
        }
    }

    /// The offset in what is drawn for `offset` in the text, or for the boundary before it if
    /// it is inside a grapheme.
    pub(crate) fn display(&self, offset: usize) -> usize {
        match self.boundaries.binary_search_by_key(&offset, |b| b.0) {
            Ok(i) => self.boundaries[i].1,
            Err(i) => self.boundaries[i.saturating_sub(1)].1,
        }
    }
}

/// One row of drawn text: a line, or the part of a wrapped line between two wraps.
struct Row {
    line: usize,
    /// Offsets in what is drawn. Every offset from `start` up to `end` belongs to this row,
    /// `end` itself only if it is the last row of its line (otherwise it starts the next row).
    start: usize,
    end: usize,
    last: bool,
    /// Where the row starts across the whole unwrapped line.
    x0: Pixels,
    /// The top of the row, from the top of the text.
    y: Pixels,
}

/// A line as GPUI shaped it, and where it is.
struct Line {
    shaped: WrappedLine,
    /// Its offset in what is drawn.
    start: usize,
    top: Pixels,
}

pub(crate) struct TextLayout {
    shown: Shown,
    lines: Vec<Line>,
    rows: Vec<Row>,
    line_height: Pixels,
}

impl TextLayout {
    /// `lines` is `shown.text` shaped, one entry for each line of it.
    pub(crate) fn new(shown: Shown, lines: Vec<WrappedLine>, line_height: Pixels) -> Self {
        let mut placed = Vec::with_capacity(lines.len());
        let mut rows: Vec<Row> = Vec::new();
        let mut start = 0;
        for (n, shaped) in lines.into_iter().enumerate() {
            let starts: Vec<usize> = std::iter::once(0)
                .chain(
                    shaped
                        .wrap_boundaries()
                        .iter()
                        .map(|b| shaped.runs()[b.run_ix].glyphs[b.glyph_ix].index),
                )
                .collect();
            let top = line_height * rows.len();
            for (k, &from) in starts.iter().enumerate() {
                let last = k + 1 == starts.len();
                rows.push(Row {
                    line: n,
                    start: start + from,
                    end: start + starts.get(k + 1).copied().unwrap_or(shaped.len()),
                    last,
                    x0: shaped.unwrapped_layout.x_for_index(from),
                    y: line_height * rows.len(),
                });
            }
            // The line break between two lines is one byte.
            let next = start + shaped.len() + 1;
            placed.push(Line { shaped, start, top });
            start = next;
        }
        Self {
            shown,
            lines: placed,
            rows,
            line_height,
        }
    }

    /// The text as drawn, for tests: dots for a masked field.
    #[cfg(test)]
    pub(crate) fn shown_text(&self) -> &str {
        &self.shown.text
    }

    pub(crate) fn line_height(&self) -> Pixels {
        self.line_height
    }

    /// The height of all the text.
    pub(crate) fn height(&self) -> Pixels {
        self.line_height * self.rows.len()
    }

    /// The width of the longest line, unwrapped.
    pub(crate) fn width(&self) -> Pixels {
        self.lines
            .iter()
            .map(|l| l.shaped.unwrapped_layout.width)
            .fold(px(0.0), Pixels::max)
    }

    /// The row an offset in what is drawn belongs to.
    fn row_of(&self, shown: usize) -> &Row {
        let at = self.rows.partition_point(|r| r.start <= shown);
        &self.rows[at.saturating_sub(1)]
    }

    fn x_in(&self, row: &Row, shown: usize) -> Pixels {
        let line = &self.lines[row.line];
        line.shaped.unwrapped_layout.x_for_index(shown - line.start) - row.x0
    }

    /// The top left of the character at `offset`, from the top left of the text.
    pub(crate) fn point_of(&self, offset: usize) -> Point<Pixels> {
        let shown = self.shown.display(offset);
        let row = self.row_of(shown);
        point(self.x_in(row, shown), row.y)
    }

    /// The grapheme boundary of the text nearest `at`, from the top left of the text. GPUI's
    /// own lookup compares only where glyphs start, so a click on the left half of the last
    /// character would land after it.
    pub(crate) fn offset_at(&self, at: Point<Pixels>) -> usize {
        let last = self.rows.len() - 1;
        let row = &self.rows[((at.y / self.line_height).floor().max(0.0) as usize).min(last)];
        let line = &self.lines[row.line];
        let target = at.x + row.x0;
        let boundaries = &self.shown.boundaries;
        let from = boundaries.partition_point(|b| b.1 < row.start);
        let to = boundaries.partition_point(|b| {
            if row.last {
                b.1 <= row.end
            } else {
                b.1 < row.end
            }
        });
        let to = to.max(from + 1).min(boundaries.len());
        let from = from.min(to - 1);
        let mut nearest: Option<(usize, Pixels)> = None;
        for &(offset, shown) in &boundaries[from..to] {
            let distance =
                (line.shaped.unwrapped_layout.x_for_index(shown - line.start) - target).abs();
            if nearest.is_none_or(|(_, best)| distance < best) {
                nearest = Some((offset, distance));
            }
        }
        nearest.map_or(0, |(offset, _)| offset)
    }

    /// One box for each row `range` touches, from the top left of the text. A line break the
    /// range covers is drawn `break_width` wide, so that a selected empty line shows.
    pub(crate) fn rects_of(&self, range: Range<usize>, break_width: Pixels) -> Vec<Bounds<Pixels>> {
        let (from, to) = (
            self.shown.display(range.start),
            self.shown.display(range.end),
        );
        let first = self
            .rows
            .partition_point(|r| r.start <= from)
            .saturating_sub(1);
        let mut rects = Vec::new();
        for row in &self.rows[first..] {
            if row.start > to {
                break;
            }
            let (a, b) = (from.max(row.start), to.min(row.end));
            let mut width = if b > a {
                self.x_in(row, b) - self.x_in(row, a)
            } else {
                px(0.0)
            };
            if row.last && to > row.end {
                width += break_width;
            }
            if width > px(0.0) {
                rects.push(Bounds::new(
                    point(self.x_in(row, a), row.y),
                    size(width, self.line_height),
                ));
            }
        }
        rects
    }

    /// Paints the lines that show in `view`, a span down from the top of the text, with the
    /// text's top left at `origin`.
    pub(crate) fn paint(
        &self,
        origin: Point<Pixels>,
        view: Range<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        for line in &self.lines {
            let height = line.shaped.size(self.line_height).height;
            if line.top + height < view.start || line.top > view.end {
                continue;
            }
            line.shaped
                .paint(
                    point(origin.x, origin.y + line.top),
                    self.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_masked_text_draws_a_dot_for_each_grapheme() {
        let text = "e\u{301}👨‍👩‍👧x";
        let shown = Shown::new(text, true, false);
        assert_eq!(shown.text, "•••");
        assert_eq!(shown.display(0), 0);
        assert_eq!(shown.display(3), MASK.len());
        assert_eq!(shown.display(text.len()), 3 * MASK.len());
    }

    #[test]
    fn a_masked_text_keeps_the_breaks_of_a_multiline_field() {
        let shown = Shown::new("ab\ncd", true, true);
        assert_eq!(shown.text, "••\n••");
        assert_eq!(shown.display(3), 2 * MASK.len() + 1);
        assert_eq!(Shown::new("ab\ncd", true, false).text, "•••••");
    }

    #[test]
    fn line_breaks_and_tabs_are_drawn_as_spaces_in_one_line_without_moving_anything() {
        let text = "a\nb\r\nc\td";
        let shown = Shown::new(text, false, false);
        assert_eq!(shown.text, "a b  c d");
        assert_eq!(shown.text.len(), text.len());
        assert_eq!(shown.display(3), 3);
    }

    #[test]
    fn a_multiline_field_keeps_its_line_breaks() {
        let shown = Shown::new("a\nb\tc", false, true);
        assert_eq!(shown.text, "a\nb c");
    }

    #[test]
    fn an_offset_inside_a_grapheme_falls_back_to_the_boundary_before_it() {
        let shown = Shown::new("a😀b", false, false);
        assert_eq!(shown.display(3), 1);
    }
}
