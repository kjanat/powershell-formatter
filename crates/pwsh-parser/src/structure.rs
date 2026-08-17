//! Shallow structural analysis over the token stream.
//!
//! The formatter does not need an execution-grade AST; it needs to know how
//! delimiters nest, which braces are hashtables versus script blocks, and
//! where statements begin and end. This pass computes exactly that, in one
//! linear walk, without ever re-lexing.

use crate::diag::{Diagnostic, DiagnosticCode, Severity};
use crate::lexer::tokenize;
use crate::span::{LineIndex, Span};
use crate::token::{Keyword, Token, TokenKind};

/// What a delimiter pair encloses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BlockKind {
    /// `{ ... }` script block, named block, or class/enum body.
    ScriptBlock,
    /// `@{ ... }` hashtable literal.
    Hashtable,
    /// `( ... )` grouping, condition, or argument list.
    Paren,
    /// `$( ... )` subexpression.
    SubExpression,
    /// `@( ... )` array expression.
    ArrayExpression,
    /// `[ ... ]` type literal, attribute, or index.
    Bracket,
}

/// Statement classification at the granularity brace placement cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StatementKind {
    /// Starts with a branching/looping/defining keyword.
    KeywordConstruct(Keyword),
    /// `$x = ...` and compound assignments.
    Assignment,
    /// Anything else (commands, expressions, pipelines).
    Pipeline,
}

/// A statement: a contiguous run of significant tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Statement {
    /// Index of the first significant token.
    pub first: u32,
    /// Index of the last significant token (inclusive).
    pub last: u32,
    pub kind: StatementKind,
}

/// A node in the shallow structure tree.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Node {
    pub kind: NodeKind,
    /// Token index of the opening delimiter (`None` for the root).
    pub open: Option<u32>,
    /// Token index of the closing delimiter (`None` for root or when
    /// unclosed at end of input).
    pub close: Option<u32>,
    pub children: Vec<Node>,
    /// Statements directly inside this node (not inside children).
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NodeKind {
    Root,
    Delimited(BlockKind),
}

/// The complete structural analysis of a source text.
#[derive(Debug, Clone)]
pub struct ParseResult {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
    /// For each token index: the index of its matching delimiter, if the
    /// token is a matched open/close delimiter.
    pub matches: Vec<Option<u32>>,
    pub root: Node,
    /// True when the input had unbalanced delimiters or unterminated
    /// protected spans; formatting policy treats this as "preserve input".
    pub is_incomplete: bool,
}

impl ParseResult {
    /// The [`BlockKind`] opened by the token at `idx`, if it is an opener.
    #[must_use]
    pub fn open_kind(&self, idx: usize) -> Option<BlockKind> {
        match self.tokens.get(idx)?.kind {
            TokenKind::LCurly => Some(BlockKind::ScriptBlock),
            TokenKind::AtCurly => Some(BlockKind::Hashtable),
            TokenKind::LParen => Some(BlockKind::Paren),
            TokenKind::AtParen => Some(BlockKind::ArrayExpression),
            TokenKind::DollarParen => Some(BlockKind::SubExpression),
            TokenKind::LBracket => Some(BlockKind::Bracket),
            _ => None,
        }
    }
}

/// Run the scanner and structural analysis in one call.
#[must_use]
pub fn parse(source: &str) -> ParseResult {
    let lex = tokenize(source);
    build(source, lex.tokens, lex.diagnostics)
}

fn closes(open: TokenKind, close: TokenKind) -> bool {
    matches!(
        (open, close),
        (TokenKind::LCurly | TokenKind::AtCurly, TokenKind::RCurly)
            | (
                TokenKind::LParen | TokenKind::AtParen | TokenKind::DollarParen,
                TokenKind::RParen
            )
            | (TokenKind::LBracket, TokenKind::RBracket)
    )
}

fn block_kind(open: TokenKind) -> BlockKind {
    match open {
        TokenKind::AtCurly => BlockKind::Hashtable,
        TokenKind::LParen => BlockKind::Paren,
        TokenKind::AtParen => BlockKind::ArrayExpression,
        TokenKind::DollarParen => BlockKind::SubExpression,
        TokenKind::LBracket => BlockKind::Bracket,
        _ => BlockKind::ScriptBlock,
    }
}

/// Maximum delimiter nesting the structural parser will descend into.
///
/// `parse_into` recurses once per opening delimiter, so unbounded input
/// depth would overflow the call stack — an abort, not a catchable panic.
/// Measured thresholds: ~20k levels on a default 8 MiB thread stack, ~10k
/// on Wasm's 4 MiB, and between 2k and 5k on a 1 MiB stack. PowerShell's
/// own parser gives up well before any of those (`ScriptTooComplicated`,
/// between 10k and 20k levels).
///
/// 1024 keeps a wide margin on the smallest stack we run on while staying
/// far above anything a human writes — real scripts nest a couple of dozen
/// deep, and the deliberately pathological nesting benchmark reaches 480.
const MAX_DEPTH: u32 = 1024;

struct Builder<'a> {
    source: &'a str,
    tokens: &'a [Token],
    diagnostics: Vec<Diagnostic>,
    matches: Vec<Option<u32>>,
    incomplete: bool,
    line_index: Option<LineIndex>,
    depth: u32,
}

impl<'a> Builder<'a> {
    fn diag(&mut self, code: DiagnosticCode, message: &str, span: Span) {
        let idx = self
            .line_index
            .get_or_insert_with(|| LineIndex::new(self.source));
        let position = idx.position(self.source, span.start);
        self.diagnostics.push(Diagnostic::new(
            code,
            Severity::Error,
            message,
            span,
            position,
        ));
    }

    /// Parse tokens `[*pos, end)` into `node` until a closing delimiter for
    /// `node` or `end` is reached.
    fn parse_into(&mut self, node: &mut Node, pos: &mut usize, end: usize) {
        let mut stmt_first: Option<u32> = None;
        let mut stmt_kind = StatementKind::Pipeline;
        let mut stmt_last: u32 = 0;
        let mut prev_significant: Option<TokenKind> = None;

        macro_rules! flush_stmt {
            () => {
                if let Some(first) = stmt_first.take() {
                    node.statements.push(Statement {
                        first,
                        last: stmt_last,
                        kind: core::mem::replace(&mut stmt_kind, StatementKind::Pipeline),
                    });
                }
            };
        }

        while *pos < end {
            let i = *pos;
            let tok = self.tokens[i];
            let kind = tok.kind;

            if kind.is_trivia() {
                if kind == TokenKind::Newline {
                    // A newline ends the statement unless the previous
                    // significant token keeps it open.
                    let continues = matches!(
                        prev_significant,
                        Some(
                            TokenKind::Pipe
                                | TokenKind::AndAnd
                                | TokenKind::OrOr
                                | TokenKind::Comma
                                | TokenKind::Operator(_)
                        )
                    ) || matches!(prev_significant, Some(k) if k.is_open_delimiter());
                    if !continues {
                        flush_stmt!();
                    }
                }
                *pos += 1;
                continue;
            }

            if kind == TokenKind::Semicolon {
                stmt_last = i as u32;
                flush_stmt!();
                *pos += 1;
                prev_significant = Some(kind);
                continue;
            }

            if kind.is_open_delimiter() {
                if self.depth >= MAX_DEPTH {
                    // Too deep to descend safely. Consume the delimiter as an
                    // ordinary token so the scan still advances and stays
                    // lossless, and mark the parse incomplete — the formatter
                    // preserves incomplete input byte-for-byte.
                    if !self.incomplete {
                        self.diag(
                            DiagnosticCode::NestingTooDeep,
                            "delimiters nested too deeply to analyze; formatting skipped",
                            tok.span,
                        );
                    }
                    self.incomplete = true;
                    if stmt_first.is_none() {
                        stmt_first = Some(i as u32);
                    }
                    stmt_last = i as u32;
                    *pos += 1;
                    prev_significant = Some(kind);
                    continue;
                }
                let open_idx = i;
                *pos += 1;
                let mut child = Node {
                    kind: NodeKind::Delimited(block_kind(kind)),
                    open: Some(open_idx as u32),
                    close: None,
                    children: Vec::new(),
                    statements: Vec::new(),
                };
                self.depth += 1;
                self.parse_into(&mut child, pos, end);
                self.depth -= 1;
                if let Some(close_idx) = child.close {
                    self.matches[open_idx] = Some(close_idx);
                    self.matches[close_idx as usize] = Some(open_idx as u32);
                    stmt_last = close_idx;
                } else {
                    self.incomplete = true;
                    self.diag(
                        DiagnosticCode::UnbalancedOpenDelimiter,
                        "delimiter is never closed",
                        tok.span,
                    );
                    stmt_last = (self.tokens.len() - 1) as u32;
                }
                if stmt_first.is_none() {
                    stmt_first = Some(open_idx as u32);
                }
                prev_significant =
                    Some(self.tokens.get(stmt_last as usize).map_or(kind, |t| t.kind));
                node.children.push(child);
                continue;
            }

            if kind.is_close_delimiter() {
                if let NodeKind::Delimited(_) = node.kind {
                    let open_kind = self.tokens[node.open.unwrap_or(0) as usize].kind;
                    if closes(open_kind, kind) {
                        // `stmt_last` already indexes the last significant
                        // token; the token before the close may be trivia.
                        flush_stmt!();
                        node.close = Some(i as u32);
                        *pos += 1;
                        return;
                    }
                }
                // Unbalanced close: recorded by the lexer already; treat it
                // as an ordinary token so downstream stays lossless.
                self.incomplete = true;
                if stmt_first.is_none() {
                    stmt_first = Some(i as u32);
                }
                stmt_last = i as u32;
                *pos += 1;
                prev_significant = Some(kind);
                continue;
            }

            // Ordinary significant token.
            if stmt_first.is_none() {
                stmt_first = Some(i as u32);
                stmt_kind = match kind {
                    TokenKind::Keyword(kw) => StatementKind::KeywordConstruct(kw),
                    TokenKind::Variable | TokenKind::SplattedVariable => {
                        // Assignment if an `=`-family operator appears before
                        // any pipe/semicolon at this nesting level.
                        StatementKind::Pipeline
                    }
                    _ => StatementKind::Pipeline,
                };
            }
            if kind == TokenKind::Operator(crate::token::OperatorKind::Assignment)
                && stmt_kind == StatementKind::Pipeline
            {
                stmt_kind = StatementKind::Assignment;
            }
            stmt_last = i as u32;
            prev_significant = Some(kind);
            *pos += 1;
        }

        flush_stmt!();
        if !matches!(node.kind, NodeKind::Root) && node.close.is_none() {
            // Ran to end of input inside a delimiter.
        }
    }
}

fn build(source: &str, tokens: Vec<Token>, diagnostics: Vec<Diagnostic>) -> ParseResult {
    let had_lex_errors = diagnostics.iter().any(|d| d.severity == Severity::Error);
    let mut builder = Builder {
        source,
        tokens: &tokens,
        diagnostics,
        matches: vec![None; tokens.len()],
        incomplete: had_lex_errors,
        line_index: None,
        depth: 0,
    };
    let mut root = Node {
        kind: NodeKind::Root,
        open: None,
        close: None,
        children: Vec::new(),
        statements: Vec::new(),
    };
    let mut pos = 0;
    let end = tokens.len();
    builder.parse_into(&mut root, &mut pos, end);
    // parse_into returns early on a matched close; the root loop consumes
    // everything, but nested unclosed nodes may have left `pos` mid-stream.
    while pos < end {
        builder.parse_into(&mut root, &mut pos, end);
    }

    let incomplete = builder.incomplete;
    ParseResult {
        matches: builder.matches,
        diagnostics: builder.diagnostics,
        tokens,
        root,
        is_incomplete: incomplete,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_are_symmetric() {
        let r = parse("if ($x) { @{ a = (1 + 2) } }");
        for (i, m) in r.matches.iter().enumerate() {
            if let Some(j) = m {
                assert_eq!(r.matches[*j as usize], Some(i as u32));
            }
        }
        assert!(!r.is_incomplete);
    }

    #[test]
    fn hashtable_node_kind() {
        let r = parse("$x = @{ a = 1 }");
        let child = &r.root.children[0];
        assert_eq!(child.kind, NodeKind::Delimited(BlockKind::Hashtable));
    }

    #[test]
    fn unclosed_brace_marks_incomplete() {
        let r = parse("function f {\n  'x'\n");
        assert!(r.is_incomplete);
        assert!(
            r.diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::UnbalancedOpenDelimiter)
        );
    }

    #[test]
    fn statements_split_on_semicolon_and_newline() {
        let r = parse("$a = 1; $b = 2\n$c = 3");
        assert_eq!(r.root.statements.len(), 3);
        assert_eq!(r.root.statements[0].kind, StatementKind::Assignment);
    }

    /// `Statement.last` must index the last *significant* token even when
    /// trivia sits between it and the closing delimiter (`{ 1 }`: the
    /// statement ends at `1`, not at the space before `}`).
    #[test]
    fn statement_last_skips_trivia_before_close() {
        let r = parse("{ 1 }");
        let child = &r.root.children[0];
        assert_eq!(child.statements.len(), 1);
        let stmt = child.statements[0];
        assert!(!r.tokens[stmt.last as usize].kind.is_trivia());
        assert_eq!(stmt.last, stmt.first);
    }

    #[test]
    fn keyword_statement_kind() {
        let r = parse("if ($x) { 1 } else { 2 }");
        assert!(matches!(
            r.root.statements[0].kind,
            StatementKind::KeywordConstruct(Keyword::If)
        ));
    }

    #[test]
    fn nesting_within_the_limit_still_parses() {
        let depth = (MAX_DEPTH - 1) as usize;
        let src = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
        let r = parse(&src);
        assert!(!r.is_incomplete, "depth {depth} should parse normally");
    }

    /// Deep nesting used to recurse until the call stack aborted the process
    /// (measured: ~20k levels on 8 MiB, under 5k on a 1 MiB Wasm stack).
    /// It must now degrade to an incomplete parse instead.
    #[test]
    fn nesting_past_the_limit_is_reported_not_fatal() {
        for depth in [MAX_DEPTH as usize + 1, 50_000] {
            for src in [
                format!("{}1{}", "(".repeat(depth), ")".repeat(depth)),
                "{".repeat(depth),
                format!("{}1{}", "@(".repeat(depth), ")".repeat(depth)),
            ] {
                let r = parse(&src);
                assert!(r.is_incomplete, "depth {depth} should be incomplete");
                let spans: usize = r.tokens.iter().map(|t| t.span.end - t.span.start).sum();
                assert_eq!(spans, src.len(), "scan stayed lossless at depth {depth}");
            }
        }
    }
}
