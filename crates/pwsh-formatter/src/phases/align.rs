//! PSAlignAssignmentStatement: align `=` inside hashtables and enums.

use crate::engine::{Engine, LineState};
use pwsh_parser::{Keyword, OperatorKind, TokenKind};

/// One alignment site: the token ending the left-hand side and (for valued
/// members) the `=` position.
struct Site {
    lhs_end: usize,
    eq: Option<usize>,
}

pub(crate) fn apply(engine: &mut Engine<'_>) {
    if !engine.opts.align_assignment {
        return;
    }
    let len = engine.len();
    // The column map is O(source); computing it per group would make
    // alignment quadratic on hashtable-heavy files. Instead share one map
    // across all groups per pass, and run a second pass so groups whose
    // columns were shifted by an enclosing group's adjustment settle.
    for _ in 0..2 {
        let cols = column_map(engine);
        let mut changed = false;
        for pos in 0..len {
            match engine.kind(pos) {
                TokenKind::AtCurly => {
                    if let Some(close) = engine.sig_match[pos] {
                        let sites = hashtable_sites(engine, pos, close);
                        changed |= align_group(engine, &cols, sites);
                    }
                }
                TokenKind::LCurly if is_enum_body(engine, pos) => {
                    if let Some(close) = engine.sig_match[pos] {
                        let sites = enum_sites(engine, pos, close);
                        changed |= align_group(engine, &cols, sites);
                    }
                }
                _ => {}
            }
        }
        if !changed {
            break;
        }
    }
}

/// `{` opening an `enum Name {` (possibly with a base type) body.
fn is_enum_body(engine: &Engine<'_>, open: usize) -> bool {
    let mut p = open;
    let mut steps = 0;
    while p > 0 && steps < 5 {
        p -= 1;
        steps += 1;
        if engine.kind(p) == TokenKind::Keyword(Keyword::Enum) {
            return true;
        }
        // Allowed between `enum` and `{`: name, base-type colon pieces.
        if !matches!(
            engine.kind(p),
            TokenKind::Identifier
                | TokenKind::Generic
                | TokenKind::Operator(OperatorKind::TernaryColon)
                | TokenKind::Operator(OperatorKind::Binary)
        ) {
            return false;
        }
    }
    false
}

fn hashtable_sites(engine: &Engine<'_>, open: usize, close: usize) -> Vec<Site> {
    let mut sites = Vec::new();
    let mut p = open + 1;
    while p < close {
        // Entry starts here; find the `=` belonging to this entry (at this
        // hashtable's nesting level) before the entry's value.
        if engine.enclosing[p] == Some(open) {
            let mut q = p;
            let mut eq = None;
            while q < close && engine.enclosing[q] == Some(open) {
                let k = engine.kind(q);
                if k == TokenKind::Operator(OperatorKind::Assignment) && engine.text(q) == "=" {
                    eq = Some(q);
                    break;
                }
                if k == TokenKind::Semicolon || (q > p && engine.gaps[q].breaks_line()) {
                    break;
                }
                q += 1;
            }
            if let Some(eq) = eq {
                let lhs_end = eq - 1;
                // Key and `=` must share a line; a comment or continuation
                // in between disqualifies the site from adjustment.
                let gap = &engine.gaps[eq];
                if !gap.breaks_line() && !gap.has_continuation && !gap.has_comment {
                    sites.push(Site {
                        lhs_end,
                        eq: Some(eq),
                    });
                }
                // Skip to the end of this entry (next `;` or line break at
                // this level).
                let mut r = eq + 1;
                while r < close {
                    if engine.enclosing[r] == Some(open)
                        && (engine.kind(r) == TokenKind::Semicolon || engine.gaps[r].breaks_line())
                    {
                        break;
                    }
                    r += 1;
                }
                p = r;
                continue;
            }
        }
        p += 1;
    }
    sites
}

fn enum_sites(engine: &Engine<'_>, open: usize, close: usize) -> Vec<Site> {
    let mut sites = Vec::new();
    let mut p = open + 1;
    while p < close {
        if engine.enclosing[p] == Some(open)
            && matches!(engine.kind(p), TokenKind::Identifier | TokenKind::Generic)
            && (engine.gaps[p].breaks_line() || p == open + 1 || {
                p > 0 && engine.kind(p - 1) == TokenKind::Semicolon
            })
        {
            let valued = p + 1 < close
                && engine.kind(p + 1) == TokenKind::Operator(OperatorKind::Assignment)
                && engine.text(p + 1) == "=";
            if valued {
                let gap = &engine.gaps[p + 1];
                if !gap.breaks_line() && !gap.has_continuation && !gap.has_comment {
                    sites.push(Site {
                        lhs_end: p,
                        eq: Some(p + 1),
                    });
                }
            } else {
                // Valueless member: widens the target, never adjusted.
                sites.push(Site {
                    lhs_end: p,
                    eq: None,
                });
            }
        }
        p += 1;
    }
    sites
}

/// Returns true when any gap decision changed.
fn align_group(engine: &mut Engine<'_>, cols: &Columns, sites: Vec<Site>) -> bool {
    if sites.is_empty() {
        return false;
    }
    // Discard lines holding more than one site.
    let mut by_line: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let lines: Vec<u32> = sites.iter().map(|s| cols.line[s.lhs_end]).collect();
    for &l in &lines {
        *by_line.entry(l).or_default() += 1;
    }
    let kept: Vec<&Site> = sites
        .iter()
        .zip(&lines)
        .filter(|(_, l)| by_line[l] == 1)
        .map(|(s, _)| s)
        .collect();
    if kept.is_empty() {
        return false;
    }
    let target = kept
        .iter()
        .map(|s| cols.end_col[s.lhs_end])
        .max()
        .unwrap_or(0)
        + 1;
    let updates: Vec<(usize, u16)> = kept
        .iter()
        .filter_map(|s| {
            let eq = s.eq?;
            let spaces = target.saturating_sub(cols.end_col[s.lhs_end]);
            Some((eq, u16::try_from(spaces).unwrap_or(u16::MAX)))
        })
        .collect();
    let mut changed = false;
    for (eq, spaces) in updates {
        let want = usize::from(spaces.max(1));
        if matches!(engine.gaps[eq].line, LineState::AsIs) && engine.gaps[eq].orig_newline {
            // Should not happen (sites require same-line), defensive.
            continue;
        }
        // PSSA corrects a site only when its current *character width*
        // differs from the target; a width-matching gap keeps its bytes
        // (a lone tab before `=` survives alignment). Compare widths and
        // leave matching gaps untouched instead of stamping spaces.
        let current = match engine.gaps[eq].exact_spaces {
            Some(e) => usize::from(e),
            None => match engine.gaps[eq].line {
                LineState::Join { spaces } => usize::from(spaces),
                _ => engine.parse.tokens[engine.gaps[eq].trivia.clone()]
                    .iter()
                    .map(|t| t.text(engine.src).chars().count())
                    .sum(),
            },
        };
        if current == want {
            continue;
        }
        engine.gaps[eq].exact_spaces = Some(spaces.max(1));
        changed = true;
    }
    changed
}

/// Simulated final columns (1-based, exclusive end) and line numbers per
/// significant token, honoring every layout decision made so far.
struct Columns {
    end_col: Vec<usize>,
    line: Vec<u32>,
}

fn column_map(engine: &Engine<'_>) -> Columns {
    let len = engine.len();
    let mut end_col = vec![0usize; len];
    let mut line_of = vec![0u32; len];
    let mut col = 1usize; // 1-based
    let mut line = 0u32;
    for pos in 0..len {
        let gap = &engine.gaps[pos];
        match &gap.line {
            LineState::Join { spaces } => {
                col += gap.exact_spaces.map_or(*spaces as usize, |e| e as usize);
                if gap.has_comment {
                    // Approximate: comments re-emitted inline.
                    col += engine.parse.tokens[gap.trivia.clone()]
                        .iter()
                        .filter(|t| t.kind.is_comment())
                        .map(|t| t.text(engine.src).chars().count() + 1)
                        .sum::<usize>();
                }
            }
            LineState::AsIs if !gap.orig_newline && !gap.has_continuation => {
                col += gap.exact_spaces.map_or_else(
                    || {
                        engine.parse.tokens[gap.trivia.clone()]
                            .iter()
                            .map(|t| t.text(engine.src).chars().count())
                            .sum::<usize>()
                    },
                    |e| e as usize,
                );
            }
            _ => {
                // Line break: new line starts at the decided indent (or the
                // original trailing whitespace width).
                line += 1;
                let indent_width = gap.indent.map_or_else(
                    || original_trailing_ws_width(engine, gap),
                    |lvl| {
                        if engine.opts.use_tabs {
                            lvl as usize
                        } else {
                            lvl as usize * engine.opts.indent_width as usize
                        }
                    },
                );
                col = 1 + indent_width;
            }
        }
        // Token itself.
        let text = engine.respell[pos]
            .as_deref()
            .unwrap_or_else(|| engine.text(pos));
        if let Some(last_nl) = text.rfind('\n') {
            line += text.matches('\n').count() as u32;
            col = 1 + text[last_nl + 1..].chars().count();
        } else {
            col += text.chars().count();
        }
        end_col[pos] = col;
        line_of[pos] = line;
    }
    Columns {
        end_col,
        line: line_of,
    }
}

fn original_trailing_ws_width(engine: &Engine<'_>, gap: &crate::engine::Gap) -> usize {
    // Width of whitespace after the last newline in the gap.
    let mut width = 0usize;
    for t in &engine.parse.tokens[gap.trivia.clone()] {
        match t.kind {
            TokenKind::Newline => width = 0,
            TokenKind::Whitespace => {
                let text = t.text(engine.src);
                match text.rfind('\n') {
                    Some(i) => width = text[i + 1..].chars().count(),
                    None => width += text.chars().count(),
                }
            }
            _ => width = 0,
        }
    }
    width
}
