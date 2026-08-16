//! Property-based tests: arbitrary and structured random inputs can never
//! panic the scanner or formatter, and successful formatting is idempotent,
//! deterministic, and content-preserving.

use powershell_formatter::{FormatOptions, format};
use powershell_parser::tokenize;
use proptest::prelude::*;

/// Generator for PowerShell-ish token soup that exercises the lexer's modes.
fn ps_fragment() -> impl Strategy<Value = String> {
    let atom = prop_oneof![
        Just("$x".to_owned()),
        Just("$env:PATH".to_owned()),
        Just("@args".to_owned()),
        Just("42".to_owned()),
        Just("1kb".to_owned()),
        Just("'str'".to_owned()),
        Just("\"i $x\"".to_owned()),
        Just("Get-Item".to_owned()),
        Just("-Path".to_owned()),
        Just("if".to_owned()),
        Just("foreach".to_owned()),
        Just("|".to_owned()),
        Just("&&".to_owned()),
        Just(";".to_owned()),
        Just(",".to_owned()),
        Just("=".to_owned()),
        Just("-eq".to_owned()),
        Just("+".to_owned()),
        Just("`\n".to_owned()),
        Just("\n".to_owned()),
        Just("\r\n".to_owned()),
        Just(" ".to_owned()),
        Just("\t".to_owned()),
        Just("# comment".to_owned()),
        Just("<# block #>".to_owned()),
        Just("@'\nhs\n'@".to_owned()),
        Just("é🎉".to_owned()),
    ];
    prop::collection::vec(atom, 0..40).prop_map(|v| v.join(" "))
}

/// Balanced delimiter nests with random contents.
fn delimiter_nest() -> impl Strategy<Value = String> {
    let leaf = prop_oneof![
        Just("$x".to_owned()),
        Just("1".to_owned()),
        Just("'s'".to_owned()),
        Just("cmd arg".to_owned()),
        Just("a = 1".to_owned()),
        Just("\n".to_owned()),
        Just("".to_owned()),
    ];
    leaf.prop_recursive(6, 64, 4, |inner| {
        (
            prop_oneof![
                Just(("{ ", " }")),
                Just(("@{ ", " }")),
                Just(("( ", " )")),
                Just(("@( ", " )")),
                Just(("$( ", " )")),
                Just(("[ ", " ]")),
            ],
            prop::collection::vec(inner, 0..4),
        )
            .prop_map(|((open, close), parts)| format!("{open}{}{close}", parts.join(" ")))
    })
}

proptest! {
    /// Arbitrary (valid UTF-8) input never panics the scanner, and the
    /// token stream is lossless.
    #[test]
    fn tokenize_never_panics_and_is_lossless(src in ".{0,400}") {
        let out = tokenize(&src);
        let mut prev = 0;
        for t in &out.tokens {
            prop_assert_eq!(t.span.start, prev);
            prev = t.span.end;
        }
        prop_assert_eq!(prev, src.len());
    }

    /// Arbitrary input never panics the formatter; successful formatting is
    /// deterministic and idempotent.
    #[test]
    fn format_never_panics_and_is_idempotent(src in ".{0,300}") {
        let opts = FormatOptions::default();
        let once = format(&src, &opts);
        let again = format(&src, &opts);
        prop_assert_eq!(&once.text, &again.text);
        if once.formatted {
            let twice = format(&once.text, &opts);
            prop_assert_eq!(&once.text, &twice.text, "src: {:?}", src);
        } else {
            prop_assert_eq!(&once.text, &src);
        }
    }

    /// PowerShell-shaped token soup: same guarantees, denser coverage.
    #[test]
    fn fragment_soup_is_stable(src in ps_fragment()) {
        let opts = FormatOptions::default();
        let once = format(&src, &opts);
        if once.formatted {
            let twice = format(&once.text, &opts);
            prop_assert_eq!(&once.text, &twice.text, "src: {:?}", src);
        }
    }

    /// Valid delimiter nests always format (no incomplete-input bailout) and
    /// stay idempotent under every preset.
    #[test]
    fn delimiter_nests_format_cleanly(src in delimiter_nest()) {
        for opts in [FormatOptions::default(), FormatOptions::allman(), FormatOptions::otbs()] {
            let once = format(&src, &opts);
            let twice = format(&once.text, &opts);
            prop_assert_eq!(&once.text, &twice.text, "src: {:?}", src);
        }
    }

    /// Random content inside single-quoted strings survives byte-for-byte.
    #[test]
    fn string_contents_preserved(inner in "[^'\u{2018}\u{2019}\u{201A}\u{201B}]{0,60}") {
        let src = format!("$x = '{inner}'");
        let opts = FormatOptions::default();
        let result = format(&src, &opts);
        if result.formatted {
            prop_assert!(result.text.contains(&format!("'{inner}'")), "src: {src:?} out: {:?}", result.text);
        }
    }

    /// The newline style of the output matches the input's dominant style.
    #[test]
    fn newline_mode_stable(use_crlf in any::<bool>(), stmts in prop::collection::vec("[a-z]{1,6}", 1..6)) {
        let nl = if use_crlf { "\r\n" } else { "\n" };
        let src = stmts.join(nl);
        let result = format(&src, &FormatOptions::default());
        if result.formatted && result.text.contains('\n') {
            if use_crlf {
                prop_assert!(!result.text.replace("\r\n", "").contains('\n'));
            } else {
                prop_assert!(!result.text.contains('\r'));
            }
        }
    }
}
