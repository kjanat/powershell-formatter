//! dprint adapter boundary.
//!
//! The dprint ABI will live here; formatting policy remains in `powershell-formatter`.

use powershell_formatter::{FormatOptions, FormatResult, format};

#[must_use]
pub fn format_text(source: &str) -> FormatResult {
    format(source, &FormatOptions::default())
}
