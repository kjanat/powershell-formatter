//! PSUseConsistentIndentation: reindent every line based on delimiter
//! nesting, pipeline style, and backtick continuations.

use crate::engine::{Engine, LineState};
use crate::options::PipelineIndentation;
use powershell_parser::{OperatorKind, TokenKind};

/// A pipeline (two or more elements joined by `|`) at one nesting level.
struct Pipeline {
    start: usize,
    end: usize,
    increments: u32,
}

fn is_tracked_opener(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::LCurly
            | TokenKind::AtCurly
            | TokenKind::DollarParen
            | TokenKind::AtParen
            | TokenKind::LParen
    )
}

fn is_tracked_closer(kind: TokenKind) -> bool {
    matches!(kind, TokenKind::RCurly | TokenKind::RParen)
}

pub(crate) fn apply(engine: &mut Engine<'_>) {
    if !engine.opts.indentation {
        return;
    }
    let style = engine.opts.pipeline_indentation;
    let len = engine.len();

    // Final-layout line number of each significant token.
    let mut line_of = vec![0u32; len];
    let mut line = 0u32;
    for (pos, slot) in line_of.iter_mut().enumerate() {
        let g = &engine.gaps[pos];
        if g.breaks_line() || g.has_continuation {
            line += 1;
        }
        *slot = line;
    }

    let mut pipelines = find_pipelines(engine);
    // Sort by end so inner pipelines are restored before outer ones.
    pipelines.sort_by_key(|p| (p.end, p.end - p.start));
    // O(1) end-position lookup: scanning every pipeline per token would be
    // quadratic on pipeline-heavy files.
    let mut ends_at: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, p) in pipelines.iter().enumerate() {
        ends_at.entry(p.end).or_default().push(i);
    }

    let mut level: u32 = 0;
    // For each tracked opener: whether it skipped its increment.
    let mut skip_stack: Vec<bool> = Vec::new();

    for pos in 0..len {
        let kind = engine.kind(pos);
        let line_start = pos == 0 || engine.gaps[pos].breaks_line();
        let continuation_start = engine.gaps[pos].has_continuation;

        if is_tracked_opener(kind) {
            if line_start || continuation_start {
                assign(engine, pos, level);
            }
            // PSSA 1.25: every opener indents, except a `(` that starts a
            // line and has content after it on the same line.
            let skipped = kind == TokenKind::LParen
                && line_start
                && pos + 1 < len
                && !engine.gaps[pos + 1].breaks_line();
            if !skipped {
                level += 1;
            }
            skip_stack.push(skipped);
        } else if is_tracked_closer(kind) {
            let skipped = skip_stack.pop().unwrap_or(true);
            if !skipped {
                level = level.saturating_sub(1);
            }
            if line_start || continuation_start {
                assign(engine, pos, level);
            }
        } else if kind == TokenKind::Pipe {
            let line_ending = pos + 1 < len && engine.gaps[pos + 1].breaks_line();
            if style == PipelineIndentation::None {
                // Pipes never affect the level; but a leading pipe still
                // starts a checked line.
                if line_start {
                    assign(engine, pos, level);
                    normalize_after_leading_pipe(engine, pos);
                }
            } else {
                if line_start {
                    assign(engine, pos, level);
                    normalize_after_leading_pipe(engine, pos);
                }
                if line_ending {
                    let increment = match style {
                        PipelineIndentation::IncreaseIndentationAfterEveryPipeline => true,
                        PipelineIndentation::IncreaseIndentationForFirstPipeline => {
                            // Only when the element before this pipe ends on
                            // the pipeline's first line.
                            innermost_pipeline(&pipelines, pos).is_some_and(|pi| {
                                pos > 0 && line_of[pos - 1] == line_of[pipelines[pi].start]
                            })
                        }
                        _ => false,
                    };
                    if increment {
                        level += 1;
                        // PSSA 1.25 restores pipeline indentation only at
                        // the end of the *outermost* pipeline: increments
                        // from pipelines nested inside another pipeline's
                        // script blocks accumulate outward.
                        if let Some(pi) = outermost_pipeline(&pipelines, pos) {
                            pipelines[pi].increments += 1;
                        }
                    }
                }
            }
        } else {
            // Ordinary token (line starter or not).
            if line_start || continuation_start {
                let mut temp = level;
                if continuation_start {
                    temp += 1;
                }
                let after_trailing_pipe =
                    line_start && pos > 0 && engine.kind(pos - 1) == TokenKind::Pipe;
                if style == PipelineIndentation::None && after_trailing_pipe {
                    // Preserve whatever indentation the author used.
                } else {
                    assign(engine, pos, temp);
                }
            }
        }

        // Restore levels at pipeline ends.
        if style != PipelineIndentation::None
            && let Some(indices) = ends_at.get(&pos)
        {
            for &i in indices {
                level = level.saturating_sub(pipelines[i].increments);
                pipelines[i].increments = 0;
            }
        }
    }

    // Trailing trivia (comments at EOF) sit at level 0.
    if engine.gaps[len].breaks_line() {
        engine.gaps[len].indent = Some(0);
    }
}

/// Record the indentation level for the line(s) begun in `gaps[pos]`.
fn assign(engine: &mut Engine<'_>, pos: usize, level: u32) {
    engine.gaps[pos].indent = Some(level);
}

/// A line-leading `| element` keeps exactly one space after the pipe
/// (PSSA re-emits `"| "` when rewriting such lines).
fn normalize_after_leading_pipe(engine: &mut Engine<'_>, pipe_pos: usize) {
    if pipe_pos + 1 < engine.len()
        && !engine.gaps[pipe_pos + 1].breaks_line()
        && !engine.gaps[pipe_pos + 1].has_comment
        && !engine.gaps[pipe_pos + 1].has_continuation
    {
        engine.gaps[pipe_pos + 1].line = LineState::Join { spaces: 1 };
    }
}

fn innermost_pipeline(pipelines: &[Pipeline], pos: usize) -> Option<usize> {
    pipelines
        .iter()
        .enumerate()
        .filter(|(_, p)| p.start <= pos && pos <= p.end)
        .min_by_key(|(_, p)| p.end - p.start)
        .map(|(i, _)| i)
}

/// The largest pipeline containing `pos` (the one whose end restores the
/// indentation).
fn outermost_pipeline(pipelines: &[Pipeline], pos: usize) -> Option<usize> {
    pipelines
        .iter()
        .enumerate()
        .filter(|(_, p)| p.start <= pos && pos <= p.end)
        .max_by_key(|(_, p)| p.end - p.start)
        .map(|(i, _)| i)
}

/// Detect pipelines (≥ 2 elements) per nesting frame.
fn find_pipelines(engine: &Engine<'_>) -> Vec<Pipeline> {
    struct Frame {
        pipeline_start: Option<usize>,
        pipes: u32,
    }
    let mut out = Vec::new();
    let mut stack = vec![Frame {
        pipeline_start: None,
        pipes: 0,
    }];

    let end_pipeline = |frame: &mut Frame, end: usize, out: &mut Vec<Pipeline>| {
        if let Some(start) = frame.pipeline_start.take() {
            if frame.pipes > 0 && end >= start {
                out.push(Pipeline {
                    start,
                    end,
                    increments: 0,
                });
            }
        }
        frame.pipes = 0;
    };

    for pos in 0..engine.len() {
        let kind = engine.kind(pos);

        // Statement-ending newline?
        if pos > 0 && engine.gaps[pos].breaks_line() {
            let prev = engine.kind(pos - 1);
            let continues = matches!(
                prev,
                TokenKind::Pipe
                    | TokenKind::AndAnd
                    | TokenKind::OrOr
                    | TokenKind::Comma
                    | TokenKind::Operator(_)
            ) || prev.is_open_delimiter()
                || kind == TokenKind::Pipe;
            if !continues && !engine.gaps[pos].has_continuation {
                let frame = stack.last_mut().expect("frame");
                end_pipeline(frame, pos - 1, &mut out);
            }
        }

        if kind.is_open_delimiter() {
            stack.push(Frame {
                pipeline_start: None,
                pipes: 0,
            });
            continue;
        }
        if kind.is_close_delimiter() {
            if stack.len() > 1 {
                let mut frame = stack.pop().expect("frame");
                if pos > 0 {
                    end_pipeline(&mut frame, pos - 1, &mut out);
                }
            }
            continue;
        }
        let frame = stack.last_mut().expect("frame");
        match kind {
            TokenKind::Semicolon | TokenKind::AndAnd | TokenKind::OrOr => {
                if pos > 0 {
                    end_pipeline(frame, pos - 1, &mut out);
                }
            }
            TokenKind::Operator(OperatorKind::Assignment) => {
                // The RHS is its own pipeline.
                frame.pipeline_start = None;
                frame.pipes = 0;
            }
            TokenKind::Pipe => {
                frame.pipes += 1;
            }
            _ => {
                if frame.pipeline_start.is_none() {
                    frame.pipeline_start = Some(pos);
                }
            }
        }
    }
    let last = engine.len().saturating_sub(1);
    for mut frame in stack {
        end_pipeline(&mut frame, last, &mut out);
    }
    out
}
