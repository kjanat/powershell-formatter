//! wasm-bindgen bindings for the browser/Node.js package.
//!
//! This crate is a thin serialization boundary: JS values in, JS values out;
//! all formatting policy stays in `pwsh-formatter`.

use pwsh_formatter::{FormatOptions, FormatRange, JsonCatalog};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsDiagnostic {
    message: String,
    code: String,
    severity: String,
    line: u32,
    column: u32,
    start: u32,
    end: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsFormatResult {
    text: String,
    formatted: bool,
    diagnostics: Vec<JsDiagnostic>,
}

// All four fields are required and unknown keys are rejected: `FormatRange`
// coordinates are 1-based, so a defaulted-to-zero field from a missing or
// misspelled key would be an invalid range accepted silently.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JsRange {
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
}

fn convert(result: pwsh_formatter::FormatResult) -> JsFormatResult {
    JsFormatResult {
        text: result.text,
        formatted: result.formatted,
        diagnostics: result
            .diagnostics
            .into_iter()
            .map(|d| JsDiagnostic {
                message: d.message,
                code: d.code.as_str().to_owned(),
                severity: d.severity.as_str().to_owned(),
                line: d.position.line,
                column: d.position.column,
                start: u32::try_from(d.span.start).unwrap_or(u32::MAX),
                end: u32::try_from(d.span.end).unwrap_or(u32::MAX),
            })
            .collect(),
    }
}

fn parse_options(options: &JsValue) -> Result<FormatOptions, JsValue> {
    if options.is_undefined() || options.is_null() {
        return Ok(FormatOptions::default());
    }
    serde_wasm_bindgen::from_value(options.clone())
        .map_err(|e| JsValue::from_str(&format!("invalid options: {e}")))
}

fn parse_catalog(catalog: &JsValue) -> Result<Option<JsonCatalog>, JsValue> {
    if catalog.is_undefined() || catalog.is_null() {
        return Ok(None);
    }
    #[derive(Deserialize)]
    struct RawCatalog {
        commands: std::collections::HashMap<String, Vec<String>>,
    }
    let raw: RawCatalog = serde_wasm_bindgen::from_value(catalog.clone())
        .map_err(|e| JsValue::from_str(&format!("invalid catalog: {e}")))?;
    let mut cat = JsonCatalog::default();
    for (name, params) in raw.commands {
        let params: Vec<&str> = params.iter().map(String::as_str).collect();
        cat.insert(&name, &params);
    }
    Ok(Some(cat))
}

/// Format a complete PowerShell source string.
///
/// `options` is a camelCase `FormatOptions` object (all fields optional);
/// `catalog` is an optional `{ commands: { Name: [params...] } }` object for
/// command/parameter casing. Returns `{ text, formatted, diagnostics }`.
#[wasm_bindgen]
pub fn format(source: &str, options: JsValue, catalog: JsValue) -> Result<JsValue, JsValue> {
    let opts = parse_options(&options)?;
    let cat = parse_catalog(&catalog)?;
    let result = match &cat {
        Some(c) => pwsh_formatter::format_with_catalog(source, &opts, c),
        None => pwsh_formatter::format(source, &opts),
    };
    serde_wasm_bindgen::to_value(&convert(result)).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Format only the given 1-based line/column range (like
/// `Invoke-Formatter -Range`).
#[wasm_bindgen(js_name = formatRange)]
pub fn format_range(
    source: &str,
    range: JsValue,
    options: JsValue,
    catalog: JsValue,
) -> Result<JsValue, JsValue> {
    let opts = parse_options(&options)?;
    let cat = parse_catalog(&catalog)?;
    let range: JsRange = serde_wasm_bindgen::from_value(range)
        .map_err(|e| JsValue::from_str(&format!("invalid range: {e}")))?;
    let range = FormatRange {
        start_line: range.start_line,
        start_column: range.start_column,
        end_line: range.end_line,
        end_column: range.end_column,
    };
    let result = match &cat {
        Some(c) => pwsh_formatter::format_range_with_catalog(source, &opts, c, range),
        None => pwsh_formatter::format_range(source, &opts, range),
    };
    serde_wasm_bindgen::to_value(&convert(result)).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// The version of the underlying formatter.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// The complete default configuration as a plain object. The JS package's
/// TypeScript declarations are validated against these keys so they cannot
/// drift from the Rust model.
#[wasm_bindgen(js_name = defaultOptions)]
pub fn default_options() -> Result<JsValue, JsValue> {
    let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
    FormatOptions::default()
        .serialize(&serializer)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
