//! Byte spans and line/column translation.

use core::fmt;

/// A half-open byte range `[start, end)` into UTF-8 source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct Span {
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        debug_assert!(start <= end, "inverted span");
        Self { start, end }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.end - self.start
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Slice `source` with this span. Panics only if the span is out of
    /// bounds or splits a UTF-8 sequence, which would be a lexer bug.
    #[must_use]
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }

    #[must_use]
    pub const fn contains(&self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }

    /// True when the two spans share at least one byte, or when an empty
    /// span (a caret position) sits inside the other — its start boundary
    /// included — or coincides with another empty span.
    #[must_use]
    pub const fn overlaps(&self, other: &Span) -> bool {
        match (self.is_empty(), other.is_empty()) {
            (true, true) => self.start == other.start,
            (true, false) => other.contains(self.start),
            (false, true) => self.contains(other.start),
            (false, false) => self.start < other.end && other.start < self.end,
        }
    }

    #[must_use]
    pub fn join(&self, other: &Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// A 1-based line/column position.
///
/// `column` counts UTF-16 code units, matching the semantics of PowerShell's
/// `IScriptExtent` (which exposes .NET string indices). This was verified
/// differentially against `pwsh` 7.5: a `🎉` (astral) character advances the
/// reported column by two, and `é` by one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

/// Precomputed line-start table for O(log n) offset → line/column mapping.
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// Byte offset of the first byte of each line. Always starts with 0.
    line_starts: Vec<usize>,
}

impl LineIndex {
    /// Build the index. Line breaks are LF, CRLF, or CR (matching the
    /// tokenizer's newline handling).
    #[must_use]
    pub fn new(source: &str) -> Self {
        let bytes = source.as_bytes();
        let mut line_starts = vec![0];
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\n' => {
                    line_starts.push(i + 1);
                    i += 1;
                }
                b'\r' => {
                    if bytes.get(i + 1) == Some(&b'\n') {
                        line_starts.push(i + 2);
                        i += 2;
                    } else {
                        line_starts.push(i + 1);
                        i += 1;
                    }
                }
                _ => i += 1,
            }
        }
        Self { line_starts }
    }

    /// Number of lines (at least 1, even for empty input).
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Byte offset at which `line` (0-based) starts. Out-of-range lines clamp
    /// to the last line start.
    #[must_use]
    pub fn line_start(&self, line: usize) -> usize {
        let idx = line.min(self.line_starts.len() - 1);
        self.line_starts[idx]
    }

    /// 0-based line containing the byte `offset`.
    #[must_use]
    pub fn line_of(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next - 1,
        }
    }

    /// Translate a byte offset into a 1-based line and 1-based UTF-16 column.
    ///
    /// `offset` must lie on a character boundary of `source`.
    #[must_use]
    pub fn position(&self, source: &str, offset: usize) -> Position {
        let offset = offset.min(source.len());
        let line = self.line_of(offset);
        let start = self.line_starts[line];
        let column: usize = source[start..offset].chars().map(char::len_utf16).sum();
        Position {
            line: u32::try_from(line + 1).unwrap_or(u32::MAX),
            column: u32::try_from(column + 1).unwrap_or(u32::MAX),
        }
    }

    /// Translate a 1-based line and 1-based UTF-16 column into a byte offset.
    /// Positions past the end of a line clamp to the line end; lines past the
    /// end of the file clamp to the source end.
    #[must_use]
    pub fn offset(&self, source: &str, position: Position) -> usize {
        if position.line == 0 {
            return 0;
        }
        let line = position.line as usize - 1;
        if line >= self.line_starts.len() {
            return source.len();
        }
        let start = self.line_starts[line];
        let end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(source.len());
        let mut remaining = position.column.saturating_sub(1) as usize;
        let mut offset = start;
        for ch in source[start..end].chars() {
            if remaining == 0 || ch == '\n' || ch == '\r' {
                break;
            }
            let units = ch.len_utf16();
            if units > remaining {
                break;
            }
            remaining -= units;
            offset += ch.len_utf8();
        }
        offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_index_handles_mixed_newlines() {
        let src = "a\nb\r\nc\rd";
        let idx = LineIndex::new(src);
        assert_eq!(idx.line_count(), 4);
        assert_eq!(idx.line_of(0), 0);
        assert_eq!(idx.line_of(2), 1);
        assert_eq!(idx.line_of(5), 2);
        assert_eq!(idx.line_of(7), 3);
    }

    #[test]
    fn position_counts_utf16_units() {
        let src = "é🎉x";
        let idx = LineIndex::new(src);
        // é = 2 bytes, 1 utf16 unit; 🎉 = 4 bytes, 2 utf16 units.
        assert_eq!(idx.position(src, 0), Position { line: 1, column: 1 });
        assert_eq!(idx.position(src, 2), Position { line: 1, column: 2 });
        assert_eq!(idx.position(src, 6), Position { line: 1, column: 4 });
    }

    #[test]
    fn offset_round_trips() {
        let src = "ab\ncé🎉\nxyz";
        let idx = LineIndex::new(src);
        for (i, _) in src.char_indices() {
            let pos = idx.position(src, i);
            assert_eq!(idx.offset(src, pos), i, "offset {i}");
        }
    }

    #[test]
    fn offset_clamps() {
        let src = "ab\ncd";
        let idx = LineIndex::new(src);
        assert_eq!(
            idx.offset(
                src,
                Position {
                    line: 1,
                    column: 99
                }
            ),
            2
        );
        assert_eq!(idx.offset(src, Position { line: 9, column: 1 }), 5);
    }

    #[test]
    fn span_overlap() {
        assert!(Span::new(0, 4).overlaps(&Span::new(3, 8)));
        assert!(!Span::new(0, 4).overlaps(&Span::new(4, 8)));
        assert!(Span::new(2, 2).overlaps(&Span::new(0, 4)));
        assert!(Span::new(0, 4).overlaps(&Span::new(2, 2)));
    }
}
