//! PSUseConsistentWhitespace: spaces around braces, parens, operators,
//! pipes, and separators.

use crate::engine::{Engine, LineState};
use powershell_parser::{Keyword, OperatorKind, TokenFlags, TokenKind};

/// Width in characters of a same-line gap (0 when empty).
fn gap_width(engine: &Engine<'_>, pos: usize) -> usize {
    engine.parse.tokens[engine.gaps[pos].trivia.clone()]
        .iter()
        .map(|t| t.text(engine.src).chars().count())
        .sum()
}

/// A same-line gap eligible for space normalization: not a line break, no
/// backtick continuation, no comment.
fn plain_gap(engine: &Engine<'_>, pos: usize) -> bool {
    let g = &engine.gaps[pos];
    !g.breaks_line() && !g.has_continuation && !g.has_comment
}

fn set_single_space(engine: &mut Engine<'_>, pos: usize) {
    if gap_width(engine, pos) != 1 || !matches!(engine.gaps[pos].line, LineState::AsIs) {
        engine.gaps[pos].line = LineState::Join { spaces: 1 };
    }
}

/// The `{` at `pos` (or the `}`'s opener) belongs to a braced member access
/// (`$x.{name}`) — excluded from inner-brace checks.
fn in_braced_member_access(engine: &Engine<'_>, open_pos: usize) -> bool {
    open_pos > 0 && engine.kind(open_pos - 1) == TokenKind::Operator(OperatorKind::MemberAccess)
}

/// Previous token carries PowerShell's MemberName-ish nature: an identifier
/// reached through member access (`(1..5).foreach{ }`).
fn prev_is_member_name(engine: &Engine<'_>, pos: usize) -> bool {
    pos >= 2
        && engine.kind(pos - 1) == TokenKind::Identifier
        && engine.kind(pos - 2) == TokenKind::Operator(OperatorKind::MemberAccess)
}

/// Whether the operator token participates in CheckOperator.
///
/// Verified empirically against PSScriptAnalyzer 1.25 `Invoke-Formatter`:
/// assignment operators, `+ - * / %`, every binary dash-word operator
/// (`-eq`, `-and`, `-band`, `-shl`, `-f`, `-join`, ... including unary
/// `-join`/`-split`), `&&` and `||` are all normalized. Not in the set:
/// `..` (never touched), `-not`/`-bnot`, `!`, `++`/`--`, member access,
/// ternary parts, and redirections.
pub(crate) fn operator_in_set(engine: &Engine<'_>, pos: usize) -> bool {
    match engine.kind(pos) {
        TokenKind::Operator(OperatorKind::Assignment) => true,
        TokenKind::AndAnd | TokenKind::OrOr => true,
        TokenKind::Operator(OperatorKind::ComparisonWord) => true,
        TokenKind::Operator(OperatorKind::Binary) => {
            matches!(engine.text(pos), "+" | "-" | "*" | "/" | "%")
        }
        TokenKind::Operator(OperatorKind::Unary) => {
            // `-`/`+` used unary still count (PowerShell's Plus/Minus token
            // kinds carry binary precedence); `++`/`--` do not.
            matches!(engine.text(pos), "+" | "-")
        }
        _ => false,
    }
}

pub(crate) fn apply(engine: &mut Engine<'_>) {
    let opts = engine.opts.clone();
    let len = engine.len();

    for pos in 0..len {
        let kind = engine.kind(pos);

        // ── CheckOpenBrace: one space before `{` ─────────────────────
        if opts.space_before_open_brace
            && kind == TokenKind::LCurly
            && pos > 0
            && plain_gap(engine, pos)
            && engine.kind(pos - 1) != TokenKind::LCurly
            && engine.kind(pos - 1) != TokenKind::LParen
            && engine.kind(pos - 1) != TokenKind::Operator(OperatorKind::MemberAccess)
            && !prev_is_member_name(engine, pos)
        {
            set_single_space(engine, pos);
        }

        // ── CheckInnerBrace ──────────────────────────────────────────
        if opts.space_inside_brace {
            // After `{` (LCurly only, and only when `{` is not at line
            // start).
            if kind == TokenKind::LCurly
                && pos + 1 < len
                && !(pos > 0 && engine.gaps[pos].breaks_line())
                && engine.kind(pos + 1) != TokenKind::RCurly
                && plain_gap(engine, pos + 1)
                && !in_braced_member_access(engine, pos)
            {
                set_single_space(engine, pos + 1);
            }
            // Before `}`.
            if kind == TokenKind::RCurly
                && pos > 0
                && plain_gap(engine, pos)
                && !matches!(engine.kind(pos - 1), TokenKind::LCurly | TokenKind::AtCurly)
                && !engine.sig_match[pos].is_some_and(|open| in_braced_member_access(engine, open))
            {
                set_single_space(engine, pos);
            }
        }

        // ── CheckOpenParen: keyword ( ────────────────────────────────
        if opts.space_after_keyword
            && kind == TokenKind::LParen
            && pos > 0
            && plain_gap(engine, pos)
            && matches!(
                engine.kind(pos - 1),
                TokenKind::Keyword(
                    Keyword::If
                        | Keyword::ElseIf
                        | Keyword::Switch
                        | Keyword::For
                        | Keyword::Foreach
                        | Keyword::While
                )
            )
        {
            set_single_space(engine, pos);
        }

        // ── CheckOperator ────────────────────────────────────────────
        if opts.space_around_operator && operator_in_set(engine, pos) && pos > 0 && pos + 1 < len {
            // Unary exception: `(-$x)`.
            let unary_exception = matches!(engine.text(pos), "+" | "-")
                && engine.kind(pos - 1) == TokenKind::LParen
                && engine.kind(pos + 1) == TokenKind::Variable;
            // `=` inside a multi-line hashtable is the alignment rule's
            // territory.
            let hashtable_exception = opts.ignore_assignment_in_hashtable
                && engine.kind(pos) == TokenKind::Operator(OperatorKind::Assignment)
                && engine.text(pos) == "="
                && engine.enclosing[pos].is_some_and(|open| {
                    engine.kind(open) == TokenKind::AtCurly && !engine.pair_is_one_line(open)
                });
            if !unary_exception && !hashtable_exception {
                // Before: a line-leading operator is pulled onto the
                // previous line (PSSA behavior) — but never across
                // comments or continuations.
                if engine.gaps[pos].breaks_line() {
                    if !engine.gaps[pos].has_comment && !engine.gaps[pos].has_continuation {
                        engine.gaps[pos].line = LineState::Join { spaces: 1 };
                    }
                } else if plain_gap(engine, pos) {
                    set_single_space(engine, pos);
                }
                // After: operator at line end is fine.
                if plain_gap(engine, pos + 1) {
                    set_single_space(engine, pos + 1);
                }
            }
        }

        // ── CheckPipe / redundant pipe whitespace ────────────────────
        if kind == TokenKind::Pipe && (opts.space_around_pipe || opts.collapse_space_around_pipe) {
            // Before. A `--%` verbatim argument owns the text up to the
            // pipe; PSSA re-inserts a space here on every run (it is not
            // idempotent) — we intentionally leave it alone.
            if pos > 0
                && plain_gap(engine, pos)
                && engine.kind(pos - 1) != TokenKind::Pipe
                && engine.kind(pos - 1) != TokenKind::RawArgument
            {
                let w = gap_width(engine, pos);
                if (w == 0 && opts.space_around_pipe) || (w > 1 && opts.collapse_space_around_pipe)
                {
                    engine.gaps[pos].line = LineState::Join { spaces: 1 };
                }
            }
            // After.
            if pos + 1 < len
                && !engine.gaps[pos].breaks_line()
                && plain_gap(engine, pos + 1)
                && engine.kind(pos + 1) != TokenKind::Pipe
            {
                let w = gap_width(engine, pos + 1);
                if (w == 0 && opts.space_around_pipe) || (w > 1 && opts.collapse_space_around_pipe)
                {
                    engine.gaps[pos + 1].line = LineState::Join { spaces: 1 };
                }
            }
        }

        // ── CheckSeparator: space after `,` / `;` ────────────────────
        if opts.space_after_separator
            && matches!(kind, TokenKind::Comma | TokenKind::Semicolon)
            && pos + 1 < len
            && plain_gap(engine, pos + 1)
        {
            set_single_space(engine, pos + 1);
        }

        // ── CheckParameter: collapse runs between command elements ───
        if opts.collapse_space_between_parameters
            && pos + 1 < len
            && plain_gap(engine, pos + 1)
            && gap_width(engine, pos + 1) > 1
            && is_command_element(engine, pos)
            && is_command_element(engine, pos + 1)
        {
            engine.gaps[pos + 1].line = LineState::Join { spaces: 1 };
        }
    }
}

/// Very shallow "command element" notion for CheckParameter: the command
/// name or an argument-position token.
fn is_command_element(engine: &Engine<'_>, pos: usize) -> bool {
    let t = engine.token(pos);
    t.flags
        .intersects(TokenFlags::COMMAND_NAME | TokenFlags::IN_COMMAND_ARGS)
        || t.kind == TokenKind::Parameter
        || (t.kind == TokenKind::RCurly
            && engine.sig_match[pos]
                .is_some_and(|o| engine.token(o).flags.contains(TokenFlags::COMMAND_ELEMENT)))
}
