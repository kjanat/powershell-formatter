//! Fuzz the scanner: arbitrary bytes must never panic, and valid UTF-8 must
//! tokenize losslessly.

#![no_main]

use libfuzzer_sys::fuzz_target;
use powershell_parser::tokenize;

fuzz_target!(|data: &[u8]| {
    let Ok(src) = core::str::from_utf8(data) else {
        return;
    };
    let out = tokenize(src);
    // Losslessness: token spans tile the input exactly.
    let mut prev = 0;
    for t in &out.tokens {
        assert_eq!(t.span.start, prev, "gap in token stream");
        assert!(t.span.end > t.span.start || t.span.end == prev);
        assert!(src.is_char_boundary(t.span.start));
        assert!(src.is_char_boundary(t.span.end));
        prev = t.span.end;
    }
    assert_eq!(prev, src.len(), "token stream misses input tail");
});
