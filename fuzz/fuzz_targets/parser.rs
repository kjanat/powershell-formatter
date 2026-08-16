//! Fuzz the structural parser: no panics, symmetric delimiter matches,
//! in-bounds statement indices.

#![no_main]

use libfuzzer_sys::fuzz_target;
use powershell_parser::parse;

fuzz_target!(|data: &[u8]| {
    let Ok(src) = core::str::from_utf8(data) else {
        return;
    };
    let result = parse(src);
    for (i, m) in result.matches.iter().enumerate() {
        if let Some(j) = m {
            let j = *j as usize;
            assert!(j < result.tokens.len(), "match index out of bounds");
            assert_eq!(
                result.matches[j],
                Some(i as u32),
                "delimiter matches must be symmetric"
            );
        }
    }
});
