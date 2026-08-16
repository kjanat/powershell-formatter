//! Host-independent PowerShell formatting engine.

use powershell_parser::scan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BraceStyle {
    #[default]
    SameLine,
    NextLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatOptions {
    pub brace_style: BraceStyle,
    pub indent_width: u8,
    pub use_tabs: bool,
    pub line_width: u16,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            brace_style: BraceStyle::SameLine,
            indent_width: 4,
            use_tabs: false,
            line_width: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatResult {
    pub text: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
}

/// Format a complete PowerShell source string.
///
/// This scaffold intentionally preserves the source unchanged. Calling the parser here fixes the
/// package boundary now so the scanner/layout implementation can evolve behind this API.
#[must_use]
pub fn format(source: &str, _options: &FormatOptions) -> FormatResult {
    let _tokens = scan(source);

    FormatResult {
        text: source.to_owned(),
        diagnostics: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_is_lossless() {
        let source = "function Test { 'hé' }\n";
        let result = format(source, &FormatOptions::default());

        assert_eq!(result.text, source);
        assert!(result.diagnostics.is_empty());
    }
}
