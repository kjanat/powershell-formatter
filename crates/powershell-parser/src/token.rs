//! Formatter-oriented token model.
//!
//! The kinds below are deliberately smaller than PowerShell's `TokenKind`:
//! they capture exactly the distinctions the formatter needs (trivia vs
//! protected text vs syntax), not the distinctions an evaluator would need.

use crate::span::Span;

/// Reserved and contextual keywords the formatter recognizes in command
/// position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Keyword {
    Begin,
    Break,
    Catch,
    Class,
    Clean,
    Configuration,
    Continue,
    Data,
    Define,
    Default,
    Do,
    DynamicParam,
    Else,
    ElseIf,
    End,
    Enum,
    Exit,
    Filter,
    Finally,
    For,
    Foreach,
    From,
    Function,
    Hidden,
    If,
    In,
    InlineScript,
    /// `using module ...` — contextual, only after `using`.
    Module,
    /// `using namespace ...` — contextual, only after `using`.
    Namespace,
    /// `using assembly ...` — contextual, only after `using`.
    Assembly,
    Parallel,
    Param,
    Process,
    Return,
    Sequence,
    Static,
    Switch,
    Throw,
    Trap,
    Try,
    Until,
    Using,
    Var,
    While,
    Workflow,
}

impl Keyword {
    /// Canonical lower-case spelling used by the casing rule.
    #[must_use]
    pub const fn canonical(self) -> &'static str {
        match self {
            Keyword::Begin => "begin",
            Keyword::Break => "break",
            Keyword::Catch => "catch",
            Keyword::Class => "class",
            Keyword::Clean => "clean",
            Keyword::Configuration => "configuration",
            Keyword::Continue => "continue",
            Keyword::Data => "data",
            Keyword::Define => "define",
            Keyword::Default => "default",
            Keyword::Do => "do",
            Keyword::DynamicParam => "dynamicparam",
            Keyword::Else => "else",
            Keyword::ElseIf => "elseif",
            Keyword::End => "end",
            Keyword::Enum => "enum",
            Keyword::Exit => "exit",
            Keyword::Filter => "filter",
            Keyword::Finally => "finally",
            Keyword::For => "for",
            Keyword::Foreach => "foreach",
            Keyword::From => "from",
            Keyword::Function => "function",
            Keyword::Hidden => "hidden",
            Keyword::If => "if",
            Keyword::In => "in",
            Keyword::InlineScript => "inlinescript",
            Keyword::Module => "module",
            Keyword::Namespace => "namespace",
            Keyword::Assembly => "assembly",
            Keyword::Parallel => "parallel",
            Keyword::Param => "param",
            Keyword::Process => "process",
            Keyword::Return => "return",
            Keyword::Sequence => "sequence",
            Keyword::Static => "static",
            Keyword::Switch => "switch",
            Keyword::Throw => "throw",
            Keyword::Trap => "trap",
            Keyword::Try => "try",
            Keyword::Until => "until",
            Keyword::Using => "using",
            Keyword::Var => "var",
            Keyword::While => "while",
            Keyword::Workflow => "workflow",
        }
    }

    /// Look a word up case-insensitively (ASCII, matching PowerShell).
    #[must_use]
    pub fn lookup(word: &str) -> Option<Keyword> {
        // Keywords are short ASCII words; a linear match on lowercased input
        // is fast and avoids allocation for the common non-keyword case.
        const TABLE: &[(&str, Keyword)] = &[
            ("begin", Keyword::Begin),
            ("break", Keyword::Break),
            ("catch", Keyword::Catch),
            ("class", Keyword::Class),
            ("clean", Keyword::Clean),
            ("configuration", Keyword::Configuration),
            ("continue", Keyword::Continue),
            ("data", Keyword::Data),
            ("define", Keyword::Define),
            ("default", Keyword::Default),
            ("do", Keyword::Do),
            ("dynamicparam", Keyword::DynamicParam),
            ("else", Keyword::Else),
            ("elseif", Keyword::ElseIf),
            ("end", Keyword::End),
            ("enum", Keyword::Enum),
            ("exit", Keyword::Exit),
            ("filter", Keyword::Filter),
            ("finally", Keyword::Finally),
            ("for", Keyword::For),
            ("foreach", Keyword::Foreach),
            ("from", Keyword::From),
            ("function", Keyword::Function),
            ("hidden", Keyword::Hidden),
            ("if", Keyword::If),
            ("in", Keyword::In),
            ("inlinescript", Keyword::InlineScript),
            ("parallel", Keyword::Parallel),
            ("param", Keyword::Param),
            ("process", Keyword::Process),
            ("return", Keyword::Return),
            ("sequence", Keyword::Sequence),
            ("static", Keyword::Static),
            ("switch", Keyword::Switch),
            ("throw", Keyword::Throw),
            ("trap", Keyword::Trap),
            ("try", Keyword::Try),
            ("until", Keyword::Until),
            ("using", Keyword::Using),
            ("var", Keyword::Var),
            ("while", Keyword::While),
            ("workflow", Keyword::Workflow),
        ];
        /// Length of the longest entry in `TABLE` (`configuration`).
        const MAX_LEN: usize = 13;
        debug_assert!(TABLE.iter().all(|(name, _)| name.len() <= MAX_LEN));
        if word.len() > MAX_LEN || !word.is_ascii() {
            return None;
        }
        let mut buf = [0u8; MAX_LEN];
        let lower = &mut buf[..word.len()];
        lower.copy_from_slice(word.as_bytes());
        lower.make_ascii_lowercase();
        let lower = core::str::from_utf8(lower).ok()?;
        TABLE
            .iter()
            .find(|(name, _)| *name == lower)
            .map(|(_, kw)| *kw)
    }
}

/// Categories of operators, at the granularity the whitespace and casing
/// rules require.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OperatorKind {
    /// `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `??=`
    Assignment,
    /// Dash word operators: `-eq`, `-like`, `-and`, `-f`, `-band`, ... incl.
    /// explicit `-c`/`-i` variants. Binary in expression contexts.
    ComparisonWord,
    /// `-not`, `-bnot`, `-split`/`-join` used unary; the lexer only marks
    /// `-not`/`-bnot` here — unary use of split/join is resolved structurally.
    UnaryWord,
    /// Symbolic binary operators: `+`, `-`, `*`, `/`, `%`, `..`, `-f` format
    /// is ComparisonWord; `??`, `?:` pieces are their own kinds below.
    Binary,
    /// `!` and unary `+`/`-` when detected, `++`/`--`.
    Unary,
    /// `??`
    NullCoalesce,
    /// `?` of a ternary conditional.
    TernaryQuestion,
    /// `:` of a ternary conditional.
    TernaryColon,
    /// `.` member access / `::` static member access / `?.`
    MemberAccess,
    /// `[` used for indexing is lexed as `LBracket`; `?[` marks this kind.
    NullConditionalIndex,
    /// `&` call operator or `.` dot-source operator in command position.
    Invocation,
    /// `>`, `>>`, `2>`, `*>&1`, `<`, ...
    Redirection,
    /// `--%`
    StopParsing,
    /// `,` unary/binary array constructor when lexed as operator context;
    /// normally lexed as `TokenKind::Comma`.
    ArrayJoin,
    /// `!` alias `-not`
    Not,
}

/// The formatter-oriented token classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TokenKind {
    // ── trivia ────────────────────────────────────────────────────────
    /// Spaces/tabs and other horizontal whitespace, including a trailing
    /// backtick line continuation (see [`TokenFlags::LINE_CONTINUATION`]).
    Whitespace,
    /// One line break: LF, CRLF, or CR.
    Newline,
    /// `# ...` to end of line (excludes the newline).
    LineComment,
    /// `<# ... #>`; may span lines.
    BlockComment,

    // ── protected/opaque ──────────────────────────────────────────────
    /// `'...'` (including smart-quote variants).
    StringLiteral,
    /// `"..."` expandable string (contents scanned but kept opaque).
    StringExpandable,
    /// `@' ... '@`
    HereStringLiteral,
    /// `@" ... "@`
    HereStringExpandable,

    // ── atoms ─────────────────────────────────────────────────────────
    /// `$name`, `$scope:name`, `$$`, `$?`, `$^`, `${braced}`.
    Variable,
    /// `@name` splatted variable.
    SplattedVariable,
    /// Numeric literal.
    Number,
    /// A word in expression/command-name position (command names, member
    /// names, type-body identifiers, switch-case bare words ...).
    Identifier,
    /// A generic argument token in command-argument position
    /// (`C:\x\y.txt`, `*.ps1`, `http://x`, bare words).
    Generic,
    /// `-Name` command parameter (possibly with `:` suffix).
    Parameter,
    /// A recognized language keyword in command position.
    Keyword(Keyword),
    /// An operator; see [`OperatorKind`].
    Operator(OperatorKind),
    /// `:label` before a loop, and label arguments of break/continue.
    Label,
    /// Raw text following `--%` up to end of line.
    RawArgument,

    // ── delimiters ────────────────────────────────────────────────────
    LCurly,
    RCurly,
    /// `@{`
    AtCurly,
    LParen,
    RParen,
    /// `@(`
    AtParen,
    /// `$(`
    DollarParen,
    LBracket,
    RBracket,

    // ── separators ────────────────────────────────────────────────────
    Comma,
    Semicolon,
    /// `|`
    Pipe,
    /// `&&`
    AndAnd,
    /// `||`
    OrOr,

    /// A byte sequence the lexer could not classify. The formatter treats it
    /// as opaque and reports a diagnostic.
    Unknown,
}

impl TokenKind {
    #[must_use]
    pub const fn is_trivia(self) -> bool {
        matches!(
            self,
            TokenKind::Whitespace
                | TokenKind::Newline
                | TokenKind::LineComment
                | TokenKind::BlockComment
        )
    }

    #[must_use]
    pub const fn is_comment(self) -> bool {
        matches!(self, TokenKind::LineComment | TokenKind::BlockComment)
    }

    #[must_use]
    pub const fn is_string(self) -> bool {
        matches!(
            self,
            TokenKind::StringLiteral
                | TokenKind::StringExpandable
                | TokenKind::HereStringLiteral
                | TokenKind::HereStringExpandable
        )
    }

    #[must_use]
    pub const fn is_open_delimiter(self) -> bool {
        matches!(
            self,
            TokenKind::LCurly
                | TokenKind::AtCurly
                | TokenKind::LParen
                | TokenKind::AtParen
                | TokenKind::DollarParen
                | TokenKind::LBracket
        )
    }

    #[must_use]
    pub const fn is_close_delimiter(self) -> bool {
        matches!(
            self,
            TokenKind::RCurly | TokenKind::RParen | TokenKind::RBracket
        )
    }
}

bitflags::bitflags! {
    /// Extra lexical facts about a token.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct TokenFlags: u16 {
        /// The token was cut short by end of input (unterminated string,
        /// comment, or here-string).
        const UNTERMINATED = 1 << 0;
        /// Whitespace token that ends with a backtick line continuation
        /// (the continuation consumes the following newline).
        const LINE_CONTINUATION = 1 << 1;
        /// Token appeared in command-argument position.
        const IN_COMMAND_ARGS = 1 << 2;
        /// The token is the name element of a command invocation.
        const COMMAND_NAME = 1 << 3;
        /// Keyword/word spelled in nonstandard casing.
        const NONSTANDARD_CASE = 1 << 4;
        /// Number token that includes a multiplier or type suffix.
        const NUMBER_SUFFIXED = 1 << 5;
        /// A `-` prefixed token that could be operator or parameter; the
        /// lexer resolved it as marked but structural analysis may care.
        const DASH_WORD = 1 << 6;
        /// A `{` opening a script block passed as a command argument
        /// (`ForEach-Object { ... }`). Brace-placement rules must not move
        /// it.
        const COMMAND_ELEMENT = 1 << 7;
    }
}

/// A lexical token. Borrows nothing: text is recovered by slicing the source
/// with `span`, keeping tokens `Copy` across FFI/adapter boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub flags: TokenFlags,
}

impl Token {
    #[must_use]
    pub const fn new(kind: TokenKind, span: Span, flags: TokenFlags) -> Self {
        Self { kind, span, flags }
    }

    #[must_use]
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        self.span.text(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_lookup_is_case_insensitive() {
        assert_eq!(Keyword::lookup("IF"), Some(Keyword::If));
        assert_eq!(Keyword::lookup("ElseIf"), Some(Keyword::ElseIf));
        assert_eq!(Keyword::lookup("dynamicParam"), Some(Keyword::DynamicParam));
        assert_eq!(Keyword::lookup("notakeyword"), None);
        assert_eq!(Keyword::lookup("ifé"), None);
    }
}
