//! PowerShell lexical scanning and shallow structural syntax.
//!
//! This crate deliberately does not execute PowerShell or depend on a PowerShell runtime.

/// A byte range in UTF-8 source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// Token categories required by the formatter.
///
/// The set is intentionally formatter-oriented rather than a clone of PowerShell's `TokenKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TokenKind {
    Source,
}

/// A lexical token borrowing no source memory so it can cross adapter boundaries cheaply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// Scan PowerShell source into formatter-oriented tokens.
///
/// The initial scaffold returns a single source token. The real scanner will replace this without
/// changing consumers' ownership model.
#[must_use]
pub fn scan(source: &str) -> Vec<Token> {
    if source.is_empty() {
        Vec::new()
    } else {
        vec![Token {
            kind: TokenKind::Source,
            span: Span::new(0, source.len()),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_span_uses_utf8_byte_offsets() {
        let source = "$x = 'hé'";
        let tokens = scan(source);

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].span, Span::new(0, source.len()));
    }
}
