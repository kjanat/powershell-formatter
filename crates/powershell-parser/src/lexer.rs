//! The lossless PowerShell scanner.
//!
//! Every byte of the input ends up in exactly one token; concatenating token
//! texts reproduces the source. The scanner is mode-driven: PowerShell
//! tokenizes differently in command-name position, command-argument position,
//! and expression position, so the lexer tracks a small context stack that
//! approximates the decisions PowerShell's parser-driven tokenizer makes.
//!
//! The behavior here follows PowerShell's own `tokenizer.cs`/`CharTraits.cs`
//! (see `docs/oracles.md`) and is differential-tested against `pwsh` itself
//! in `tests/powershell-oracle`; when in doubt, PowerShell wins.

use crate::diag::{Diagnostic, DiagnosticCode, Severity};
use crate::span::{LineIndex, Span};
use crate::token::{Keyword, OperatorKind, Token, TokenFlags, TokenKind};

/// Result of scanning: the lossless token stream plus lexical diagnostics.
#[derive(Debug, Clone)]
pub struct LexOutput {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Scan PowerShell source into a lossless, formatter-oriented token stream.
#[must_use]
pub fn tokenize(source: &str) -> LexOutput {
    Lexer::new(source).run()
}

/// What the lexer expects next inside the current frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Start of a statement / pipeline element: words may be keywords or
    /// command names, `:label` may appear, expressions may start.
    StatementStart,
    /// After a command name: bare words are generic arguments, `-x` is a
    /// parameter.
    CommandArgs,
    /// An expression operand is expected (after a binary/unary operator,
    /// after `,` in an expression, after a cast type literal).
    ExprOperand,
    /// An expression operand was just consumed: dash-words are operators,
    /// `[` is indexing, `.`/`::` are member access.
    ExprOperator,
    /// Inside `@{}` expecting a key (or `}`).
    HashKey,
    /// After a `function`/`filter`/`class`/`enum`/`configuration`/`workflow`
    /// keyword or an invocation operator (`&`, dot-source `.`): the next
    /// token names a definition or command, and arguments follow.
    DefinitionName,
    /// After a member-access operator in command-argument position
    /// (`Should -Be $x.Length`): a simple member name follows, then
    /// argument mode resumes.
    ArgMemberName,
    /// After the `using` keyword: `namespace`/`module`/`assembly` keep
    /// their keyword nature, then the target is a generic word.
    UsingDirective,
}

impl Mode {
    /// True when numbers terminate with PowerShell's expression-mode rules
    /// (`ForceStartNewTokenAfterNumber`); in command-argument position only
    /// the `ForceStartNewToken` set ends a number.
    fn number_expression_rules(self) -> bool {
        !matches!(
            self,
            Mode::CommandArgs | Mode::DefinitionName | Mode::UsingDirective
        )
    }
}

/// The kind of nesting frame we are inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameKind {
    /// File top level or `{ ... }` script block: newlines separate
    /// statements.
    Block,
    /// `@{ ... }` hashtable literal.
    Hash,
    /// `( ... )` grouping/condition: newlines continue the construct.
    Paren,
    /// `$( ... )` / `@( ... )`: statements inside, newlines separate them.
    SubExpr,
    /// `[ ... ]` type literal or index.
    Bracket { index: bool },
}

#[derive(Debug, Clone, Copy)]
struct Frame {
    kind: FrameKind,
    mode: Mode,
    /// Mode restored at statement boundaries (newline/semicolon). Blocks
    /// normally reset to `StatementStart`; class/enum bodies reset to
    /// signature scanning.
    base: Mode,
}

struct Lexer<'src> {
    src: &'src str,
    bytes: &'src [u8],
    pos: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
    frames: Vec<Frame>,
    line_index: Option<LineIndex>,
    /// Set after a `class`/`enum` keyword: the next `{` opens a signature
    /// body (member declarations), not a statement block.
    pending_signature_body: bool,
}

// ── character classification helpers ─────────────────────────────────
//
// These mirror PowerShell's CharTraits.cs. Newlines are only `\n`/`\r`;
// U+0085/U+2028/U+2029 are *whitespace*, never line terminators.

fn is_horizontal_ws(c: char) -> bool {
    matches!(c, '\t' | '\u{0B}' | '\u{0C}' | ' ' | '\u{A0}' | '\u{85}')
        || (c > '\u{FF}' && c.is_whitespace() && c != '\n' && c != '\r')
}

/// Characters PowerShell accepts as a dash (operator/parameter leader).
fn is_dash(c: char) -> bool {
    matches!(c, '-' | '\u{2013}' | '\u{2014}' | '\u{2015}')
}

/// Characters PowerShell accepts as a single quote.
fn is_single_quote(c: char) -> bool {
    matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}')
}

/// Characters PowerShell accepts as a double quote.
fn is_double_quote(c: char) -> bool {
    matches!(c, '"' | '\u{201C}' | '\u{201D}' | '\u{201E}')
}

/// First character of an unbraced variable name (`VarNameFirst`).
fn is_variable_start(c: char) -> bool {
    matches!(c, '$' | ':' | '?' | '^' | '_') || c.is_alphanumeric()
}

/// Subsequent characters of an unbraced variable name.
fn is_variable_char(c: char) -> bool {
    matches!(c, '?' | '_') || c.is_alphanumeric()
}

/// Identifier start (`IsIdentifierStart`).
fn is_identifier_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

/// Word characters for identifiers / keywords (`IsIdentifierFollow`).
fn is_word_char(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

/// PowerShell's `ForceStartNewToken`: the only characters that always end a
/// generic/word token. Notably `< > [ ] = + - . : ? @ # ' "` do NOT.
fn force_start_new_token(c: char) -> bool {
    matches!(
        c,
        '\0' | '\n' | '\r' | '&' | '(' | ')' | ',' | ';' | '{' | '|' | '}'
    ) || is_horizontal_ws(c)
}

/// PowerShell's `ForceStartNewTokenAfterNumber` (expression mode only).
/// `?`/`:` additionally end numbers only when the parser expects a possible
/// ternary (`ForceEndNumberOnTernaryOpChars`): true in expression operand
/// positions (`$null ?? 9?10:11`, `[int]6?7:8`) but not at pipeline starts,
/// where `1?2:3` is a single generic command word. Verified against pwsh 7.5.
fn ends_number_expression(c: char, ternary: bool) -> bool {
    matches!(
        c,
        '!' | '#' | '%' | '*' | '+' | '.' | '/' | '<' | '=' | '>' | ']'
    ) || is_dash(c)
        || (ternary && matches!(c, '?' | ':'))
}

impl<'src> Lexer<'src> {
    fn new(src: &'src str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
            tokens: Vec::with_capacity(src.len() / 6 + 8),
            diagnostics: Vec::new(),
            frames: vec![Frame {
                kind: FrameKind::Block,
                mode: Mode::StatementStart,
                base: Mode::StatementStart,
            }],
            line_index: None,
            pending_signature_body: false,
        }
    }

    fn run(mut self) -> LexOutput {
        while self.pos < self.src.len() {
            let before = self.pos;
            self.next_token();
            debug_assert!(self.pos > before, "lexer made no progress at {before}");
            if self.pos == before {
                // Defensive: never loop forever on unexpected input.
                if let Some(c) = self.peek() {
                    self.pos += c.len_utf8();
                    self.emit(TokenKind::Unknown, before, TokenFlags::empty());
                }
            }
        }
        LexOutput {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    // ── small utilities ──────────────────────────────────────────────

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.src.get(self.pos + offset..)?.chars().next()
    }

    fn char_at(&self, at: usize) -> Option<char> {
        self.src.get(at..)?.chars().next()
    }

    fn byte(&self, at: usize) -> Option<u8> {
        self.bytes.get(at).copied()
    }

    fn starts_with(&self, s: &str) -> bool {
        self.src[self.pos..].starts_with(s)
    }

    fn mode(&self) -> Mode {
        self.frames.last().map_or(Mode::StatementStart, |f| f.mode)
    }

    fn set_mode(&mut self, mode: Mode) {
        if let Some(f) = self.frames.last_mut() {
            f.mode = mode;
        }
    }

    fn frame_kind(&self) -> FrameKind {
        self.frames.last().map_or(FrameKind::Block, |f| f.kind)
    }

    fn push_frame(&mut self, kind: FrameKind, mode: Mode) {
        self.frames.push(Frame {
            kind,
            mode,
            base: mode,
        });
    }

    fn base_mode(&self) -> Mode {
        self.frames.last().map_or(Mode::StatementStart, |f| f.base)
    }

    /// True inside a class/enum body (signature scanning).
    fn in_signature_body(&self) -> bool {
        self.frames
            .last()
            .is_some_and(|f| f.kind == FrameKind::Block && f.base == Mode::ExprOperand)
    }

    fn pop_frame(&mut self) {
        if self.frames.len() > 1 {
            self.frames.pop();
        }
    }

    fn emit(&mut self, kind: TokenKind, start: usize, flags: TokenFlags) {
        debug_assert!(self.pos > start, "empty token at {start}");
        self.tokens
            .push(Token::new(kind, Span::new(start, self.pos), flags));
    }

    fn diag(&mut self, code: DiagnosticCode, severity: Severity, message: &str, span: Span) {
        let index = self
            .line_index
            .get_or_insert_with(|| LineIndex::new(self.src));
        let position = index.position(self.src, span.start);
        self.diagnostics
            .push(Diagnostic::new(code, severity, message, span, position));
    }

    /// Mode to adopt after consuming a complete expression operand.
    fn operand_consumed(&mut self) {
        let next = match self.mode() {
            Mode::CommandArgs | Mode::DefinitionName | Mode::ArgMemberName => Mode::CommandArgs,
            Mode::HashKey => Mode::HashKey,
            _ => Mode::ExprOperator,
        };
        self.set_mode(next);
    }

    /// True when the previous token touches the current position (no trivia
    /// between) and is a primary expression that member access / indexing
    /// can attach to even in command-argument mode.
    fn adjacent_primary(&self) -> bool {
        self.tokens.last().is_some_and(|t| {
            t.span.end == self.pos
                && matches!(
                    t.kind,
                    TokenKind::Variable
                        | TokenKind::SplattedVariable
                        | TokenKind::RParen
                        | TokenKind::RBracket
                        | TokenKind::RCurly
                        | TokenKind::Identifier
                        | TokenKind::StringLiteral
                        | TokenKind::StringExpandable
                        | TokenKind::HereStringLiteral
                        | TokenKind::HereStringExpandable
                )
        })
    }

    // ── the dispatcher ───────────────────────────────────────────────

    fn next_token(&mut self) {
        let start = self.pos;
        let Some(c) = self.peek() else { return };

        // Trivia first: identical in every mode.
        if c == '\n' || c == '\r' {
            self.scan_newline(start);
            return;
        }
        if is_horizontal_ws(c) || c == '\u{FEFF}' {
            self.scan_whitespace(start);
            return;
        }
        if c == '`' {
            // Backtick + newline/whitespace: line continuation, swallowed
            // with the surrounding whitespace run (as PowerShell does).
            match self.peek_at(1) {
                Some(n) if n == '\n' || n == '\r' || is_horizontal_ws(n) => {
                    self.scan_whitespace(start);
                }
                Some(_) => {
                    // Backtick escape starts a generic token.
                    self.scan_generic(start);
                }
                None => {
                    self.pos += 1;
                    self.emit(TokenKind::Unknown, start, TokenFlags::UNTERMINATED);
                }
            }
            return;
        }
        if c == '#' {
            self.scan_line_comment(start);
            return;
        }
        if self.starts_with("<#") {
            self.scan_block_comment(start);
            return;
        }

        if is_single_quote(c) {
            self.scan_single_quoted(start);
            self.operand_consumed();
            return;
        }
        if is_double_quote(c) {
            self.scan_double_quoted(start);
            self.operand_consumed();
            return;
        }

        // `@` family: @' @" @{ @( @splat.
        if c == '@' {
            match self.peek_at(1) {
                Some(q) if is_single_quote(q) => {
                    self.scan_here_string(start, false);
                    self.operand_consumed();
                    return;
                }
                Some(q) if is_double_quote(q) => {
                    self.scan_here_string(start, true);
                    self.operand_consumed();
                    return;
                }
                Some('{') => {
                    self.pos += 2;
                    self.emit(TokenKind::AtCurly, start, TokenFlags::empty());
                    self.push_frame(FrameKind::Hash, Mode::HashKey);
                    return;
                }
                Some('(') => {
                    self.pos += 2;
                    self.emit(TokenKind::AtParen, start, TokenFlags::empty());
                    self.push_frame(FrameKind::SubExpr, Mode::StatementStart);
                    return;
                }
                Some(n) if is_variable_start(n) => {
                    self.pos += 1;
                    self.scan_variable_name();
                    self.emit(TokenKind::SplattedVariable, start, TokenFlags::empty());
                    self.operand_consumed();
                    return;
                }
                _ => {
                    self.pos += 1;
                    self.emit(TokenKind::Unknown, start, TokenFlags::empty());
                    self.diag(
                        DiagnosticCode::UnrecognizedToken,
                        Severity::Warning,
                        "unrecognized token after '@'",
                        Span::new(start, self.pos),
                    );
                    return;
                }
            }
        }

        if c == '$' {
            if self.peek_at(1) == Some('(') {
                self.pos += 2;
                self.emit(TokenKind::DollarParen, start, TokenFlags::empty());
                self.push_frame(FrameKind::SubExpr, Mode::StatementStart);
                return;
            }
            self.scan_variable(start);
            return;
        }

        // Delimiters.
        match c {
            '{' => {
                self.pos += 1;
                self.emit(TokenKind::LCurly, start, TokenFlags::empty());
                let body_mode = if self.pending_signature_body {
                    Mode::ExprOperand
                } else {
                    Mode::StatementStart
                };
                self.pending_signature_body = false;
                self.push_frame(FrameKind::Block, body_mode);
                return;
            }
            '}' => {
                self.pos += 1;
                self.emit(TokenKind::RCurly, start, TokenFlags::empty());
                if matches!(self.frame_kind(), FrameKind::Block | FrameKind::Hash)
                    && self.frames.len() > 1
                {
                    self.pop_frame();
                    self.operand_consumed();
                } else {
                    self.report_unbalanced(start);
                }
                return;
            }
            '(' => {
                // Method-call and attribute argument lists are expression
                // contexts; other parens contain pipelines.
                let expr_args = self.mode() == Mode::ExprOperator
                    || matches!(self.frame_kind(), FrameKind::Bracket { index: false });
                self.pos += 1;
                self.emit(TokenKind::LParen, start, TokenFlags::empty());
                self.push_frame(
                    FrameKind::Paren,
                    if expr_args {
                        Mode::ExprOperand
                    } else {
                        Mode::StatementStart
                    },
                );
                return;
            }
            ')' => {
                self.pos += 1;
                self.emit(TokenKind::RParen, start, TokenFlags::empty());
                if matches!(self.frame_kind(), FrameKind::Paren | FrameKind::SubExpr)
                    && self.frames.len() > 1
                {
                    self.pop_frame();
                    self.operand_consumed();
                } else {
                    self.report_unbalanced(start);
                }
                return;
            }
            '[' => {
                // In command-argument position, `[` glued to more text is a
                // wildcard/generic argument (`echo [int]` is one word) —
                // unless it indexes an immediately preceding expression
                // (`Should -Be $errors[$i]`).
                if self.mode() == Mode::CommandArgs && !self.adjacent_primary() {
                    if self.peek_at(1).is_some_and(|n| !force_start_new_token(n)) {
                        self.scan_generic(start);
                        return;
                    }
                    // Standalone `[` in arguments: still generic.
                    self.scan_generic(start);
                    return;
                }
                if self.mode() == Mode::CommandArgs && self.adjacent_primary() {
                    self.pos += 1;
                    self.emit(TokenKind::LBracket, start, TokenFlags::empty());
                    self.push_frame(FrameKind::Bracket { index: true }, Mode::ExprOperand);
                    return;
                }
                // Inside a type literal, nested brackets are generic type
                // argument lists / array ranks, not indexing.
                let index = self.mode() == Mode::ExprOperator
                    && !matches!(self.frame_kind(), FrameKind::Bracket { index: false });
                self.pos += 1;
                self.emit(TokenKind::LBracket, start, TokenFlags::empty());
                self.push_frame(FrameKind::Bracket { index }, Mode::ExprOperand);
                return;
            }
            ']' => {
                self.pos += 1;
                self.emit(TokenKind::RBracket, start, TokenFlags::empty());
                if let FrameKind::Bracket { index } = self.frame_kind() {
                    self.pop_frame();
                    if index {
                        self.operand_consumed();
                    } else if self.starts_with("::") || self.peek() == Some('.') {
                        // `[int]::Parse` / `[int].Name`: the literal is an
                        // operand.
                        self.operand_consumed();
                    } else if matches!(self.mode(), Mode::CommandArgs | Mode::HashKey) {
                        // keep argument/hash-key mode
                    } else {
                        // `[int]$x` cast: an operand still follows.
                        self.set_mode(Mode::ExprOperand);
                    }
                } else {
                    self.report_unbalanced(start);
                }
                return;
            }
            ',' => {
                self.pos += 1;
                self.emit(TokenKind::Comma, start, TokenFlags::empty());
                if !matches!(self.mode(), Mode::CommandArgs | Mode::HashKey) {
                    self.set_mode(Mode::ExprOperand);
                }
                return;
            }
            ';' => {
                self.pos += 1;
                self.emit(TokenKind::Semicolon, start, TokenFlags::empty());
                self.pending_signature_body = false;
                let base = self.base_mode();
                self.set_mode(base);
                return;
            }
            '|' => {
                if self.peek_at(1) == Some('|') {
                    self.pos += 2;
                    self.emit(TokenKind::OrOr, start, TokenFlags::empty());
                } else {
                    self.pos += 1;
                    self.emit(TokenKind::Pipe, start, TokenFlags::empty());
                }
                self.set_mode(Mode::StatementStart);
                return;
            }
            '&' => {
                if self.peek_at(1) == Some('&') {
                    self.pos += 2;
                    self.emit(TokenKind::AndAnd, start, TokenFlags::empty());
                    self.set_mode(Mode::StatementStart);
                } else {
                    self.pos += 1;
                    self.emit(
                        TokenKind::Operator(OperatorKind::Invocation),
                        start,
                        TokenFlags::empty(),
                    );
                    // `& cmd args...`: whatever names the command, argument
                    // mode follows it.
                    self.set_mode(Mode::DefinitionName);
                }
                return;
            }
            _ => {}
        }

        // Numbers. `1>`..`6>` are redirections in command position, but in
        // expression-operand position the number wins and the `>` is lexed
        // separately (PowerShell "undoes" the redirection there).
        if c.is_ascii_digit() && self.mode() != Mode::ExprOperator {
            let redirection_here = !self.mode().number_expression_rules()
                && ('1'..='7').contains(&c)
                && self.byte(self.pos + 1) == Some(b'>');
            if !redirection_here && self.try_scan_number(start, false) {
                return;
            }
        }
        if c == '.'
            && matches!(self.mode(), Mode::StatementStart | Mode::ExprOperand)
            && self.peek_at(1).is_some_and(|d| d.is_ascii_digit())
            && self.try_scan_number(start, false)
        {
            return;
        }

        if is_dash(c) {
            self.scan_dash(start);
            return;
        }

        // Symbolic operators, redirections, labels, dots.
        if self.scan_symbolic(start) {
            return;
        }

        // Words: keywords, identifiers, command names, generic arguments.
        if is_word_char(c) || self.mode() == Mode::CommandArgs {
            self.scan_word_or_generic(start);
            return;
        }

        // Anything else: single unknown char.
        self.pos += c.len_utf8();
        self.emit(TokenKind::Unknown, start, TokenFlags::empty());
        self.diag(
            DiagnosticCode::UnrecognizedToken,
            Severity::Warning,
            &format!("unrecognized character {c:?}"),
            Span::new(start, self.pos),
        );
    }

    fn report_unbalanced(&mut self, start: usize) {
        self.diag(
            DiagnosticCode::UnbalancedCloseDelimiter,
            Severity::Error,
            "closing delimiter has no matching opener",
            Span::new(start, self.pos),
        );
        self.operand_consumed();
    }

    // ── trivia ───────────────────────────────────────────────────────

    fn scan_newline(&mut self, start: usize) {
        if self.byte(self.pos) == Some(b'\r') && self.byte(self.pos + 1) == Some(b'\n') {
            self.pos += 2;
        } else {
            self.pos += 1;
        }
        self.emit(TokenKind::Newline, start, TokenFlags::empty());
        match self.frame_kind() {
            FrameKind::Paren | FrameKind::Bracket { .. } => {}
            FrameKind::Hash => self.set_mode(Mode::HashKey),
            FrameKind::SubExpr | FrameKind::Block => {
                // A newline continues the statement only when an operand is
                // still pending after a binary operator or comma
                // (`$x = 1 +` NL `2`); otherwise a new statement starts.
                let pending_operand = self.mode() == Mode::ExprOperand
                    && self
                        .last_significant()
                        .is_some_and(|k| matches!(k, TokenKind::Operator(_) | TokenKind::Comma));
                if !pending_operand {
                    let base = self.base_mode();
                    self.set_mode(base);
                }
            }
        }
    }

    /// Kind of the most recent non-trivia token, if any.
    fn last_significant(&self) -> Option<TokenKind> {
        self.tokens
            .iter()
            .rev()
            .find(|t| !t.kind.is_trivia())
            .map(|t| t.kind)
    }

    fn scan_whitespace(&mut self, start: usize) {
        let mut flags = TokenFlags::empty();
        while let Some(c) = self.peek() {
            if is_horizontal_ws(c) || c == '\u{FEFF}' {
                self.pos += c.len_utf8();
            } else if c == '`' {
                match self.peek_at(1) {
                    Some('\n' | '\r') => {
                        self.pos += 1;
                        if self.byte(self.pos) == Some(b'\r')
                            && self.byte(self.pos + 1) == Some(b'\n')
                        {
                            self.pos += 2;
                        } else {
                            self.pos += 1;
                        }
                        flags |= TokenFlags::LINE_CONTINUATION;
                    }
                    Some(n) if is_horizontal_ws(n) => {
                        // Backtick + whitespace: both swallowed as trivia.
                        self.pos += 1;
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }
        self.emit(TokenKind::Whitespace, start, flags);
    }

    fn scan_line_comment(&mut self, start: usize) {
        while let Some(c) = self.peek() {
            if c == '\n' || c == '\r' {
                break;
            }
            self.pos += c.len_utf8();
        }
        self.emit(TokenKind::LineComment, start, TokenFlags::empty());
    }

    fn scan_block_comment(&mut self, start: usize) {
        self.pos += 2; // <#
        let mut flags = TokenFlags::empty();
        loop {
            if self.pos >= self.src.len() {
                flags |= TokenFlags::UNTERMINATED;
                self.diag(
                    DiagnosticCode::UnterminatedComment,
                    Severity::Error,
                    "block comment is missing its closing '#>'",
                    Span::new(start, self.pos),
                );
                break;
            }
            if self.starts_with("#>") {
                self.pos += 2;
                break;
            }
            let c = self.peek().unwrap_or('\0');
            self.pos += c.len_utf8().max(1);
        }
        self.emit(TokenKind::BlockComment, start, flags);
    }

    // ── strings ──────────────────────────────────────────────────────

    fn scan_single_quoted(&mut self, start: usize) {
        let quote = self.peek().unwrap_or('\'');
        self.pos += quote.len_utf8();
        let mut flags = TokenFlags::empty();
        loop {
            let Some(c) = self.peek() else {
                flags |= TokenFlags::UNTERMINATED;
                self.diag(
                    DiagnosticCode::UnterminatedString,
                    Severity::Error,
                    "string is missing its terminating quote",
                    Span::new(start, self.pos),
                );
                break;
            };
            self.pos += c.len_utf8();
            if is_single_quote(c) {
                if self.peek().is_some_and(is_single_quote) {
                    let n = self.peek().unwrap_or('\'');
                    self.pos += n.len_utf8();
                } else {
                    break;
                }
            }
        }
        self.emit(TokenKind::StringLiteral, start, flags);
    }

    fn scan_double_quoted(&mut self, start: usize) {
        let quote = self.peek().unwrap_or('"');
        self.pos += quote.len_utf8();
        let mut flags = TokenFlags::empty();
        loop {
            let Some(c) = self.peek() else {
                flags |= TokenFlags::UNTERMINATED;
                self.diag(
                    DiagnosticCode::UnterminatedString,
                    Severity::Error,
                    "string is missing its terminating quote",
                    Span::new(start, self.pos),
                );
                break;
            };
            if c == '`' {
                self.pos += 1;
                if let Some(n) = self.peek() {
                    self.pos += n.len_utf8();
                }
                continue;
            }
            if c == '$' && self.peek_at(1) == Some('(') {
                self.pos += 2;
                self.skip_subexpression_parens();
                continue;
            }
            self.pos += c.len_utf8();
            if is_double_quote(c) {
                if self.peek().is_some_and(is_double_quote) {
                    let n = self.peek().unwrap_or('"');
                    self.pos += n.len_utf8();
                } else {
                    break;
                }
            }
        }
        self.emit(TokenKind::StringExpandable, start, flags);
    }

    /// Consume a `$( ... )` embedded in an expandable string or generic
    /// token. PowerShell scans these by *naive paren counting* — quotes and
    /// braces inside are not understood, except that a backtick skips the
    /// next char and a doubled double-quote collapses. We reproduce that
    /// faithfully so string extents match `pwsh`. `self.pos` is just past
    /// the opening `$(`.
    fn skip_subexpression_parens(&mut self) {
        let mut depth: usize = 1;
        while depth > 0 {
            let Some(c) = self.peek() else { return };
            if c == '`' {
                self.pos += 1;
                if let Some(n) = self.peek() {
                    self.pos += n.len_utf8();
                }
                continue;
            }
            if is_double_quote(c) && self.peek_at(c.len_utf8()).is_some_and(is_double_quote) {
                self.pos += c.len_utf8();
                let n = self.peek().unwrap_or('"');
                self.pos += n.len_utf8();
                continue;
            }
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            self.pos += c.len_utf8();
        }
    }

    fn scan_here_string(&mut self, start: usize, expandable: bool) {
        // self.pos at `@`; the quote follows.
        let quote = self.peek_at(1).unwrap_or('\'');
        self.pos += 1 + quote.len_utf8();
        let mut flags = TokenFlags::empty();

        // Header: only whitespace may follow before the newline.
        while self.peek().is_some_and(is_horizontal_ws) {
            let c = self.peek().unwrap_or(' ');
            self.pos += c.len_utf8();
        }
        // Terminator: a newline followed by the closing quote + `@` at
        // column 0.
        let close_single = !expandable;
        let mut found = false;
        while self.pos < self.src.len() {
            let c = self.peek().unwrap_or('\0');
            if c == '\n' || c == '\r' {
                let nl_len = if c == '\r' && self.byte(self.pos + 1) == Some(b'\n') {
                    2
                } else {
                    1
                };
                let after = self.pos + nl_len;
                let mut it = self.src[after.min(self.src.len())..].chars();
                let q = it.next();
                let matches_quote = q.is_some_and(|q| {
                    if close_single {
                        is_single_quote(q)
                    } else {
                        is_double_quote(q)
                    }
                });
                if matches_quote && it.next() == Some('@') {
                    let qlen = q.map_or(1, char::len_utf8);
                    self.pos = after + qlen + 1;
                    found = true;
                    break;
                }
                self.pos = after;
            } else {
                self.pos += c.len_utf8();
            }
        }
        if !found {
            flags |= TokenFlags::UNTERMINATED;
            self.diag(
                DiagnosticCode::UnterminatedString,
                Severity::Error,
                "here-string is missing its terminator",
                Span::new(start, self.pos),
            );
        }
        let kind = if expandable {
            TokenKind::HereStringExpandable
        } else {
            TokenKind::HereStringLiteral
        };
        self.emit(kind, start, flags);
    }

    // ── variables ────────────────────────────────────────────────────

    fn scan_variable(&mut self, start: usize) {
        self.pos += 1; // $
        match self.peek() {
            Some('{') => {
                self.pos += 1;
                let mut terminated = false;
                while let Some(c) = self.peek() {
                    if c == '`' {
                        self.pos += 1;
                        if let Some(n) = self.peek() {
                            self.pos += n.len_utf8();
                        }
                        continue;
                    }
                    self.pos += c.len_utf8();
                    if c == '}' {
                        terminated = true;
                        break;
                    }
                }
                let mut flags = TokenFlags::empty();
                if !terminated {
                    flags |= TokenFlags::UNTERMINATED;
                    self.diag(
                        DiagnosticCode::UnterminatedVariable,
                        Severity::Error,
                        "braced variable is missing its closing '}'",
                        Span::new(start, self.pos),
                    );
                }
                self.emit(TokenKind::Variable, start, flags);
                self.operand_consumed();
            }
            Some(c) if c == '$' || c == '^' || c == '?' => {
                // Special single-char variables $$ $? $^.
                self.pos += c.len_utf8();
                if self.variable_needs_generic_rescan() {
                    self.pos = start;
                    self.scan_generic(start);
                    return;
                }
                self.emit(TokenKind::Variable, start, TokenFlags::empty());
                self.operand_consumed();
            }
            Some(c) if is_variable_start(c) => {
                self.scan_variable_name();
                if self.variable_needs_generic_rescan() {
                    // Command mode: `$TestDrive\out.txt` is one generic
                    // word, not a variable followed by text.
                    self.pos = start;
                    self.scan_generic(start);
                    return;
                }
                self.emit(TokenKind::Variable, start, TokenFlags::empty());
                self.operand_consumed();
            }
            _ => {
                // `$` followed by nothing variable-ish: in command-argument
                // position it glues into a generic word (`$+`); elsewhere it
                // stands alone.
                if self.mode() == Mode::CommandArgs {
                    self.scan_generic(start);
                } else {
                    self.emit(TokenKind::Variable, start, TokenFlags::empty());
                    self.operand_consumed();
                }
            }
        }
    }

    /// In command-argument mode, a variable followed by a character that is
    /// neither a clean terminator nor a member/index operator makes the
    /// whole token a generic word (PowerShell's rescan-as-generic rule).
    fn variable_needs_generic_rescan(&self) -> bool {
        if !matches!(self.mode(), Mode::CommandArgs | Mode::DefinitionName) {
            return false;
        }
        match self.peek() {
            None => false,
            Some(c) => !(force_start_new_token(c) || matches!(c, '.' | '[' | '=')),
        }
    }

    /// Scan an unbraced variable name (after `$`/`@`). Accepts word chars
    /// and `:` (kept unless followed by another `:`, which belongs to a
    /// static-member operator).
    fn scan_variable_name(&mut self) {
        while let Some(c) = self.peek() {
            if is_variable_char(c) {
                self.pos += c.len_utf8();
            } else if c == ':' && self.peek_at(1) != Some(':') {
                // A trailing colon stays inside the token (matching
                // PowerShell, which reports it as an error later).
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    // ── numbers ──────────────────────────────────────────────────────

    /// Attempt to scan a numeric literal, optionally signed. Returns false
    /// (restoring position) if the characters do not form a number that
    /// terminates properly; the caller then falls back to generic scanning.
    fn try_scan_number(&mut self, start: usize, signed: bool) -> bool {
        let saved = self.pos;
        let mut flags = TokenFlags::empty();
        let bytes = self.bytes;
        let mut i = self.pos;

        if signed {
            // The sign dash may be any Unicode dash.
            let c = self.char_at(i).unwrap_or('-');
            i += c.len_utf8();
        }

        let hex_or_binary = if bytes.get(i) == Some(&b'0') {
            match bytes.get(i + 1) {
                Some(b'x' | b'X') => {
                    i += 2;
                    let s = i;
                    while bytes.get(i).is_some_and(u8::is_ascii_hexdigit) {
                        i += 1;
                    }
                    if i == s {
                        self.pos = saved;
                        return false;
                    }
                    true
                }
                Some(b'b' | b'B') => {
                    i += 2;
                    let s = i;
                    while bytes.get(i).is_some_and(|b| matches!(b, b'0' | b'1')) {
                        i += 1;
                    }
                    if i == s {
                        self.pos = saved;
                        return false;
                    }
                    true
                }
                _ => false,
            }
        } else {
            false
        };

        if !hex_or_binary {
            let digits_start = i;
            while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if bytes.get(i) == Some(&b'.') {
                // `1..5`: the dot belongs to the range operator.
                if bytes.get(i + 1) != Some(&b'.') {
                    i += 1;
                    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                        i += 1;
                    }
                }
            }
            if i == digits_start {
                self.pos = saved;
                return false;
            }
            if matches!(bytes.get(i), Some(b'e' | b'E')) {
                let mut j = i + 1;
                if matches!(bytes.get(j), Some(b'+' | b'-')) {
                    j += 1;
                }
                if bytes.get(j).is_some_and(u8::is_ascii_digit) {
                    i = j;
                    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                        i += 1;
                    }
                }
            }
        }

        // Type suffixes (u[lsy] | l | s | d | y | n), then kb/mb/gb/tb/pb.
        let suffix_start = i;
        let lower = |b: u8| b.to_ascii_lowercase();
        match bytes.get(i).map(|b| lower(*b)) {
            Some(b'u') => {
                i += 1;
                if matches!(bytes.get(i).map(|b| lower(*b)), Some(b'l' | b's' | b'y')) {
                    i += 1;
                }
            }
            Some(b'l' | b'd' | b'n' | b'y' | b's') => {
                i += 1;
            }
            _ => {}
        }
        if let (Some(a), Some(b)) = (bytes.get(i).map(|b| lower(*b)), bytes.get(i + 1)) {
            if matches!(a, b'k' | b'm' | b'g' | b't' | b'p') && lower(*b) == b'b' {
                i += 2;
            }
        }
        if i > suffix_start {
            flags |= TokenFlags::NUMBER_SUFFIXED;
        }

        // Termination: ForceStartNewToken always ends a number; the
        // expression-mode extras only apply outside command-argument
        // position.
        let ternary = self.mode() == Mode::ExprOperand;
        let terminated = match self.src[i.min(self.src.len())..].chars().next() {
            None => true,
            Some(c) => {
                force_start_new_token(c)
                    || (self.mode().number_expression_rules() && ends_number_expression(c, ternary))
            }
        };
        if !terminated {
            self.pos = saved;
            return false;
        }
        self.pos = i;
        self.emit(TokenKind::Number, start, flags);
        self.operand_consumed();
        true
    }

    // ── dash: operators and parameters ───────────────────────────────

    fn scan_dash(&mut self, start: usize) {
        let dash = self.peek().unwrap_or('-');
        let dlen = dash.len_utf8();
        let mode = self.mode();
        let after = self.peek_at(dlen);

        // `-=`
        if after == Some('=') {
            self.pos += dlen + 1;
            self.emit(
                TokenKind::Operator(OperatorKind::Assignment),
                start,
                TokenFlags::empty(),
            );
            self.set_mode(Mode::StatementStart);
            return;
        }

        // `--` and `--%`.
        if after.is_some_and(is_dash) {
            let second_len = after.map_or(1, char::len_utf8);
            let third = self.peek_at(dlen + second_len);
            if third == Some('%') {
                // `--%` stop-parsing: verbatim to end of line, or an
                // unquoted `|` / `&&`.
                self.pos += dlen + second_len + 1;
                self.emit(
                    TokenKind::Operator(OperatorKind::StopParsing),
                    start,
                    TokenFlags::empty(),
                );
                self.scan_verbatim_argument();
                return;
            }
            if mode != Mode::CommandArgs {
                self.pos += dlen + second_len;
                self.emit(
                    TokenKind::Operator(OperatorKind::Unary),
                    start,
                    TokenFlags::empty(),
                );
                if mode == Mode::ExprOperator {
                    self.operand_consumed();
                } else {
                    self.set_mode(Mode::ExprOperand);
                }
                return;
            }
            // In argument position `--…` is a generic word (e.g. `--help`).
            self.scan_generic(start);
            return;
        }

        // Signed numbers in operand position: `-5` is one Number token.
        if matches!(mode, Mode::StatementStart | Mode::ExprOperand)
            && after.is_some_and(|c| c.is_ascii_digit() || c == '.')
            && self.try_scan_number(start, true)
        {
            return;
        }

        // Dash followed by a parameter/operator word (letter, `_` or `?`).
        if after.is_some_and(|c| is_identifier_start(c) || c == '?') {
            match mode {
                Mode::CommandArgs | Mode::DefinitionName | Mode::HashKey => {
                    self.scan_parameter_command_mode(start, dlen);
                }
                _ => {
                    // Expression-ish position: operator lookup.
                    let word_start = self.pos + dlen;
                    let mut end = word_start;
                    for ch in self.src[word_start..].chars() {
                        if is_word_char(ch) {
                            end += ch.len_utf8();
                        } else {
                            break;
                        }
                    }
                    let word = &self.src[word_start..end];
                    if let Some(kind) = classify_dash_word(word) {
                        self.pos = end;
                        self.emit(TokenKind::Operator(kind), start, TokenFlags::DASH_WORD);
                        self.set_mode(Mode::ExprOperand);
                        if matches!(kind, OperatorKind::ComparisonWord) {
                            // Comparison operators put the RHS back into
                            // pipeline-element position semantically, but
                            // operand mode is the right lexing behavior.
                        }
                    } else {
                        // Not an operator: a parameter token (e.g. inside
                        // `param(...)` or malformed expressions).
                        self.pos = end;
                        if self.peek() == Some(':') {
                            self.pos += 1;
                        }
                        self.emit(TokenKind::Parameter, start, TokenFlags::DASH_WORD);
                    }
                }
            }
            return;
        }

        // Bare dash.
        if mode == Mode::CommandArgs {
            // `-3`, `-$x`, or a lone `-`: one generic argument.
            self.scan_generic(start);
            return;
        }
        self.pos += dlen;
        let kind = if mode == Mode::ExprOperator {
            OperatorKind::Binary
        } else {
            OperatorKind::Unary
        };
        self.emit(TokenKind::Operator(kind), start, TokenFlags::empty());
        self.set_mode(Mode::ExprOperand);
    }

    /// Command-mode parameter scan (`ScanParameter`): accepts letters,
    /// digits, dashes, `_`, `?` and most other chars; stops at whitespace,
    /// `{ } ( ) ; , | & . [` and newlines; a `:` is consumed and ends the
    /// name; a quote makes the whole token generic.
    fn scan_parameter_command_mode(&mut self, start: usize, dash_len: usize) {
        let mut i = self.pos + dash_len;
        while let Some(c) = self.char_at(i) {
            if is_horizontal_ws(c)
                || matches!(
                    c,
                    '{' | '}' | '(' | ')' | ';' | ',' | '|' | '&' | '.' | '[' | '\r' | '\n' | '\0'
                )
            {
                break;
            }
            if c == ':' {
                i += 1;
                break;
            }
            if is_single_quote(c) || is_double_quote(c) || c == '$' || c == '`' {
                // Rescan the whole thing as one generic token.
                self.scan_generic(start);
                return;
            }
            i += c.len_utf8();
        }
        self.pos = i;
        self.emit(TokenKind::Parameter, start, TokenFlags::DASH_WORD);
    }

    /// After `--%`: raw text until end of line, or an unquoted `|`/`&&`.
    fn scan_verbatim_argument(&mut self) {
        // Leading whitespace stays ordinary trivia.
        let ws_start = self.pos;
        while self.peek().is_some_and(is_horizontal_ws) {
            let c = self.peek().unwrap_or(' ');
            self.pos += c.len_utf8();
        }
        if self.pos > ws_start {
            self.emit(TokenKind::Whitespace, ws_start, TokenFlags::empty());
        }
        let raw_start = self.pos;
        let mut in_quotes = false;
        while let Some(c) = self.peek() {
            if c == '\n' || c == '\r' {
                break;
            }
            if is_double_quote(c) {
                in_quotes = !in_quotes;
            } else if !in_quotes {
                if c == '|' {
                    break;
                }
                if c == '&' && self.peek_at(1) == Some('&') {
                    break;
                }
            }
            self.pos += c.len_utf8();
        }
        if self.pos > raw_start {
            self.emit(TokenKind::RawArgument, raw_start, TokenFlags::empty());
        }
    }

    // ── symbolic operators ───────────────────────────────────────────

    /// Scan redirection/symbolic operators. Returns false if the current
    /// character does not begin one.
    fn scan_symbolic(&mut self, start: usize) -> bool {
        let mode = self.mode();
        let rest = &self.src[self.pos..];

        // Redirections: >, >>, n>, n>>, n>&m, *>, *>>, *>&m, <.
        if let Some(len) = redirection_len(rest) {
            self.pos += len;
            self.emit(
                TokenKind::Operator(OperatorKind::Redirection),
                start,
                TokenFlags::empty(),
            );
            // Redirection targets are command-mode arguments.
            if !matches!(mode, Mode::ExprOperand) {
                self.set_mode(Mode::CommandArgs);
            }
            return true;
        }

        // `!`: generic when starting a word (`!true` is a bare word),
        // operator when followed by space/digit/`$`/`(` etc.
        if rest.starts_with('!') {
            if self.peek_at(1).is_some_and(is_identifier_start) {
                self.scan_generic(start);
                if mode == Mode::StatementStart {
                    if let Some(t) = self.tokens.last_mut() {
                        t.flags |= TokenFlags::COMMAND_NAME;
                    }
                    self.set_mode(Mode::CommandArgs);
                }
                return true;
            }
            self.pos += 1;
            self.emit(
                TokenKind::Operator(OperatorKind::Not),
                start,
                TokenFlags::empty(),
            );
            self.set_mode(Mode::ExprOperand);
            return true;
        }

        // `?` family.
        if rest.starts_with('?') {
            if rest.starts_with("??=") {
                self.pos += 3;
                self.emit(
                    TokenKind::Operator(OperatorKind::Assignment),
                    start,
                    TokenFlags::empty(),
                );
                self.set_mode(Mode::StatementStart);
                return true;
            }
            if rest.starts_with("??") {
                self.pos += 2;
                self.emit(
                    TokenKind::Operator(OperatorKind::NullCoalesce),
                    start,
                    TokenFlags::empty(),
                );
                self.set_mode(Mode::ExprOperand);
                return true;
            }
            if rest.starts_with("?.") {
                self.pos += 2;
                self.emit(
                    TokenKind::Operator(OperatorKind::MemberAccess),
                    start,
                    TokenFlags::empty(),
                );
                self.set_mode(Mode::ExprOperand);
                return true;
            }
            if rest.starts_with("?[") {
                self.pos += 2;
                self.emit(
                    TokenKind::Operator(OperatorKind::NullConditionalIndex),
                    start,
                    TokenFlags::empty(),
                );
                self.push_frame(FrameKind::Bracket { index: true }, Mode::ExprOperand);
                return true;
            }
            match mode {
                Mode::ExprOperator => {
                    self.pos += 1;
                    self.emit(
                        TokenKind::Operator(OperatorKind::TernaryQuestion),
                        start,
                        TokenFlags::empty(),
                    );
                    // Ternary branches are expression contexts.
                    self.set_mode(Mode::ExprOperand);
                    return true;
                }
                Mode::StatementStart | Mode::CommandArgs | Mode::DefinitionName => {
                    // `?` = Where-Object alias / wildcard argument.
                    self.scan_generic(start);
                    if mode == Mode::StatementStart {
                        if let Some(t) = self.tokens.last_mut() {
                            t.flags |= TokenFlags::COMMAND_NAME;
                        }
                        self.set_mode(Mode::CommandArgs);
                    }
                    return true;
                }
                _ => {
                    self.pos += 1;
                    self.emit(
                        TokenKind::Operator(OperatorKind::TernaryQuestion),
                        start,
                        TokenFlags::empty(),
                    );
                    return true;
                }
            }
        }

        // `:` — `::`, labels, ternary colon.
        if rest.starts_with(':') {
            if rest.starts_with("::") {
                self.pos += 2;
                self.emit(
                    TokenKind::Operator(OperatorKind::MemberAccess),
                    start,
                    TokenFlags::empty(),
                );
                if mode == Mode::CommandArgs {
                    self.set_mode(Mode::ArgMemberName);
                } else {
                    self.set_mode(Mode::ExprOperand);
                }
                return true;
            }
            if mode == Mode::StatementStart && self.peek_at(1).is_some_and(is_identifier_start) {
                self.pos += 1;
                while self.peek().is_some_and(is_word_char) {
                    let c = self.peek().unwrap_or('a');
                    self.pos += c.len_utf8();
                }
                self.emit(TokenKind::Label, start, TokenFlags::empty());
                return true;
            }
            if matches!(mode, Mode::CommandArgs | Mode::DefinitionName) {
                // A stray colon in arguments is argument text.
                self.scan_generic(start);
                return true;
            }
            self.pos += 1;
            self.emit(
                TokenKind::Operator(OperatorKind::TernaryColon),
                start,
                TokenFlags::empty(),
            );
            // The alternative branch of a ternary is an expression.
            self.set_mode(Mode::ExprOperand);
            return true;
        }

        // `.` — member access, dot-source, `..`, or generic.
        if rest.starts_with('.') {
            if rest.starts_with("..") {
                if mode == Mode::CommandArgs
                    || (mode == Mode::StatementStart
                        && self.peek_at(2).is_some_and(|c| !force_start_new_token(c)))
                {
                    // `..\dir` — one generic word.
                    self.scan_generic(start);
                    if mode == Mode::StatementStart {
                        if let Some(t) = self.tokens.last_mut() {
                            t.flags |= TokenFlags::COMMAND_NAME;
                        }
                        self.set_mode(Mode::CommandArgs);
                    }
                    return true;
                }
                self.pos += 2;
                self.emit(
                    TokenKind::Operator(OperatorKind::Binary),
                    start,
                    TokenFlags::empty(),
                );
                self.set_mode(Mode::ExprOperand);
                return true;
            }
            match mode {
                Mode::ExprOperator => {
                    self.pos += 1;
                    self.emit(
                        TokenKind::Operator(OperatorKind::MemberAccess),
                        start,
                        TokenFlags::empty(),
                    );
                    self.set_mode(Mode::ExprOperand);
                    return true;
                }
                Mode::StatementStart | Mode::DefinitionName => {
                    let next = self.peek_at(1);
                    if next.is_none()
                        || next.is_some_and(|c| {
                            force_start_new_token(c)
                                || c == '$'
                                || is_single_quote(c)
                                || is_double_quote(c)
                        })
                    {
                        // Dot-source: `. cmd`, `. $script`, `. "path"`,
                        // `. {block}`.
                        self.pos += 1;
                        self.emit(
                            TokenKind::Operator(OperatorKind::Invocation),
                            start,
                            TokenFlags::empty(),
                        );
                        self.set_mode(Mode::DefinitionName);
                        return true;
                    }
                    // `.\script.ps1` — a command word.
                    self.scan_generic(start);
                    if let Some(t) = self.tokens.last_mut() {
                        t.flags |= TokenFlags::COMMAND_NAME;
                    }
                    self.set_mode(Mode::CommandArgs);
                    return true;
                }
                Mode::CommandArgs => {
                    if self.adjacent_primary() {
                        // `$x.Length` in argument position: member access.
                        self.pos += 1;
                        self.emit(
                            TokenKind::Operator(OperatorKind::MemberAccess),
                            start,
                            TokenFlags::empty(),
                        );
                        self.set_mode(Mode::ArgMemberName);
                        return true;
                    }
                    self.scan_generic(start);
                    return true;
                }
                _ => {
                    self.pos += 1;
                    self.emit(
                        TokenKind::Operator(OperatorKind::MemberAccess),
                        start,
                        TokenFlags::empty(),
                    );
                    return true;
                }
            }
        }

        // Remaining symbolic operators. In command-argument position, glued
        // operator chars belong to generic words (`2+2` is one argument);
        // standalone ones (followed by a force-start char) are operators.
        let (text, kind): (&str, OperatorKind) = if rest.starts_with("+=") {
            ("+=", OperatorKind::Assignment)
        } else if rest.starts_with("*=") {
            ("*=", OperatorKind::Assignment)
        } else if rest.starts_with("/=") {
            ("/=", OperatorKind::Assignment)
        } else if rest.starts_with("%=") {
            ("%=", OperatorKind::Assignment)
        } else if rest.starts_with("++") {
            ("++", OperatorKind::Unary)
        } else if rest.starts_with('=') {
            ("=", OperatorKind::Assignment)
        } else if rest.starts_with('+') {
            ("+", OperatorKind::Binary)
        } else if rest.starts_with('*') {
            ("*", OperatorKind::Binary)
        } else if rest.starts_with('/') {
            ("/", OperatorKind::Binary)
        } else if rest.starts_with('%') {
            ("%", OperatorKind::Binary)
        } else {
            return false;
        };

        if mode == Mode::CommandArgs {
            // In argument position operator characters are argument text:
            // glued they extend a word (`2+2`), standalone they are still
            // retagged as generic arguments by PowerShell's parser.
            self.scan_generic(start);
            return true;
        }
        if mode == Mode::StatementStart && (text == "%" || text == "*") {
            let follow = self.peek_at(text.len());
            if follow.is_none_or(force_start_new_token) {
                // `%` / `*` as a command name (ForEach-Object alias, wildcard).
                self.pos += text.len();
                self.emit(TokenKind::Generic, start, TokenFlags::COMMAND_NAME);
                self.set_mode(Mode::CommandArgs);
                return true;
            }
            self.scan_generic(start);
            if let Some(t) = self.tokens.last_mut() {
                t.flags |= TokenFlags::COMMAND_NAME;
            }
            self.set_mode(Mode::CommandArgs);
            return true;
        }

        // Signed number after `+` in operand position.
        if text == "+"
            && matches!(mode, Mode::StatementStart | Mode::ExprOperand)
            && self
                .peek_at(1)
                .is_some_and(|c| c.is_ascii_digit() || c == '.')
            && self.try_scan_number(start, true)
        {
            return true;
        }

        self.pos += text.len();
        self.emit(TokenKind::Operator(kind), start, TokenFlags::empty());
        match kind {
            OperatorKind::Assignment => self.set_mode(Mode::StatementStart),
            OperatorKind::Unary if mode == Mode::ExprOperator => self.operand_consumed(),
            _ => self.set_mode(Mode::ExprOperand),
        }
        true
    }

    // ── words & generic arguments ────────────────────────────────────

    fn scan_word_or_generic(&mut self, start: usize) {
        let mode = self.mode();

        if mode == Mode::CommandArgs {
            self.scan_generic(start);
            return;
        }
        if mode == Mode::UsingDirective {
            let mut end = self.pos;
            for ch in self.src[self.pos..].chars() {
                if is_word_char(ch) {
                    end += ch.len_utf8();
                } else {
                    break;
                }
            }
            let word = &self.src[self.pos..end];
            let kw = match () {
                () if word.eq_ignore_ascii_case("namespace") => Some(Keyword::Namespace),
                () if word.eq_ignore_ascii_case("module") => Some(Keyword::Module),
                () if word.eq_ignore_ascii_case("assembly") => Some(Keyword::Assembly),
                () => None,
            };
            if let Some(kw) = kw
                && self.char_at(end).is_none_or(force_start_new_token)
            {
                let mut flags = TokenFlags::empty();
                if word != kw.canonical() {
                    flags |= TokenFlags::NONSTANDARD_CASE;
                }
                self.pos = end;
                self.emit(TokenKind::Keyword(kw), start, flags);
                self.set_mode(Mode::DefinitionName);
                return;
            }
            self.scan_generic(start);
            self.set_mode(Mode::CommandArgs);
            return;
        }
        if mode == Mode::ArgMemberName {
            // A simple member name; argument mode resumes after it.
            let mut end = self.pos;
            for ch in self.src[self.pos..].chars() {
                if is_word_char(ch) {
                    end += ch.len_utf8();
                } else {
                    break;
                }
            }
            if end > self.pos {
                self.pos = end;
                self.emit(TokenKind::Identifier, start, TokenFlags::empty());
                self.set_mode(Mode::CommandArgs);
                return;
            }
            self.scan_generic(start);
            self.set_mode(Mode::CommandArgs);
            return;
        }

        // Type-name scanning inside `[ ... ]` type literals: dots, nested
        // type separators, and arity backticks stay in one identifier
        // (`System.Collections.Generic.Dictionary`, ``List`1``).
        if matches!(self.frame_kind(), FrameKind::Bracket { index: false })
            && mode == Mode::ExprOperand
        {
            let mut end = self.pos;
            for ch in self.src[self.pos..].chars() {
                if is_word_char(ch) || matches!(ch, '.' | '\\' | '+' | '#' | '`') {
                    end += ch.len_utf8();
                } else {
                    break;
                }
            }
            if end > self.pos {
                self.pos = end;
                self.emit(TokenKind::Identifier, start, TokenFlags::empty());
                self.operand_consumed();
                return;
            }
        }

        let mut end = self.pos;
        for ch in self.src[self.pos..].chars() {
            if is_word_char(ch) {
                end += ch.len_utf8();
            } else {
                break;
            }
        }
        if end == self.pos {
            self.scan_generic(start);
            return;
        }
        let word = &self.src[self.pos..end];
        let next = self.char_at(end);

        match mode {
            Mode::StatementStart => {
                let glued = next.is_some_and(|c| !force_start_new_token(c));
                if !glued && let Some(kw) = Keyword::lookup(word) {
                    self.pos = end;
                    let mut flags = TokenFlags::empty();
                    if word != kw.canonical() {
                        flags |= TokenFlags::NONSTANDARD_CASE;
                    }
                    self.emit(TokenKind::Keyword(kw), start, flags);
                    if matches!(kw, Keyword::Class | Keyword::Enum) {
                        self.pending_signature_body = true;
                    }
                    self.set_mode(keyword_following_mode(kw));
                    return;
                }
                self.scan_generic(start);
                if let Some(tok) = self.tokens.last_mut() {
                    tok.flags |= TokenFlags::COMMAND_NAME;
                }
                self.set_mode(Mode::CommandArgs);
            }
            Mode::HashKey => {
                self.pos = end;
                self.emit(TokenKind::Identifier, start, TokenFlags::empty());
                self.set_mode(Mode::ExprOperator);
            }
            Mode::DefinitionName => {
                // Function/class names may contain dashes, dots, etc.
                self.scan_generic(start);
                self.set_mode(Mode::CommandArgs);
            }
            Mode::ExprOperand
            | Mode::ExprOperator
            | Mode::CommandArgs
            | Mode::ArgMemberName
            | Mode::UsingDirective => {
                self.pos = end;
                // `foreach ($x in $y)`: `in` is a keyword after the loop
                // variable inside the foreach parens.
                if mode == Mode::ExprOperator
                    && matches!(self.frame_kind(), FrameKind::Paren)
                    && word.eq_ignore_ascii_case("in")
                {
                    let mut flags = TokenFlags::empty();
                    if word != "in" {
                        flags |= TokenFlags::NONSTANDARD_CASE;
                    }
                    self.emit(TokenKind::Keyword(Keyword::In), start, flags);
                    self.set_mode(Mode::StatementStart);
                    return;
                }
                // Class/enum member modifiers keep their keyword nature in
                // signature bodies (`hidden [int] $x`, `static Foo()`).
                if mode == Mode::ExprOperand
                    && self.in_signature_body()
                    && let Some(kw) = Keyword::lookup(word)
                    && matches!(kw, Keyword::Hidden | Keyword::Static)
                {
                    let mut flags = TokenFlags::empty();
                    if word != kw.canonical() {
                        flags |= TokenFlags::NONSTANDARD_CASE;
                    }
                    self.emit(TokenKind::Keyword(kw), start, flags);
                    // Modifier: another member declaration element follows.
                    self.set_mode(Mode::ExprOperand);
                    return;
                }
                self.emit(TokenKind::Identifier, start, TokenFlags::empty());
                self.operand_consumed();
            }
        }
    }

    /// Scan a generic command-argument token: everything up to a
    /// `ForceStartNewToken` char, absorbing backtick escapes, quoted
    /// segments, variables, and `$( ... )` subexpressions.
    fn scan_generic(&mut self, start: usize) {
        while let Some(c) = self.peek() {
            if force_start_new_token(c) {
                break;
            }
            match c {
                '`' => {
                    self.pos += 1;
                    match self.peek() {
                        Some(n) if n == '\n' || n == '\r' => {
                            // The continuation belongs to trivia.
                            self.pos -= 1;
                            break;
                        }
                        Some(n) => self.pos += n.len_utf8(),
                        None => break,
                    }
                }
                '$' => {
                    if self.peek_at(1) == Some('(') {
                        self.pos += 2;
                        self.skip_subexpression_parens();
                    } else if self.peek_at(1) == Some('{') {
                        let s = self.pos;
                        let count = self.tokens.len();
                        self.scan_variable(s);
                        self.tokens.truncate(count);
                    } else {
                        self.pos += 1;
                        self.scan_variable_name();
                    }
                }
                q if is_single_quote(q) => {
                    let s = self.pos;
                    let count = self.tokens.len();
                    self.scan_single_quoted(s);
                    self.tokens.truncate(count);
                }
                q if is_double_quote(q) => {
                    let s = self.pos;
                    let count = self.tokens.len();
                    self.scan_double_quoted(s);
                    self.tokens.truncate(count);
                }
                _ => self.pos += c.len_utf8(),
            }
        }
        if self.pos == start {
            if let Some(c) = self.peek() {
                self.pos += c.len_utf8();
            }
        }
        self.emit(TokenKind::Generic, start, TokenFlags::IN_COMMAND_ARGS);
    }
}

/// Mode adopted right after a keyword token.
fn keyword_following_mode(kw: Keyword) -> Mode {
    match kw {
        Keyword::Function
        | Keyword::Filter
        | Keyword::Workflow
        | Keyword::Configuration
        | Keyword::Class
        | Keyword::Enum
        | Keyword::Data => Mode::DefinitionName,
        Keyword::Using => Mode::UsingDirective,
        _ => Mode::StatementStart,
    }
}

/// Length of a redirection operator at the start of `rest`, if any.
fn redirection_len(rest: &str) -> Option<usize> {
    let b = rest.as_bytes();
    let mut i = 0;
    match b.first()? {
        b'1'..=b'6' | b'*' => {
            if b.get(1) == Some(&b'>') {
                i = 1;
            } else {
                return None;
            }
        }
        b'>' => {}
        b'<' => return Some(1),
        _ => return None,
    }
    i += 1; // consume '>'
    if b.get(i) == Some(&b'>') {
        return Some(i + 1);
    }
    if b.get(i) == Some(&b'&') && matches!(b.get(i + 1), Some(b'1' | b'2')) {
        return Some(i + 2);
    }
    Some(i)
}

/// Classify a dash word as an operator, if it is one. Mirrors PowerShell's
/// `s_operatorTable` (case-insensitive; `c`/`i` prefixed variants).
fn classify_dash_word(word: &str) -> Option<OperatorKind> {
    let mut lower = [0u8; 16];
    if word.len() > 15 || !word.is_ascii() {
        return None;
    }
    let l = &mut lower[..word.len()];
    l.copy_from_slice(word.as_bytes());
    l.make_ascii_lowercase();
    let w = core::str::from_utf8(l).ok()?;

    let base = match w.strip_prefix(['c', 'i']) {
        Some(stripped) if is_prefixable_op(stripped) => stripped,
        _ => w,
    };

    match base {
        "eq" | "ne" | "gt" | "ge" | "lt" | "le" | "like" | "notlike" | "match" | "notmatch"
        | "replace" | "contains" | "notcontains" | "in" | "notin" | "split" | "join" | "is"
        | "isnot" | "as" | "band" | "bor" | "bxor" | "shl" | "shr" | "and" | "or" | "xor" | "f" => {
            Some(OperatorKind::ComparisonWord)
        }
        "not" | "bnot" => Some(OperatorKind::UnaryWord),
        _ => None,
    }
}

fn is_prefixable_op(base: &str) -> bool {
    matches!(
        base,
        "eq" | "ne"
            | "gt"
            | "ge"
            | "lt"
            | "le"
            | "like"
            | "notlike"
            | "match"
            | "notmatch"
            | "replace"
            | "contains"
            | "notcontains"
            | "in"
            | "notin"
            | "split"
            | "join"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        tokenize(src)
            .tokens
            .iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| t.kind)
            .collect()
    }

    fn texts(src: &str) -> Vec<String> {
        tokenize(src)
            .tokens
            .iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| t.text(src).to_owned())
            .collect()
    }

    #[test]
    fn generic_fallback_for_numbers() {
        assert_eq!(texts("echo 77z.exe"), ["echo", "77z.exe"]);
        assert_eq!(texts("echo 2+2"), ["echo", "2+2"]);
        assert_eq!(
            kinds("$x = 2 + 2")[2..],
            [
                TokenKind::Number,
                TokenKind::Operator(OperatorKind::Binary),
                TokenKind::Number
            ]
        );
        assert_eq!(texts("echo 1kb")[1], "1kb");
        assert_eq!(kinds("echo 1kb")[1], TokenKind::Number);
        assert_eq!(kinds("echo 2k")[1], TokenKind::Generic);
    }

    #[test]
    fn ternary_variable_glue() {
        // `$true?2:3` is a single variable token (verified against pwsh).
        assert_eq!(texts("$true?2:3"), ["$true?2:3"]);
        assert_eq!(kinds("$true?2:3"), [TokenKind::Variable]);
        let k = kinds("$true ? 2 : 3");
        assert_eq!(
            k,
            [
                TokenKind::Variable,
                TokenKind::Operator(OperatorKind::TernaryQuestion),
                TokenKind::Number,
                TokenKind::Operator(OperatorKind::TernaryColon),
                TokenKind::Number,
            ]
        );
    }

    #[test]
    fn range_operator_carveout() {
        assert_eq!(
            kinds("1..5"),
            [
                TokenKind::Number,
                TokenKind::Operator(OperatorKind::Binary),
                TokenKind::Number
            ]
        );
        assert_eq!(texts("write 1..2"), ["write", "1..2"]);
    }

    #[test]
    fn variables_with_scopes_and_specials() {
        assert_eq!(texts("$env:PATH.Length")[0], "$env:PATH");
        assert_eq!(texts("$$ $? $^ $_"), ["$$", "$?", "$^", "$_"]);
        assert_eq!(texts("${a`}b}c"), ["${a`}b}", "c"]);
        assert_eq!(kinds("@args"), [TokenKind::SplattedVariable]);
    }

    #[test]
    fn redirections() {
        assert_eq!(
            kinds("cmd 2>&1")[1],
            TokenKind::Operator(OperatorKind::Redirection)
        );
        assert_eq!(texts("cmd 2>&1")[1], "2>&1");
        assert_eq!(texts("cmd 7>x"), ["cmd", "7>x"]);
        assert_eq!(texts("cmd *> $null")[1], "*>");
    }

    #[test]
    fn parameters() {
        assert_eq!(texts("dir -Path:*"), ["dir", "-Path:", "*"]);
        assert_eq!(kinds("dir -Path:*")[1], TokenKind::Parameter);
        assert_eq!(texts("cmd -foo-bar")[1], "-foo-bar");
        assert_eq!(kinds("cmd -foo-bar")[1], TokenKind::Parameter);
        // In expression position `-eq` is an operator.
        assert_eq!(
            kinds("$a -eq 1")[1],
            TokenKind::Operator(OperatorKind::ComparisonWord)
        );
    }

    #[test]
    fn stop_parsing() {
        let t = texts("cmd --% %USER% \"a|b\" | more");
        assert_eq!(t, ["cmd", "--%", "%USER% \"a|b\" ", "|", "more"]);
    }

    #[test]
    fn bang_words() {
        assert_eq!(kinds("!true"), [TokenKind::Generic]);
        assert_eq!(
            kinds("! $x"),
            [TokenKind::Operator(OperatorKind::Not), TokenKind::Variable]
        );
        assert_eq!(
            kinds("!5"),
            [TokenKind::Operator(OperatorKind::Not), TokenKind::Number]
        );
    }

    #[test]
    fn signed_numbers_in_expressions() {
        assert_eq!(kinds("$x = -5")[2..], [TokenKind::Number]);
        assert_eq!(texts("$x = -5")[2], "-5");
        assert_eq!(
            kinds("$x = -$y")[2..],
            [
                TokenKind::Operator(OperatorKind::Unary),
                TokenKind::Variable
            ]
        );
    }

    #[test]
    fn dot_forms() {
        assert_eq!(texts(". .\\script.ps1"), [".", ".\\script.ps1"]);
        assert_eq!(
            kinds(". .\\script.ps1")[0],
            TokenKind::Operator(OperatorKind::Invocation)
        );
        assert_eq!(kinds(".\\script.ps1"), [TokenKind::Generic]);
        assert_eq!(
            kinds("$x.Length")[1],
            TokenKind::Operator(OperatorKind::MemberAccess)
        );
    }

    #[test]
    fn labels() {
        let k = kinds(":outer while ($true) { break outer }");
        assert_eq!(k[0], TokenKind::Label);
        assert_eq!(k[1], TokenKind::Keyword(Keyword::While));
    }

    #[test]
    fn strings_with_embedded_subexpressions() {
        assert_eq!(kinds("\"a $(1 + (2)) b\""), [TokenKind::StringExpandable]);
        assert_eq!(kinds("\"$(\"\"abc\"\")\""), [TokenKind::StringExpandable]);
        // Smart quotes.
        assert_eq!(kinds("\u{2018}smart\u{2019}"), [TokenKind::StringLiteral]);
    }

    #[test]
    fn here_string_footer_column_zero() {
        let src = "@'\nline1\n '@\nstill\n'@";
        let out = tokenize(src);
        assert_eq!(out.tokens[0].kind, TokenKind::HereStringLiteral);
        assert_eq!(out.tokens[0].span.end, src.len());
    }

    #[test]
    fn parameter_glued_quote_is_generic() {
        assert_eq!(
            kinds("cmd -foo\"bar\""),
            [TokenKind::Generic, TokenKind::Generic,]
        );
        assert_eq!(texts("cmd -foo\"bar\"")[1], "-foo\"bar\"");
    }

    #[test]
    fn command_mode_bracket_wildcards() {
        assert_eq!(texts("Get-Item [a-z]*.txt")[1], "[a-z]*.txt");
        assert_eq!(kinds("[int]$x")[0], TokenKind::LBracket);
    }
}
