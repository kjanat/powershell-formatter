//! PowerShell lexical scanning and shallow structural syntax.
//!
//! This crate provides a **lossless, formatter-oriented** view of PowerShell
//! source: every input byte belongs to exactly one token, strings and
//! comments are opaque protected spans, and a shallow structural pass groups
//! tokens into the nests and statements a formatter needs. It deliberately
//! does not build an execution-capable AST and never depends on a PowerShell
//! runtime.

mod diag;
mod lexer;
mod span;
mod structure;
mod token;

pub use diag::{Diagnostic, DiagnosticCode, Severity};
pub use lexer::{LexOutput, tokenize};
pub use span::{LineIndex, Position, Span};
pub use structure::{BlockKind, Node, NodeKind, ParseResult, StatementKind, parse};
pub use token::{Keyword, OperatorKind, Token, TokenFlags, TokenKind};

#[cfg(test)]
mod tests {
    use super::*;

    /// Concatenating all token texts must reproduce the source exactly.
    fn assert_lossless(source: &str) {
        let out = tokenize(source);
        let mut rebuilt = String::new();
        let mut prev_end = 0;
        for t in &out.tokens {
            assert_eq!(t.span.start, prev_end, "gap before {t:?} in {source:?}");
            rebuilt.push_str(t.text(source));
            prev_end = t.span.end;
        }
        assert_eq!(prev_end, source.len(), "missing tail in {source:?}");
        assert_eq!(rebuilt, source);
    }

    #[test]
    fn lossless_on_assorted_sources() {
        for src in [
            "",
            "$x = 1",
            "function foo {\n\"hello\"\n  }",
            "IF($x-EQ 1){'yes'}ELSE{'no'}",
            "$x = @{ one = 1; two = 2 }",
            "$x = @'\ncontent belongs exactly here\n'@",
            "Get-ChildItem -Path C:\\Windows | Where-Object { $_.Length -gt 1kb }",
            "\"interp $($x + 1) end\"",
            "ls | ? { $_ } | % Name",
            "1..5 | ForEach-Object { $_ * 2 }",
            "cmd --% raw & | stuff\nnext",
            "[int]$x = 5; $y = $x -as [string]",
            "écho hé\u{FEFF}",
            "$x = \"héllo 🎉\"",
        ] {
            assert_lossless(src);
        }
    }

    #[test]
    fn hashtable_vs_scriptblock_braces() {
        let out = tokenize("$x = @{ a = 1 }; & { 'b' }");
        let kinds: Vec<TokenKind> = out
            .tokens
            .iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| t.kind)
            .collect();
        assert!(kinds.contains(&TokenKind::AtCurly));
        assert!(kinds.contains(&TokenKind::LCurly));
    }

    #[test]
    fn keywords_only_in_command_position() {
        let out = tokenize("if ($true) { }\nWrite-Output if");
        let toks: Vec<(TokenKind, &str)> = out
            .tokens
            .iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| (t.kind, t.text("if ($true) { }\nWrite-Output if")))
            .collect();
        assert_eq!(toks[0].0, TokenKind::Keyword(Keyword::If));
        // Final `if` is a command argument, not a keyword.
        assert_eq!(toks.last().unwrap().0, TokenKind::Generic);
    }
}
