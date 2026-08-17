//! Width-aware layout: lines longer than `line_width` that contain pipeline
//! operators are broken after each pipe, letting the indentation phase apply
//! the configured pipeline-continuation style.
//!
//! This goes beyond PSScriptAnalyzer (which never re-flows lines); it only
//! ever *adds* line breaks, so idempotence and PSSA-parity on already-short
//! lines are unaffected. `line_width = 0` disables it.

use crate::engine::{Engine, LineState};
use pwsh_parser::TokenKind;

pub(crate) fn apply(engine: &mut Engine<'_>) {
    let width = engine.opts.line_width as usize;
    if width == 0 {
        return;
    }
    let len = engine.len();
    let mut line_start_pos = 0usize;

    let mut pos = 0usize;
    while pos <= len {
        let at_break = pos == len || (pos > 0 && engine.gaps[pos].breaks_line());
        if at_break {
            let line_end_pos = pos; // exclusive
            if line_exceeds_width(engine, line_start_pos, line_end_pos, width) {
                break_after_pipes(engine, line_start_pos, line_end_pos);
            }
            line_start_pos = pos;
        }
        pos += 1;
    }
}

/// Approximate the rendered width of the line holding tokens
/// `[start, end)`. Uses decided gap widths; multi-line tokens and backtick
/// continuations end the measurement (their tails live on other lines, and
/// the indent phase may still change a continuation's indentation, so
/// counting across one is not stable between passes).
fn line_exceeds_width(engine: &Engine<'_>, start: usize, end: usize, width: usize) -> bool {
    let mut col = 0usize;
    for pos in start..end {
        if pos > start {
            let gap = &engine.gaps[pos];
            if gap.has_continuation {
                return false;
            }
            col += match &gap.line {
                LineState::Join { spaces } => *spaces as usize,
                LineState::AsIs => engine.parse.tokens[gap.trivia.clone()]
                    .iter()
                    .map(|t| t.text(engine.src).chars().count())
                    .sum(),
                LineState::Break { .. } => return false,
            };
        }
        let text = engine.text(pos);
        if text.contains('\n') {
            return false;
        }
        col += text.chars().count();
        if col > width {
            return true;
        }
    }
    false
}

/// Break after every pipe on the line (`cmd |` + newline), the layout the
/// pipeline-indentation styles are designed around.
fn break_after_pipes(engine: &mut Engine<'_>, start: usize, end: usize) {
    for pos in start..end {
        if engine.kind(pos) != TokenKind::Pipe {
            continue;
        }
        let after = pos + 1;
        if after >= engine.len() {
            continue;
        }
        let gap = &engine.gaps[after];
        if gap.breaks_line() || gap.has_comment || gap.has_continuation {
            continue;
        }
        // Never break inside a one-line delimiter pair: the brace phase has
        // already decided those stay one-line, and changing that here would
        // make a second pass decide differently (non-idempotence).
        if engine.enclosing[pos].is_some_and(|open| engine.pair_is_one_line(open)) {
            continue;
        }
        // Never break before a spaced operator: CheckOperator pulls a
        // line-leading operator back onto the previous line, so the break
        // would be undone on the next pass.
        if engine.opts.space_around_operator && super::whitespace::operator_in_set(engine, after) {
            continue;
        }
        engine.gaps[after].line = LineState::Break {
            cap_blanks: None,
            strip_ws: true,
        };
    }
}
