//! Formatting configuration.
//!
//! The option surface mirrors the useful configuration of PSScriptAnalyzer's
//! formatting rules (see `docs/configuration.md`) while using strong enums
//! internally. Serde support (on by default) drives the CLI config file, the
//! dprint plugin schema, and the JS API.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Where opening braces of statement blocks go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum BraceStyle {
    /// `if ($x) {` — brace on the same line (K&R / OTBS / Stroustrup).
    #[default]
    SameLine,
    /// Brace on its own line (Allman).
    NextLine,
}

/// Whether `else`/`elseif`/`catch`/`finally` cuddle onto the closing brace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum BranchKeywordPlacement {
    /// `}` then `else {` on the next line (Stroustrup — PSSA default).
    #[default]
    NextLine,
    /// `} else {` (OTBS).
    Cuddled,
}

/// How pipeline continuation lines are indented, mirroring PSSA's
/// `PipelineIndentation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum PipelineIndentation {
    /// Indent once at the first line-ending pipe of a pipeline.
    #[default]
    IncreaseIndentationForFirstPipeline,
    /// Indent one more level after every line-ending pipe.
    IncreaseIndentationAfterEveryPipeline,
    /// Continuation lines stay at the pipeline's own level.
    NoIndentation,
    /// Leave lines after a trailing pipe exactly as written.
    None,
}

/// Line-ending handling for output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum EndOfLine {
    /// Keep the input's detected dominant newline style (PSSA behavior).
    #[default]
    Auto,
    /// Force `\n`.
    Lf,
    /// Force `\r\n`.
    Crlf,
}

/// The complete formatting configuration.
///
/// Defaults reproduce PSScriptAnalyzer's `CodeFormatting` preset.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase", default))]
pub struct FormatOptions {
    /// Spaces per indentation level (ignored when `use_tabs`).
    pub indent_width: u8,
    /// Indent with tabs instead of spaces.
    pub use_tabs: bool,
    /// Target maximum line width. Used by width-aware layout; `0` disables
    /// width-based decisions.
    pub line_width: u16,
    /// Run the opening-brace placement rule at all (`PSPlaceOpenBrace.Enable`).
    pub place_open_brace: bool,
    /// Run the closing-brace placement rule at all
    /// (`PSPlaceCloseBrace.Enable`).
    pub place_close_brace: bool,
    /// Opening-brace placement (`PSPlaceOpenBrace.OnSameLine`).
    pub brace_style: BraceStyle,
    /// Require a newline after every opening brace
    /// (`PSPlaceOpenBrace.NewLineAfter`).
    pub newline_after_open_brace: bool,
    /// Leave one-line `{ ... }` blocks alone
    /// (`PSPlaceOpenBrace/PSPlaceCloseBrace.IgnoreOneLineBlock`).
    pub ignore_one_line_block: bool,
    /// Branch keyword placement after `}`
    /// (`PSPlaceCloseBrace.NewLineAfter`, inverted sense).
    pub branch_keyword_placement: BranchKeywordPlacement,
    /// Remove blank lines directly before `}`
    /// (`PSPlaceCloseBrace.NoEmptyLineBefore`).
    pub no_empty_line_before_close_brace: bool,

    /// Enforce a space before opening braces
    /// (`PSUseConsistentWhitespace.CheckOpenBrace`).
    pub space_before_open_brace: bool,
    /// Enforce single spaces just inside one-line braces
    /// (`PSUseConsistentWhitespace.CheckInnerBrace`).
    pub space_inside_brace: bool,
    /// Enforce a space between keywords and `(`
    /// (`PSUseConsistentWhitespace.CheckOpenParen`).
    pub space_after_keyword: bool,
    /// Enforce single spaces around binary/assignment operators
    /// (`PSUseConsistentWhitespace.CheckOperator`).
    pub space_around_operator: bool,
    /// Enforce a space after `,`/`;`
    /// (`PSUseConsistentWhitespace.CheckSeparator`).
    pub space_after_separator: bool,
    /// Add missing spaces around `|` (`PSUseConsistentWhitespace.CheckPipe`).
    pub space_around_pipe: bool,
    /// Also collapse redundant spaces around `|`
    /// (`PSUseConsistentWhitespace.CheckPipeForRedundantWhitespace`).
    pub collapse_space_around_pipe: bool,
    /// Collapse runs of spaces between command elements
    /// (`PSUseConsistentWhitespace.CheckParameter`).
    pub collapse_space_between_parameters: bool,
    /// Skip `=` inside multi-line hashtables when checking operator spacing
    /// (`PSUseConsistentWhitespace.IgnoreAssignmentOperatorInsideHashTable`).
    pub ignore_assignment_in_hashtable: bool,

    /// Reindent lines (`PSUseConsistentIndentation`).
    pub indentation: bool,
    /// Pipeline continuation indentation style.
    pub pipeline_indentation: PipelineIndentation,

    /// Align `=` in multi-member hashtables and enums
    /// (`PSAlignAssignmentStatement`).
    pub align_assignment: bool,

    /// Lowercase keywords (`PSUseCorrectCasing.CheckKeyword`).
    pub keyword_casing: bool,
    /// Lowercase operators (`PSUseCorrectCasing.CheckOperator`).
    pub operator_casing: bool,
    /// Correct command/parameter casing via the injected
    /// [`crate::CommandCatalog`] (`PSUseCorrectCasing.CheckCommands`).
    pub command_casing: bool,

    /// Output newline handling.
    pub end_of_line: EndOfLine,
    /// Ensure the output ends with exactly one final newline. `None`
    /// preserves the input's final-newline state (PSSA behavior).
    pub final_newline: Option<bool>,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_width: 4,
            use_tabs: false,
            line_width: 120,
            place_open_brace: true,
            place_close_brace: true,
            brace_style: BraceStyle::SameLine,
            newline_after_open_brace: true,
            ignore_one_line_block: true,
            branch_keyword_placement: BranchKeywordPlacement::NextLine,
            no_empty_line_before_close_brace: false,
            space_before_open_brace: true,
            space_inside_brace: true,
            space_after_keyword: true,
            space_around_operator: true,
            space_after_separator: true,
            space_around_pipe: true,
            collapse_space_around_pipe: false,
            collapse_space_between_parameters: false,
            ignore_assignment_in_hashtable: true,
            indentation: true,
            pipeline_indentation: PipelineIndentation::default(),
            align_assignment: true,
            keyword_casing: true,
            operator_casing: true,
            command_casing: true,
            end_of_line: EndOfLine::default(),
            final_newline: None,
        }
    }
}

impl FormatOptions {
    /// PSScriptAnalyzer `CodeFormatting` preset (the default).
    #[must_use]
    pub fn pssa_default() -> Self {
        Self::default()
    }

    /// PSScriptAnalyzer `CodeFormattingOTBS` preset: cuddled `} else {`.
    #[must_use]
    pub fn otbs() -> Self {
        Self {
            branch_keyword_placement: BranchKeywordPlacement::Cuddled,
            ..Self::default()
        }
    }

    /// PSScriptAnalyzer `CodeFormattingStroustrup` preset (same values as
    /// the default preset).
    #[must_use]
    pub fn stroustrup() -> Self {
        Self::default()
    }

    /// PSScriptAnalyzer `CodeFormattingAllman` preset: braces on their own
    /// lines.
    #[must_use]
    pub fn allman() -> Self {
        Self {
            brace_style: BraceStyle::NextLine,
            ..Self::default()
        }
    }

    /// One indentation level as text.
    #[must_use]
    pub fn indent_unit(&self) -> String {
        if self.use_tabs {
            "\t".to_owned()
        } else {
            " ".repeat(self.indent_width as usize)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_pssa_codeformatting() {
        let o = FormatOptions::default();
        assert_eq!(o.indent_width, 4);
        assert!(!o.use_tabs);
        assert_eq!(o.brace_style, BraceStyle::SameLine);
        assert!(o.newline_after_open_brace);
        assert!(o.ignore_one_line_block);
        assert_eq!(o.branch_keyword_placement, BranchKeywordPlacement::NextLine);
        assert!(o.ignore_assignment_in_hashtable);
        assert!(!o.collapse_space_around_pipe);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn options_round_trip_serde() {
        let o = FormatOptions::otbs();
        let json = serde_json::to_string(&o).unwrap();
        let back: FormatOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(o, back);
        // Unknown fields are rejected? No: default serde behavior ignores
        // them; partial configs must work.
        let partial: FormatOptions =
            serde_json::from_str(r#"{"indentWidth": 2, "braceStyle": "nextLine"}"#).unwrap();
        assert_eq!(partial.indent_width, 2);
        assert_eq!(partial.brace_style, BraceStyle::NextLine);
        assert!(partial.newline_after_open_brace);
    }
}
