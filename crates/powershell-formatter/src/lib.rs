//! Host-independent PowerShell formatting engine.
//!
//! One lexical/structural analysis, one set of layout decisions, one render:
//! no PowerShell process, no runspace, no CLR. Behavior follows
//! PSScriptAnalyzer's `Invoke-Formatter` (see `docs/formatting.md` for the
//! documented, intentional divergences).
//!
//! ```
//! use powershell_formatter::{FormatOptions, format};
//!
//! let result = format("function foo {\n\"hello\"\n  }", &FormatOptions::default());
//! assert_eq!(result.text, "function foo {\n    \"hello\"\n}");
//! ```

mod catalog;
mod engine;
mod options;
mod phases;

pub use catalog::{CommandCatalog, JsonCatalog};
pub use options::{
    BraceStyle, BranchKeywordPlacement, EndOfLine, FormatOptions, PipelineIndentation,
};
pub use powershell_parser::{Diagnostic, DiagnosticCode, Position, Severity, Span};

use powershell_parser::{LineIndex, parse};

/// The outcome of a formatting request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FormatResult {
    /// The formatted source (identical to the input when formatting was
    /// skipped or made no changes).
    pub text: String,
    /// Diagnostics collected during scanning/analysis.
    pub diagnostics: Vec<Diagnostic>,
    /// False when structural/lexical problems made formatting unsafe and
    /// the input was preserved unchanged.
    pub formatted: bool,
}

/// A line/column range in `Invoke-Formatter -Range` style: 1-based start
/// line/column and end line/column (end-exclusive columns, matching
/// PowerShell extents).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatRange {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Format a complete PowerShell source string with the default (or given)
/// options and no command catalog.
#[must_use]
pub fn format(source: &str, options: &FormatOptions) -> FormatResult {
    format_impl(source, options, None, None)
}

/// Format with an injected command catalog for command/parameter casing.
#[must_use]
pub fn format_with_catalog(
    source: &str,
    options: &FormatOptions,
    catalog: &dyn CommandCatalog,
) -> FormatResult {
    format_impl(source, options, Some(catalog), None)
}

/// Format only the given line/column range, leaving the rest byte-identical.
///
/// Matching `Invoke-Formatter -Range`, only whole corrections inside the
/// range are applied; the range does not expand beyond what is needed.
#[must_use]
pub fn format_range(source: &str, options: &FormatOptions, range: FormatRange) -> FormatResult {
    format_impl(source, options, None, Some(range))
}

/// Range formatting with a catalog.
#[must_use]
pub fn format_range_with_catalog(
    source: &str,
    options: &FormatOptions,
    catalog: &dyn CommandCatalog,
    range: FormatRange,
) -> FormatResult {
    format_impl(source, options, Some(catalog), Some(range))
}

fn format_impl(
    source: &str,
    options: &FormatOptions,
    catalog: Option<&dyn CommandCatalog>,
    range: Option<FormatRange>,
) -> FormatResult {
    let mut result = format_once(source, options, catalog, range);
    // Idempotence is a hard invariant, but moving tokens across lines can
    // change mode-dependent token classes (`=` after a command name is an
    // argument; at statement start it is an operator), which later phases
    // read. Whole-file formatting therefore verifies the output is a
    // fixpoint, stepping once more when it is not. Range formatting is
    // exempt: the range names coordinates in the *original* text, so
    // re-running it against shifted text would format the wrong region.
    if range.is_some() || !result.formatted || result.text == source {
        return result;
    }
    for _ in 0..2 {
        let next = format_once(&result.text, options, catalog, None);
        if next.formatted && next.text == result.text {
            return result;
        }
        if !next.formatted {
            break;
        }
        result.text = next.text;
    }
    // No stable layout within the pass budget (or an intermediate result
    // stopped formatting cleanly): preserve the input.
    let index = LineIndex::new(source);
    let mut diagnostics = result.diagnostics;
    diagnostics.push(Diagnostic::new(
        DiagnosticCode::FormattingSkipped,
        Severity::Warning,
        "formatting did not reach a stable result; input preserved unchanged",
        Span::new(0, 0),
        index.position(source, 0),
    ));
    FormatResult {
        text: source.to_owned(),
        diagnostics,
        formatted: false,
    }
}

fn format_once(
    source: &str,
    options: &FormatOptions,
    catalog: Option<&dyn CommandCatalog>,
    range: Option<FormatRange>,
) -> FormatResult {
    let analysis = parse(source);

    // Safety policy: structurally uncertain input is preserved untouched.
    if analysis.is_incomplete {
        let mut diagnostics = analysis.diagnostics;
        let index = LineIndex::new(source);
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::FormattingSkipped,
            Severity::Info,
            "input has unbalanced or unterminated syntax; formatting skipped to avoid corrupting it",
            Span::new(0, 0),
            index.position(source, 0),
        ));
        return FormatResult {
            text: source.to_owned(),
            diagnostics,
            formatted: false,
        };
    }

    let byte_range = range.map(|r| {
        let index = LineIndex::new(source);
        let start = index.offset(
            source,
            Position {
                line: r.start_line,
                column: r.start_column,
            },
        );
        let end = index.offset(
            source,
            Position {
                line: r.end_line,
                column: r.end_column,
            },
        );
        Span::new(start.min(end), end.max(start))
    });

    let mut engine = engine::Engine::new(source, &analysis, options);
    engine::run_phases(&mut engine, catalog);
    let mut text = engine.render(byte_range);

    if range.is_none() {
        apply_final_newline(&mut text, options, engine.newline);
    }

    // Safety net: layout changes must never alter protected content. On
    // structurally odd (yet balanced) input, moving tokens across lines can
    // flip mode-dependent lexing and swallow a comment or string into a
    // bare word; verify by re-scanning the output and preserve the input
    // when anything protected changed.
    if text != source && !protected_content_matches(source, &text) {
        let mut diagnostics = analysis.diagnostics;
        let index = LineIndex::new(source);
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::PreservationCheckFailed,
            Severity::Warning,
            "formatting would have altered protected content; input preserved unchanged",
            Span::new(0, 0),
            index.position(source, 0),
        ));
        return FormatResult {
            text: source.to_owned(),
            diagnostics,
            formatted: false,
        };
    }

    FormatResult {
        text,
        diagnostics: analysis.diagnostics,
        formatted: true,
    }
}

/// String, here-string, and comment token texts must survive formatting
/// byte-for-byte, in order.
fn protected_content_matches(source: &str, formatted: &str) -> bool {
    let protected = |src: &str| -> Vec<String> {
        powershell_parser::tokenize(src)
            .tokens
            .iter()
            .filter(|t| t.kind.is_string() || t.kind.is_comment())
            .map(|t| t.text(src).to_owned())
            .collect()
    };
    protected(source) == protected(formatted)
}

fn apply_final_newline(text: &mut String, options: &FormatOptions, newline: &str) {
    match options.final_newline {
        None => {}
        Some(true) => {
            let had_content = !text.is_empty();
            while text.ends_with('\n') || text.ends_with('\r') {
                text.pop();
            }
            // `had_content`, not the post-trim check: a newline-only file
            // must normalize to one final newline, not be emptied.
            if !text.is_empty() || had_content {
                text.push_str(newline);
            }
        }
        Some(false) => {
            while text.ends_with('\n') || text.ends_with('\r') {
                text.pop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(src: &str) -> String {
        format(src, &FormatOptions::default()).text
    }

    #[test]
    fn baseline_function() {
        assert_eq!(
            fmt("function foo {\n\"hello\"\n  }"),
            "function foo {\n    \"hello\"\n}"
        );
    }

    #[test]
    fn compact_if_else() {
        // Oracle-verified: Invoke-Formatter 1.25 produces exactly this.
        let out = fmt("IF($x-EQ 1){'yes'}ELSE{'no'}");
        assert_eq!(out, "if ($x -eq 1) { 'yes' }else { 'no' }");
    }

    #[test]
    fn hashtable_not_scriptblock() {
        let src = "$x = @{ one = 1; two = 2 }";
        assert_eq!(fmt(src), src);
    }

    #[test]
    fn here_string_untouched() {
        let src = "$x = @'\ncontent belongs exactly here\n'@";
        assert_eq!(fmt(src), src);
    }

    #[test]
    fn incomplete_input_preserved() {
        let src = "function f {\n  'x'\n";
        let res = format(src, &FormatOptions::default());
        assert_eq!(res.text, src);
        assert!(!res.formatted);
        assert!(
            res.diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::FormattingSkipped)
        );
    }

    #[test]
    fn idempotent_on_samples() {
        for src in [
            "function foo {\n\"hello\"\n  }",
            "IF($x-EQ 1){'yes'}ELSE{'no'}",
            "$x = @{ one = 1; two = 2 }",
            "foreach ($x in 1..10) {\n$x\n}",
            "Get-Process |\nWhere-Object CPU |\nSelect-Object Name",
        ] {
            let once = fmt(src);
            let twice = fmt(&once);
            assert_eq!(once, twice, "not idempotent for {src:?}");
        }
    }

    #[test]
    fn fuzz_regression_comment_never_swallowed() {
        // Fuzz-found (minimized): joining a line-leading operator to the
        // previous line flipped the lexer into argument mode, absorbing the
        // `#` comment into a bare word. The preservation check must catch
        // this and preserve the input.
        let src = ">\n/s#";
        let res = format(src, &FormatOptions::default());
        assert_eq!(
            protected(&res.text),
            protected(src),
            "comments must never be swallowed"
        );
        let twice = format(&res.text, &FormatOptions::default());
        assert_eq!(res.text, twice.text);
    }

    fn protected(src: &str) -> Vec<String> {
        powershell_parser::tokenize(src)
            .tokens
            .iter()
            .filter(|t| t.kind.is_comment() || t.kind.is_string())
            .map(|t| t.text(src).to_owned())
            .collect()
    }

    #[test]
    fn long_pipelines_reflow_at_line_width() {
        let src = "Get-Process | Where-Object { $_.CPU -gt 5 } | Sort-Object CPU | Select-Object -First 3";
        let narrow = FormatOptions {
            line_width: 60,
            ..FormatOptions::default()
        };
        let out = format(src, &narrow).text;
        assert_eq!(
            out,
            "Get-Process |\n    Where-Object { $_.CPU -gt 5 } |\n    Sort-Object CPU |\n    Select-Object -First 3"
        );
        // Idempotent, and disabled when lineWidth is 0.
        assert_eq!(format(&out, &narrow).text, out);
        let off = FormatOptions {
            line_width: 0,
            ..FormatOptions::default()
        };
        assert_eq!(format(src, &off).text, src);
    }

    #[test]
    fn range_formatting_limits_changes() {
        let src = "if($a){'x'}\nif($b){'y'}";
        let out = format_range(
            src,
            &FormatOptions::default(),
            FormatRange {
                start_line: 2,
                start_column: 1,
                end_line: 2,
                end_column: 12,
            },
        )
        .text;
        // Line 1 untouched; every correction inside the range applies.
        // (Intentional divergence: Invoke-Formatter's iterative fixer drops
        // corrections that drift past the range as earlier edits grow the
        // line — see docs/formatting.md.)
        assert_eq!(out, "if($a){'x'}\nif ($b) { 'y' }");
    }

    /// `finalNewline: true` on a whitespace-only file must normalize it to
    /// one newline, not empty it (regression: the post-trim emptiness check
    /// used to suppress the push).
    #[test]
    fn final_newline_keeps_whitespace_only_files() {
        let opts = FormatOptions {
            final_newline: Some(true),
            ..FormatOptions::default()
        };
        // The single kept newline uses the input's detected style.
        for (src, expected) in [("\n", "\n"), ("\r\n", "\r\n"), ("\n\n\n", "\n")] {
            let result = format(src, &opts);
            assert!(result.formatted, "src {src:?}");
            assert_eq!(result.text, expected, "src {src:?}");
        }
        assert_eq!(format("", &opts).text, "");
    }

    /// With `newlineAfterOpenBrace: false`, relocating a comment past the
    /// `{` would let the line comment swallow the code that stays on the
    /// same line; the brace must stay put instead of tripping the
    /// preservation check into a silent skip.
    #[test]
    fn brace_comment_relocation_respects_same_line_content() {
        let opts = FormatOptions {
            newline_after_open_brace: false,
            ignore_one_line_block: false,
            ..FormatOptions::default()
        };
        let src = "if ($x) # note\n{ 1 }";
        let result = format(src, &opts);
        assert!(result.formatted, "must format, not preservation-skip");
        let twice = format(&result.text, &opts);
        assert_eq!(result.text, twice.text);
    }
}
