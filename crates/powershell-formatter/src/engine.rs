//! The formatting engine: one analysis, one set of layout decisions, one
//! render.
//!
//! The token stream from `powershell-parser` is split into *significant*
//! tokens and the *gaps* (trivia runs) between them. Formatting rules never
//! edit text directly; they update the layout state of gaps (join with N
//! spaces / break with indentation) or record token respellings (casing).
//! Rules run in PSScriptAnalyzer's order against the evolving gap state, so
//! the final render reproduces `Invoke-Formatter`'s fixpoint without ever
//! re-parsing.

use crate::FormatOptions;
use crate::catalog::CommandCatalog;
use crate::options::EndOfLine;
use powershell_parser::{Keyword, ParseResult, Span, Token, TokenFlags, TokenKind};

/// Line-structure decision for one gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LineState {
    /// Keep the original trivia's line structure (spaces and newlines as
    /// written); indentation may still be rewritten.
    AsIs,
    /// Force the two neighbors onto the same line with exactly `spaces`
    /// spaces (comments in the gap are preserved inline).
    Join { spaces: u8 },
    /// Force a line break. `cap_blanks` limits blank lines (None keeps the
    /// original count). `strip_ws` drops whitespace preceding a forced
    /// newline (corrections anchored on an opening token push trailing
    /// spaces onto the next line, where indentation rewrites them; those
    /// anchored on a closing token leave them in place).
    Break {
        cap_blanks: Option<u32>,
        strip_ws: bool,
    },
}

/// A gap between two significant tokens (or file edges).
#[derive(Debug, Clone)]
pub(crate) struct Gap {
    /// Token-index range (into `ParseResult::tokens`) of the trivia run.
    pub trivia: core::ops::Range<usize>,
    pub line: LineState,
    /// Indentation level for lines started inside this gap (the following
    /// token's line and any comment lines). `None` keeps original leading
    /// whitespace.
    pub indent: Option<u32>,
    /// Extra indentation text quirks: `Some` overrides the level-based
    /// indent entirely (used for alignment spaces before `=`).
    pub exact_spaces: Option<u16>,
    /// A comment moved into the start of this gap by the open-brace rule
    /// (rendered as ` <comment>` before the line break).
    pub moved_comment: Option<String>,
    /// Marks gaps whose trivia contains a backtick line continuation; such
    /// gaps never Join/Break but may be reindented after the continuation.
    pub has_continuation: bool,
    /// Cached: original trivia contains at least one newline token.
    pub orig_newline: bool,
    /// Cached: original trivia contains at least one comment token.
    pub has_comment: bool,
}

impl Gap {
    /// Whether this gap separates lines in the *current* layout state.
    pub fn breaks_line(&self) -> bool {
        match self.line {
            LineState::AsIs => self.orig_newline,
            LineState::Join { .. } => false,
            LineState::Break { .. } => true,
        }
    }
}

/// Prepared formatting context shared by all phases.
pub(crate) struct Engine<'s> {
    pub src: &'s str,
    pub parse: &'s ParseResult,
    /// Indices (into `parse.tokens`) of significant tokens, in order.
    pub sig: Vec<usize>,
    /// `gaps[i]` precedes `sig[i]`; `gaps[sig.len()]` is trailing trivia.
    pub gaps: Vec<Gap>,
    /// Respelled token text (casing fixes), by significant-token position.
    pub respell: Vec<Option<String>>,
    /// For each significant position: the significant position of its
    /// matching delimiter (from the structural parse).
    pub sig_match: Vec<Option<usize>>,
    /// For each significant position: significant position of the innermost
    /// enclosing open delimiter (None at top level).
    pub enclosing: Vec<Option<usize>>,
    pub opts: &'s FormatOptions,
    /// Detected output newline ("\n" or "\r\n").
    pub newline: &'static str,
}

impl<'s> Engine<'s> {
    pub fn new(src: &'s str, parse: &'s ParseResult, opts: &'s FormatOptions) -> Self {
        let tokens = &parse.tokens;
        let mut sig = Vec::with_capacity(tokens.len());
        let mut gaps = Vec::with_capacity(tokens.len() / 2 + 2);
        let mut tok_to_sig = vec![usize::MAX; tokens.len()];

        let mut i = 0;
        while i <= tokens.len() {
            let trivia_start = i;
            while i < tokens.len() && tokens[i].kind.is_trivia() {
                i += 1;
            }
            let trivia = trivia_start..i;
            let orig_newline = tokens[trivia.clone()]
                .iter()
                .any(|t| t.kind == TokenKind::Newline);
            let has_comment = tokens[trivia.clone()].iter().any(|t| t.kind.is_comment());
            let has_continuation = tokens[trivia.clone()]
                .iter()
                .any(|t| t.flags.contains(TokenFlags::LINE_CONTINUATION));
            gaps.push(Gap {
                trivia,
                line: LineState::AsIs,
                indent: None,
                exact_spaces: None,
                moved_comment: None,
                has_continuation,
                orig_newline,
                has_comment,
            });
            if i < tokens.len() {
                tok_to_sig[i] = sig.len();
                sig.push(i);
                i += 1;
            } else {
                break;
            }
        }

        let sig_match: Vec<Option<usize>> = sig
            .iter()
            .map(|&ti| {
                parse.matches[ti].and_then(|m| {
                    let m = m as usize;
                    let s = tok_to_sig[m];
                    (s != usize::MAX).then_some(s)
                })
            })
            .collect();

        // Innermost enclosing open delimiter, per significant position.
        let mut enclosing = vec![None; sig.len()];
        let mut stack: Vec<usize> = Vec::new();
        for (pos, &ti) in sig.iter().enumerate() {
            let kind = parse.tokens[ti].kind;
            if kind.is_close_delimiter() {
                // The closer belongs to the frame it closes.
                if let Some(&top) = stack.last() {
                    if sig_match[top] == Some(pos) {
                        stack.pop();
                    }
                }
                enclosing[pos] = stack.last().copied();
                continue;
            }
            enclosing[pos] = stack.last().copied();
            if kind.is_open_delimiter() && sig_match[pos].is_some() {
                stack.push(pos);
            }
        }

        let newline = detect_newline(src, parse, opts);
        let respell = vec![None; sig.len()];
        Self {
            src,
            parse,
            sig,
            gaps,
            respell,
            sig_match,
            enclosing,
            opts,
            newline,
        }
    }

    pub fn token(&self, pos: usize) -> &Token {
        &self.parse.tokens[self.sig[pos]]
    }

    pub fn kind(&self, pos: usize) -> TokenKind {
        self.token(pos).kind
    }

    pub fn text(&self, pos: usize) -> &str {
        self.token(pos).text(self.src)
    }

    /// Number of significant tokens.
    pub fn len(&self) -> usize {
        self.sig.len()
    }

    /// True when the delimiter pair opening at `open_pos` spans a single
    /// line in the current layout (no breaking gap and no multi-line token
    /// strictly inside).
    pub fn pair_is_one_line(&self, open_pos: usize) -> bool {
        let Some(close_pos) = self.sig_match[open_pos] else {
            return false;
        };
        for p in open_pos + 1..=close_pos {
            if self.gaps[p].breaks_line() {
                return false;
            }
            if p < close_pos && self.token_is_multiline(p) {
                return false;
            }
        }
        true
    }

    /// A token whose text spans multiple lines (here-strings, multi-line
    /// strings...).
    pub fn token_is_multiline(&self, pos: usize) -> bool {
        let t = self.text(pos);
        t.contains('\n') || t.contains('\r')
    }

    /// True when the `{` at `pos` opens a *statement* block: the body of a
    /// keyword construct (if/loops/try/function/class/switch clauses/named
    /// blocks). Verified against PSScriptAnalyzer 1.25: all brace-placement
    /// behaviors apply only to these; script blocks used as expressions or
    /// command arguments (`$x = {`, `% { }`, `.foreach({`) are never moved.
    pub fn is_statement_brace(&self, pos: usize) -> bool {
        if self.kind(pos) != TokenKind::LCurly || pos == 0 {
            return false;
        }
        // Directly inside a switch body, clause braces are statement braces.
        if let Some(open) = self.enclosing[pos]
            && self.is_switch_body(open)
        {
            return true;
        }
        match self.kind(pos - 1) {
            TokenKind::Keyword(kw) => matches!(
                kw,
                Keyword::Else
                    | Keyword::Try
                    | Keyword::Finally
                    | Keyword::Do
                    | Keyword::Begin
                    | Keyword::Process
                    | Keyword::End
                    | Keyword::Clean
                    | Keyword::DynamicParam
                    | Keyword::Data
                    | Keyword::Trap
                    | Keyword::Catch
                    | Keyword::Default
                    | Keyword::InlineScript
                    | Keyword::Parallel
                    | Keyword::Sequence
            ),
            TokenKind::RParen => self.sig_match[pos - 1].is_some_and(|open| {
                open > 0
                    && match self.kind(open - 1) {
                        TokenKind::Keyword(kw) => matches!(
                            kw,
                            Keyword::If
                                | Keyword::ElseIf
                                | Keyword::While
                                | Keyword::For
                                | Keyword::Foreach
                                | Keyword::Switch
                                | Keyword::Until
                        ),
                        // `function foo ($a) {`
                        TokenKind::Generic | TokenKind::Identifier => {
                            self.preceded_by_definition_keyword(open - 1)
                        }
                        _ => false,
                    }
            }),
            // `catch [A], [B] {` / `trap [Ex] {`
            TokenKind::RBracket => {
                let mut p = pos - 1;
                loop {
                    match self.kind(p) {
                        TokenKind::RBracket => {
                            let Some(open) = self.sig_match[p] else {
                                return false;
                            };
                            if open == 0 {
                                return false;
                            }
                            p = open - 1;
                        }
                        TokenKind::Comma => {
                            if p == 0 {
                                return false;
                            }
                            p -= 1;
                        }
                        TokenKind::Keyword(Keyword::Catch | Keyword::Trap) => return true,
                        _ => return false,
                    }
                }
            }
            // `function foo {` / `class Foo : Base {` / `enum Color {`
            TokenKind::Generic | TokenKind::Identifier => {
                self.preceded_by_definition_keyword(pos - 1)
            }
            _ => false,
        }
    }

    /// Walk back over a definition header (name, base-class list) looking
    /// for `function`/`filter`/`workflow`/`configuration`/`class`/`enum`.
    fn preceded_by_definition_keyword(&self, name_pos: usize) -> bool {
        let mut p = name_pos;
        let mut steps = 0;
        while p > 0 && steps < 8 {
            p -= 1;
            steps += 1;
            match self.kind(p) {
                TokenKind::Keyword(
                    Keyword::Function
                    | Keyword::Filter
                    | Keyword::Workflow
                    | Keyword::Configuration
                    | Keyword::Class
                    | Keyword::Enum,
                ) => return true,
                TokenKind::Identifier
                | TokenKind::Generic
                | TokenKind::Comma
                | TokenKind::Operator(_) => {}
                _ => return false,
            }
        }
        false
    }

    /// `open` is the `{` of a `switch (...) { ... }` body.
    fn is_switch_body(&self, open: usize) -> bool {
        self.kind(open) == TokenKind::LCurly
            && open > 0
            && self.kind(open - 1) == TokenKind::RParen
            && self.sig_match[open - 1]
                .is_some_and(|p| p > 0 && self.kind(p - 1) == TokenKind::Keyword(Keyword::Switch))
    }

    /// Renders the final output.
    pub fn render(&self, range: Option<Span>) -> String {
        let mut out = String::with_capacity(self.src.len() + self.src.len() / 8);
        for pos in 0..=self.len() {
            let gap = &self.gaps[pos];
            let in_range = range.is_none_or(|r| self.gap_in_range(pos, &r));
            if in_range {
                self.render_gap(gap, pos, &mut out);
            } else {
                for t in &self.parse.tokens[gap.trivia.clone()] {
                    out.push_str(t.text(self.src));
                }
            }
            if pos < self.len() {
                let token_in_range = range.is_none_or(|r| r.overlaps(&self.token(pos).span));
                match &self.respell[pos] {
                    Some(text) if token_in_range => out.push_str(text),
                    _ => out.push_str(self.text(pos)),
                }
            }
        }
        out
    }

    fn gap_in_range(&self, pos: usize, range: &Span) -> bool {
        let start = self.gaps[pos]
            .trivia
            .clone()
            .next()
            .map_or_else(|| self.gap_anchor(pos), |t| self.parse.tokens[t].span.start);
        let end = self.gaps[pos]
            .trivia
            .clone()
            .last()
            .map_or_else(|| self.gap_anchor(pos), |t| self.parse.tokens[t].span.end);
        range.overlaps(&Span::new(start, end))
    }

    fn gap_anchor(&self, pos: usize) -> usize {
        if pos < self.len() {
            self.token(pos).span.start
        } else {
            self.src.len()
        }
    }

    fn indent_text(&self, level: u32) -> String {
        if self.opts.use_tabs {
            "\t".repeat(level as usize)
        } else {
            " ".repeat(level as usize * self.opts.indent_width as usize)
        }
    }

    fn render_gap(&self, gap: &Gap, pos: usize, out: &mut String) {
        if let Some(comment) = &gap.moved_comment {
            out.push(' ');
            out.push_str(comment);
        }
        match &gap.line {
            LineState::Join { spaces } => {
                if let Some(exact) = gap.exact_spaces {
                    for _ in 0..exact {
                        out.push(' ');
                    }
                    self.render_join_comments(gap, out, 0);
                    return;
                }
                self.render_join_comments(gap, out, *spaces);
            }
            LineState::AsIs if !gap.orig_newline && !gap.has_continuation => {
                // Same-line gap kept as written (unless alignment overrode).
                if let Some(exact) = gap.exact_spaces {
                    for _ in 0..exact {
                        out.push(' ');
                    }
                    self.render_join_comments(gap, out, 0);
                    return;
                }
                if pos == 0 && gap.indent.is_some() {
                    // Leading trivia of the first line: a BOM survives, the
                    // whitespace is rewritten to the decided indentation.
                    for t in &self.parse.tokens[gap.trivia.clone()] {
                        if t.text(self.src).contains('\u{FEFF}') {
                            out.push('\u{FEFF}');
                        }
                        if t.kind.is_comment() {
                            out.push_str(t.text(self.src));
                        }
                    }
                    if let Some(level) = gap.indent {
                        out.push_str(&self.indent_text(level));
                    }
                    return;
                }
                for t in &self.parse.tokens[gap.trivia.clone()] {
                    out.push_str(t.text(self.src));
                }
            }
            LineState::AsIs | LineState::Break { .. } => {
                self.render_break(gap, pos, out);
            }
        }
    }

    /// Join rendering: N spaces, preserving any comments (space-separated).
    fn render_join_comments(&self, gap: &Gap, out: &mut String, spaces: u8) {
        let mut emitted_comment = false;
        for t in &self.parse.tokens[gap.trivia.clone()] {
            if t.kind.is_comment() {
                if !emitted_comment {
                    for _ in 0..spaces.max(1) {
                        out.push(' ');
                    }
                } else {
                    out.push(' ');
                }
                out.push_str(t.text(self.src));
                emitted_comment = true;
            }
        }
        if emitted_comment {
            out.push(' ');
        } else {
            for _ in 0..spaces {
                out.push(' ');
            }
        }
    }

    /// Break/AsIs-multi-line rendering: preserve comments and blank lines,
    /// rewrite line-leading whitespace per the decided indentation.
    fn render_break(&self, gap: &Gap, pos: usize, out: &mut String) {
        let (cap_blanks, strip_ws) = match &gap.line {
            LineState::Break {
                cap_blanks,
                strip_ws,
            } => (*cap_blanks, *strip_ws),
            _ => (None, false),
        };
        let indent = gap.indent.map(|l| self.indent_text(l));
        let mut newlines_emitted: u32 = 0;
        let mut consecutive_newlines: u32 = 0;
        let mut pending_ws: Option<&str> = None;
        let mut at_line_start = false;

        for t in &self.parse.tokens[gap.trivia.clone()] {
            match t.kind {
                TokenKind::Whitespace => {
                    let text = t.text(self.src);
                    if t.flags.contains(TokenFlags::LINE_CONTINUATION) {
                        // Emit everything up to and including the final
                        // newline verbatim; the trailing spaces become the
                        // continuation indent.
                        let cut = text.rfind(['\n']).map_or(0, |i| i + 1);
                        if let Some(p) = pending_ws.take() {
                            out.push_str(p);
                        }
                        out.push_str(&text[..cut]);
                        pending_ws = Some(&text[cut..]);
                        at_line_start = true;
                        newlines_emitted += 1;
                        consecutive_newlines = 0;
                    } else {
                        pending_ws = Some(text);
                    }
                }
                TokenKind::Newline => {
                    consecutive_newlines += 1;
                    let capped = cap_blanks.is_some_and(|cap| consecutive_newlines > cap + 1);
                    if !capped {
                        // PSSA never trims trailing whitespace: spaces
                        // before an original newline survive.
                        if let Some(p) = pending_ws.take() {
                            out.push_str(p);
                        }
                        self.push_newline(out);
                        newlines_emitted += 1;
                    } else {
                        pending_ws = None;
                    }
                    at_line_start = true;
                }
                k if k.is_comment() => {
                    if at_line_start {
                        match (&indent, pending_ws.take()) {
                            (Some(ind), _) => out.push_str(ind),
                            (None, Some(p)) => out.push_str(p),
                            (None, None) => {}
                        }
                    } else if let Some(p) = pending_ws.take() {
                        out.push_str(p);
                    } else if !out.is_empty() && !out.ends_with([' ', '\t', '\n']) {
                        out.push(' ');
                    }
                    out.push_str(t.text(self.src));
                    at_line_start = false;
                    consecutive_newlines = 0;
                }
                _ => {}
            }
        }

        if newlines_emitted == 0 {
            // Forced break with no original newline. Close-anchored
            // corrections leave the original whitespace before the newline
            // (PSSA never trims trailing spaces); open-anchored ones move
            // past it.
            match pending_ws.take() {
                Some(p) if !strip_ws => out.push_str(p),
                _ => {}
            }
            self.push_newline(out);
            at_line_start = true;
        }
        if at_line_start {
            match (&indent, pending_ws) {
                (Some(ind), _) => {
                    if pos < self.len() {
                        out.push_str(ind);
                    }
                }
                (None, Some(p)) => out.push_str(p),
                (None, None) => {}
            }
        } else if let Some(p) = pending_ws {
            out.push_str(p);
        }
    }

    fn push_newline(&self, out: &mut String) {
        out.push_str(self.newline);
    }
}

/// Detect the output newline from options and source content.
///
/// Auto mode inspects the first newline *between* tokens — newlines inside
/// here-strings and multi-line strings are protected content the formatter
/// never rewrites, so they must not steer detection (that would make mixed
/// files non-idempotent). PSScriptAnalyzer instead refuses mixed-newline
/// input outright; we normalize the newlines we own to the first style.
fn detect_newline(src: &str, parse: &ParseResult, opts: &FormatOptions) -> &'static str {
    match opts.end_of_line {
        EndOfLine::Lf => "\n",
        EndOfLine::Crlf => "\r\n",
        EndOfLine::Auto => {
            // Only plain Newline tokens participate: backtick-continuation
            // newlines render verbatim, so they must not steer detection.
            for t in &parse.tokens {
                if t.kind == TokenKind::Newline {
                    let text = t.text(src);
                    return if text.as_bytes()[0] == b'\r' {
                        "\r\n"
                    } else {
                        "\n"
                    };
                }
            }
            // No trivia newlines: fall back to the first newline anywhere
            // (a forced break will then match whatever a second pass sees).
            match src.find('\n') {
                Some(i) if i > 0 && src.as_bytes()[i - 1] == b'\r' => "\r\n",
                _ => "\n",
            }
        }
    }
}

/// Run all formatting phases over the engine, in PSSA rule order.
pub(crate) fn run_phases(engine: &mut Engine<'_>, catalog: Option<&dyn CommandCatalog>) {
    crate::phases::braces::place_close_braces(engine);
    crate::phases::braces::place_open_braces(engine);
    crate::phases::whitespace::apply(engine);
    crate::phases::reflow::apply(engine);
    crate::phases::indent::apply(engine);
    crate::phases::align::apply(engine);
    crate::phases::casing::apply(engine, catalog);
}
