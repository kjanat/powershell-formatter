//! Fuzz the complete formatter: no panics, valid UTF-8 out, deterministic,
//! idempotent, protected content preserved.

#![no_main]

use libfuzzer_sys::fuzz_target;
use powershell_formatter::{FormatOptions, format};
use powershell_parser::tokenize;

fuzz_target!(|data: &[u8]| {
    let Ok(src) = core::str::from_utf8(data) else {
        return;
    };
    let opts = FormatOptions::default();
    let once = format(src, &opts);
    let again = format(src, &opts);
    assert_eq!(once.text, again.text, "formatting must be deterministic");

    if !once.formatted {
        assert_eq!(once.text, src, "skipped formatting must preserve input");
        return;
    }

    // Idempotence.
    let twice = format(&once.text, &opts);
    assert_eq!(once.text, twice.text, "formatting must be idempotent");

    // Protected content preservation: string and comment token texts
    // survive byte-for-byte, in order.
    let before: Vec<String> = tokenize(src)
        .tokens
        .iter()
        .filter(|t| t.kind.is_string() || t.kind.is_comment())
        .map(|t| t.text(src).to_owned())
        .collect();
    let after: Vec<String> = tokenize(&once.text)
        .tokens
        .iter()
        .filter(|t| t.kind.is_string() || t.kind.is_comment())
        .map(|t| t.text(&once.text).to_owned())
        .collect();
    assert_eq!(before, after, "protected content must be preserved");
});
