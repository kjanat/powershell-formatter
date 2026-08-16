//! Diagnostics shared by the scanner, structural parser, and formatter.

use crate::span::{Position, Span};

/// Machine-readable diagnostic codes. Codes are stable public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DiagnosticCode {
    /// A string or here-string ran to end of input without its terminator.
    UnterminatedString,
    /// A `<# ... #>` comment ran to end of input.
    UnterminatedComment,
    /// A `${...}` variable ran to end of input.
    UnterminatedVariable,
    /// A closing delimiter had no matching opener.
    UnbalancedCloseDelimiter,
    /// An opening delimiter was never closed.
    UnbalancedOpenDelimiter,
    /// A byte sequence could not be classified.
    UnrecognizedToken,
    /// Formatting was skipped because the input could not be analyzed safely.
    FormattingSkipped,
    /// An invalid formatting range was requested.
    InvalidRange,
}

impl DiagnosticCode {
    /// Short stable string form, e.g. for CLI/JSON output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            DiagnosticCode::UnterminatedString => "unterminated-string",
            DiagnosticCode::UnterminatedComment => "unterminated-comment",
            DiagnosticCode::UnterminatedVariable => "unterminated-variable",
            DiagnosticCode::UnbalancedCloseDelimiter => "unbalanced-close-delimiter",
            DiagnosticCode::UnbalancedOpenDelimiter => "unbalanced-open-delimiter",
            DiagnosticCode::UnrecognizedToken => "unrecognized-token",
            DiagnosticCode::FormattingSkipped => "formatting-skipped",
            DiagnosticCode::InvalidRange => "invalid-range",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Severity {
    /// Formatting proceeded; the note is informational.
    Info,
    /// Formatting proceeded but the input looks suspicious.
    Warning,
    /// Formatting (or part of it) was not performed.
    Error,
}

/// A diagnostic with both byte-span and line/column locations.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub message: String,
    /// Byte span in the original source.
    pub span: Span,
    /// 1-based line/UTF-16-column of `span.start`.
    pub position: Position,
}

impl Diagnostic {
    #[must_use]
    pub fn new(
        code: DiagnosticCode,
        severity: Severity,
        message: impl Into<String>,
        span: Span,
        position: Position,
    ) -> Self {
        Self {
            code,
            severity,
            message: message.into(),
            span,
            position,
        }
    }
}
