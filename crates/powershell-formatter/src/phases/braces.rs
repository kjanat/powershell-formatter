//! Brace placement (PSPlaceCloseBrace, then PSPlaceOpenBrace — PSSA order).
//!
//! Empirical ground rules (verified against PSScriptAnalyzer 1.25):
//! every brace-movement behavior applies only to *statement* braces
//! (keyword construct bodies, function/class bodies, switch clauses) and —
//! for the close-brace rule — to multi-line hashtable braces. Script blocks
//! used as expressions or command arguments are never moved.

use crate::engine::{Engine, LineState};
use crate::options::{BraceStyle, BranchKeywordPlacement};
use powershell_parser::{Keyword, TokenKind};

fn is_branch_keyword(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Keyword(Keyword::Else | Keyword::ElseIf | Keyword::Catch | Keyword::Finally)
    )
}

/// PSPlaceCloseBrace: `}` on its own line; optional blank-line removal;
/// branch keyword cuddling/uncuddling.
pub(crate) fn place_close_braces(engine: &mut Engine<'_>) {
    if !engine.opts.place_close_brace {
        return;
    }
    for pos in 0..engine.len() {
        if engine.kind(pos) != TokenKind::RCurly {
            continue;
        }
        let Some(open_pos) = engine.sig_match[pos] else {
            // PSSA stops scanning at the first unmatched close brace.
            break;
        };
        let open_kind = engine.kind(open_pos);
        let one_line = engine.pair_is_one_line(open_pos);

        // One-line hashtables are unconditionally exempt.
        if open_kind == TokenKind::AtCurly && one_line {
            continue;
        }
        let movable = open_kind == TokenKind::AtCurly || engine.is_statement_brace(open_pos);
        let ignored = !movable || (engine.opts.ignore_one_line_block && one_line);

        // Violation A: close brace on its own line.
        if !ignored && !engine.gaps[pos].breaks_line() && !engine.gaps[pos].has_continuation {
            engine.gaps[pos].line = LineState::Break {
                cap_blanks: None,
                strip_ws: false,
            };
        }

        // Violation B: no blank line before `}`.
        if engine.opts.no_empty_line_before_close_brace && engine.gaps[pos].breaks_line() {
            engine.gaps[pos].line = LineState::Break {
                cap_blanks: Some(0),
                strip_ws: false,
            };
        }

        // Branch keyword placement after `}`.
        if pos + 1 < engine.len() && is_branch_keyword(engine.kind(pos + 1)) && !ignored {
            match engine.opts.branch_keyword_placement {
                BranchKeywordPlacement::NextLine => {
                    if !engine.gaps[pos + 1].breaks_line() {
                        engine.gaps[pos + 1].line = LineState::Break {
                            cap_blanks: None,
                            strip_ws: true,
                        };
                    }
                }
                BranchKeywordPlacement::Cuddled => {
                    // `} else` with exactly one space. A comment between
                    // defeats detection (PSSA limitation, kept for parity).
                    if !engine.gaps[pos + 1].has_comment && !engine.gaps[pos + 1].has_continuation {
                        engine.gaps[pos + 1].line = LineState::Join { spaces: 1 };
                    }
                }
            }
        }
    }
}

/// PSPlaceOpenBrace: same-line vs next-line placement plus newline-after.
pub(crate) fn place_open_braces(engine: &mut Engine<'_>) {
    if !engine.opts.place_open_brace {
        return;
    }
    for pos in 0..engine.len() {
        if engine.kind(pos) != TokenKind::LCurly {
            continue;
        }
        if !engine.is_statement_brace(pos) {
            continue;
        }
        let one_line = engine.pair_is_one_line(pos);
        if engine.opts.ignore_one_line_block && one_line {
            continue;
        }

        match engine.opts.brace_style {
            BraceStyle::SameLine => {
                if pos > 0 && engine.gaps[pos].breaks_line() && !engine.gaps[pos].has_continuation {
                    if engine.gaps[pos].has_comment {
                        // PSSA moves a single trailing comment to just
                        // after the brace.
                        let comments: Vec<String> = engine.parse.tokens
                            [engine.gaps[pos].trivia.clone()]
                        .iter()
                        .filter(|t| t.kind.is_comment())
                        .map(|t| t.text(engine.src).to_owned())
                        .collect();
                        // Relocating is only safe when the gap after the
                        // brace ends the line — otherwise the line comment
                        // would swallow the code after it (the preservation
                        // check catches that, but as a silent format skip).
                        if comments.len() == 1
                            && (engine.opts.newline_after_open_brace
                                || engine.gaps[pos + 1].breaks_line())
                        {
                            engine.gaps[pos].line = LineState::Join { spaces: 1 };
                            engine.gaps[pos].has_comment = false;
                            let taken = comments.into_iter().next();
                            strip_gap_trivia(engine, pos);
                            engine.gaps[pos + 1].moved_comment = taken;
                        }
                        // Multiple comments: PSSA does not handle this
                        // either; leave the brace where it is.
                    } else {
                        engine.gaps[pos].line = LineState::Join { spaces: 1 };
                    }
                }
            }
            BraceStyle::NextLine => {
                if pos > 0 && !engine.gaps[pos].breaks_line() && !engine.gaps[pos].has_continuation
                {
                    engine.gaps[pos].line = LineState::Break {
                        cap_blanks: None,
                        strip_ws: true,
                    };
                }
            }
        }

        // Newline after `{`.
        if engine.opts.newline_after_open_brace
            && pos + 1 < engine.len()
            && !engine.gaps[pos + 1].breaks_line()
            && !engine.gaps[pos + 1].has_continuation
        {
            engine.gaps[pos + 1].line = LineState::Break {
                cap_blanks: None,
                strip_ws: true,
            };
        }
    }
}

/// Empty a gap's trivia range so a Join render does not re-emit relocated
/// comments.
fn strip_gap_trivia(engine: &mut Engine<'_>, pos: usize) {
    let r = engine.gaps[pos].trivia.clone();
    engine.gaps[pos].trivia = r.start..r.start;
    engine.gaps[pos].orig_newline = false;
}
