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
///
/// Parsing, [`Clone`], and [`Drop`] are all iterative and safe at any
/// nesting depth (the tree can legitimately be tens of thousands of levels
/// deep — see [`MAX_DEPTH`]). The derived `Debug` and `PartialEq` still
/// recurse per level; they exist for tests over shallow trees.
#[derive(Debug, PartialEq, Eq)]
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

/// The derived drop glue frees `children` recursively — one call-stack
/// frame per nesting level, an abort at the depths this parser accepts.
/// Reparent descendants onto a heap worklist instead.
impl Drop for Node {
    fn drop(&mut self) {
        let mut stack = core::mem::take(&mut self.children);
        while let Some(mut node) = stack.pop() {
            stack.append(&mut node.children);
        }
    }
}

/// Derived clone glue recurses per nesting level exactly like derived
/// drop; clone through an explicit worklist instead.
impl Clone for Node {
    fn clone(&self) -> Self {
        fn shallow(n: &Node) -> Node {
            Node {
                kind: n.kind,
                open: n.open,
                close: n.close,
                children: Vec::with_capacity(n.children.len()),
                statements: n.statements.clone(),
            }
        }
        struct Work<'a> {
            src: &'a Node,
            dst: Node,
            next: usize,
        }
        let mut stack = vec![Work {
            src: self,
            dst: shallow(self),
            next: 0,
        }];
        loop {
            let top = stack.last_mut().expect("clone worklist is never empty");
            if let Some(child) = top.src.children.get(top.next) {
                top.next += 1;
                stack.push(Work {
                    src: child,
                    dst: shallow(child),
                    next: 0,
                });
            } else {
                let done = stack.pop().expect("clone worklist is never empty").dst;
                match stack.last_mut() {
                    Some(parent) => parent.dst.children.push(done),
                    None => return done,
                }
            }
        }
    }
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
/// Depth is a heap question: each level is one [`Frame`] on an explicit
/// worklist (roughly a hundred bytes), so the bound no longer protects the
/// call stack. It survives as an allocation sanity cap — past it, opening
/// delimiters are consumed as ordinary tokens and the parse is declined
/// (`NestingTooDeep`, input preserved byte-for-byte), the same contract as
/// always.
///
/// 131 072 is 4× the 32 000-level guarantee pinned by the deep-nesting
/// tests — which itself out-tolerates PowerShell's own parser
/// (`ScriptTooComplicated` lands between 10k and 20k levels on pwsh 7.5.2)
/// — while capping frame memory for hostile input around a dozen MiB.
const MAX_DEPTH: usize = 131_072;

/// Cap on `UnbalancedOpenDelimiter` diagnostics per parse.
///
/// Diagnosing every unclosed open was fine under the old depth bound, but
/// at worklist depths it is six figures of noise nobody can act on — and
/// quadratic work, since each diagnostic's position scans its line. The
/// outermost few are the actionable ones: a human fixes the first unclosed
/// brace and reparses. `is_incomplete` covers the rest either way.
const MAX_UNCLOSED_DIAGNOSTICS: usize = 32;

/// One in-progress node plus the statement accumulator live inside it.
/// Nesting pushes a `Frame`; a matching close pops it — the parse itself
/// never recurses, so input depth costs heap, not call stack.
struct Frame {
    node: Node,
    stmt_first: Option<u32>,
    stmt_kind: StatementKind,
    stmt_last: u32,
    prev_significant: Option<TokenKind>,
}

impl Frame {
    fn new(node: Node) -> Self {
        Frame {
            node,
            stmt_first: None,
            stmt_kind: StatementKind::Pipeline,
            stmt_last: 0,
            prev_significant: None,
        }
    }

    fn flush_stmt(&mut self) {
        if let Some(first) = self.stmt_first.take() {
            self.node.statements.push(Statement {
                first,
                last: self.stmt_last,
                kind: core::mem::replace(&mut self.stmt_kind, StatementKind::Pipeline),
            });
        }
    }
}

struct Builder<'a> {
    source: &'a str,
    tokens: &'a [Token],
    diagnostics: Vec<Diagnostic>,
    matches: Vec<Option<u32>>,
    incomplete: bool,
    line_index: Option<LineIndex>,
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

    /// Attach a finished child whose delimiter was matched: record the
    /// match pair and fold the child into the parent's statement
    /// accumulator as one significant unit ending at the close token.
    fn attach_closed(&mut self, parent: &mut Frame, done: Node, open_idx: u32, close_idx: u32) {
        self.matches[open_idx as usize] = Some(close_idx);
        self.matches[close_idx as usize] = Some(open_idx);
        parent.stmt_last = close_idx;
        if parent.stmt_first.is_none() {
            parent.stmt_first = Some(open_idx);
        }
        parent.prev_significant = Some(self.tokens[close_idx as usize].kind);
        parent.node.children.push(done);
    }

    /// Attach a child that ran to end of input without its close.
    fn attach_unclosed(&mut self, parent: &mut Frame, done: Node, open_idx: u32, diagnose: bool) {
        self.incomplete = true;
        if diagnose {
            self.diag(
                DiagnosticCode::UnbalancedOpenDelimiter,
                "delimiter is never closed",
                self.tokens[open_idx as usize].span,
            );
        }
        let last = (self.tokens.len() - 1) as u32;
        parent.stmt_last = last;
        if parent.stmt_first.is_none() {
            parent.stmt_first = Some(open_idx);
        }
        parent.prev_significant = Some(self.tokens[last as usize].kind);
        parent.node.children.push(done);
    }

    /// Build the structure tree in one linear pass over the tokens, with an
    /// explicit frame stack instead of recursion: an opening delimiter
    /// pushes a [`Frame`], a matching close pops one. Nesting depth costs
    /// heap only.
    fn build_tree(&mut self) -> Node {
        let mut stack = vec![Frame::new(Node {
            kind: NodeKind::Root,
            open: None,
            close: None,
            children: Vec::new(),
            statements: Vec::new(),
        })];
        let end = self.tokens.len();
        let mut pos = 0;

        while pos < end {
            let i = pos;
            let tok = self.tokens[i];
            let kind = tok.kind;
            let depth = stack.len() - 1;
            let top = stack.last_mut().expect("the root frame is never popped");

            if kind.is_trivia() {
                if kind == TokenKind::Newline {
                    // A newline ends the statement unless the previous
                    // significant token keeps it open.
                    let continues = matches!(
                        top.prev_significant,
                        Some(
                            TokenKind::Pipe
                                | TokenKind::AndAnd
                                | TokenKind::OrOr
                                | TokenKind::Comma
                                | TokenKind::Operator(_)
                        )
                    ) || matches!(top.prev_significant, Some(k) if k.is_open_delimiter());
                    if !continues {
                        top.flush_stmt();
                    }
                }
                pos += 1;
                continue;
            }

            if kind == TokenKind::Semicolon {
                top.stmt_last = i as u32;
                top.flush_stmt();
                pos += 1;
                top.prev_significant = Some(kind);
                continue;
            }

            if kind.is_open_delimiter() {
                if depth >= MAX_DEPTH {
                    // Past the sanity cap. Consume the delimiter as an
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
                    if top.stmt_first.is_none() {
                        top.stmt_first = Some(i as u32);
                    }
                    top.stmt_last = i as u32;
                    pos += 1;
                    top.prev_significant = Some(kind);
                    continue;
                }
                pos += 1;
                stack.push(Frame::new(Node {
                    kind: NodeKind::Delimited(block_kind(kind)),
                    open: Some(i as u32),
                    close: None,
                    children: Vec::new(),
                    statements: Vec::new(),
                }));
                continue;
            }

            if kind.is_close_delimiter() {
                let matched = match top.node.kind {
                    NodeKind::Delimited(_) => {
                        let open_kind = self.tokens[top.node.open.unwrap_or(0) as usize].kind;
                        closes(open_kind, kind)
                    }
                    _ => false,
                };
                if matched {
                    // `stmt_last` already indexes the last significant
                    // token; the token before the close may be trivia.
                    top.flush_stmt();
                    top.node.close = Some(i as u32);
                    pos += 1;
                    let done = stack.pop().expect("the root frame is never popped");
                    let parent = stack.last_mut().expect("a Delimited frame has a parent");
                    let open_idx = done.node.open.unwrap_or(0);
                    self.attach_closed(parent, done.node, open_idx, i as u32);
                    continue;
                }
                // Unbalanced close: recorded by the lexer already; treat it
                // as an ordinary token so downstream stays lossless.
                self.incomplete = true;
                if top.stmt_first.is_none() {
                    top.stmt_first = Some(i as u32);
                }
                top.stmt_last = i as u32;
                pos += 1;
                top.prev_significant = Some(kind);
                continue;
            }

            // Ordinary significant token.
            if top.stmt_first.is_none() {
                top.stmt_first = Some(i as u32);
                top.stmt_kind = match kind {
                    TokenKind::Keyword(kw) => StatementKind::KeywordConstruct(kw),
                    _ => StatementKind::Pipeline,
                };
            }
            // A statement is an Assignment when an `=`-family operator token
            // appears at its nesting level. The context sensitivity lives in
            // the lexer, mirroring pwsh's own tokenizer: `=` in
            // command-argument position scans as a Generic argument (so
            // `foo | bar = 1` stays a Pipeline), and Operator(Assignment)
            // only exists where pwsh reads an assignment operator.
            if kind == TokenKind::Operator(crate::token::OperatorKind::Assignment)
                && top.stmt_kind == StatementKind::Pipeline
            {
                top.stmt_kind = StatementKind::Assignment;
            }
            top.stmt_last = i as u32;
            top.prev_significant = Some(kind);
            pos += 1;
        }

        // End of input: every frame still open is an unclosed delimiter.
        // Unwind innermost-first so diagnostics come out in the same order
        // the recursive descent produced them.
        while stack.len() > 1 {
            let mut done = stack.pop().expect("the root frame is never popped");
            done.flush_stmt();
            // The popped frame lived at depth `stack.len()`; only the
            // outermost few unclosed opens get individual diagnostics.
            let diagnose = stack.len() <= MAX_UNCLOSED_DIAGNOSTICS;
            let parent = stack.last_mut().expect("a Delimited frame has a parent");
            let open_idx = done.node.open.unwrap_or(0);
            self.attach_unclosed(parent, done.node, open_idx, diagnose);
        }
        let mut root = stack.pop().expect("the root frame is never popped");
        root.flush_stmt();
        root.node
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
    };
    let root = builder.build_tree();
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

    /// The assignment rule delegates context to the lexer: `=` in
    /// command-argument position is a Generic token (pinned differentially
    /// against pwsh by the oracle suite), so only a real assignment
    /// operator promotes a statement.
    #[test]
    fn assignment_classification_follows_the_lexer() {
        // Command-argument `=` is not an operator; the pipeline stays one.
        let r = parse("foo | bar = 1");
        assert_eq!(r.root.statements[0].kind, StatementKind::Pipeline);
        assert!(
            !r.tokens
                .iter()
                .any(|t| t.kind == TokenKind::Operator(crate::token::OperatorKind::Assignment)),
            "command-mode `=` must scan as an argument, not an operator"
        );

        // Real assignment operators promote, regardless of what follows.
        for src in ["$x = foo | bar", "@splat = 1"] {
            let r = parse(src);
            assert_eq!(
                r.root.statements[0].kind,
                StatementKind::Assignment,
                "{src}"
            );
        }

        // Statements split at `;` classify independently.
        let r = parse("foo; $y = 1");
        let kinds: Vec<StatementKind> = r.root.statements.iter().map(|s| s.kind).collect();
        assert_eq!(kinds, [StatementKind::Pipeline, StatementKind::Assignment]);

        // Hashtable entries are assignments of their own nesting level.
        let r = parse("@{ a = 1; b = 2 }");
        let entries = &r.root.children[0].statements;
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|s| s.kind == StatementKind::Assignment));

        // `foo | $x = 1` is a pwsh parse error (assignment cannot be a
        // pipeline element); with no valid reading to preserve, the token
        // is taken at face value and the statement classifies Assignment.
        let r = parse("foo | $x = 1");
        assert_eq!(r.root.statements[0].kind, StatementKind::Assignment);
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
        let depth = MAX_DEPTH - 1;
        let src = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
        let r = parse(&src);
        assert!(!r.is_incomplete, "depth {depth} should parse normally");
    }

    /// Nesting past the sanity cap must degrade to an incomplete parse —
    /// never a crash — and stay lossless.
    #[test]
    fn nesting_past_the_limit_is_reported_not_fatal() {
        for depth in [MAX_DEPTH + 1, 500_000] {
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

    /// The point of the worklist parser: 32k levels — past PowerShell's own
    /// `ScriptTooComplicated` ceiling — parse, match, clone, and drop on a
    /// 1 MiB stack. A spawned thread makes the stack size explicit instead
    /// of inheriting whatever the test runner happens to provide.
    #[test]
    fn deep_nesting_on_a_one_mib_stack() {
        std::thread::Builder::new()
            .stack_size(1 << 20)
            .spawn(|| {
                let depth = 32_768;
                let src = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
                let r = parse(&src);
                assert!(!r.is_incomplete, "32k levels are within tolerance");
                let spans: usize = r.tokens.iter().map(|t| t.span.end - t.span.start).sum();
                assert_eq!(spans, src.len(), "lossless at 32k levels");
                // Outermost pair is matched end-to-end (one token per byte).
                assert_eq!(r.matches[0], Some((src.len() - 1) as u32));
                let deep_clone = r.root.clone();
                drop(deep_clone);
                drop(r);
            })
            .expect("spawn thread")
            .join()
            .expect("32k-deep parse must not overflow a 1 MiB stack");
    }

    /// Unbalanced input at the same depth: every unclosed open is
    /// diagnosed, and tearing down the deep unclosed tree is just as
    /// stack-safe as the balanced case.
    #[test]
    fn deep_unbalanced_nesting_on_a_one_mib_stack() {
        std::thread::Builder::new()
            .stack_size(1 << 20)
            .spawn(|| {
                let depth = 32_768;
                let src = format!("{}1", "(".repeat(depth));
                let r = parse(&src);
                assert!(r.is_incomplete);
                assert!(
                    r.diagnostics
                        .iter()
                        .any(|d| d.code == DiagnosticCode::UnbalancedOpenDelimiter)
                );
                let spans: usize = r.tokens.iter().map(|t| t.span.end - t.span.start).sum();
                assert_eq!(spans, src.len(), "lossless at 32k unbalanced levels");
                drop(r);
            })
            .expect("spawn thread")
            .join()
            .expect("32k-deep unbalanced parse must not overflow a 1 MiB stack");
    }
}
