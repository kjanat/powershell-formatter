//! Corpus invariants: every fixture in `tests/corpus/files`,
//! `tests/powershell-oracle/inputs`, and `tests/pssa-parity/inputs` must
//! survive scanning and formatting with strong guarantees.

use powershell_formatter::{FormatOptions, format};
use powershell_parser::{TokenKind, tokenize};
use std::path::PathBuf;

fn corpus_files() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests");
    let mut out = Vec::new();
    for dir in [
        "corpus/files",
        "powershell-oracle/inputs",
        "pssa-parity/inputs",
    ] {
        let dir = root.join(dir);
        for entry in
            std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        {
            let path = entry.expect("dir entry").path();
            if path
                .extension()
                .is_some_and(|e| e == "ps1" || e == "psm1" || e == "psd1")
            {
                out.push(path);
            }
        }
    }
    assert!(out.len() >= 15, "corpus unexpectedly small: {}", out.len());
    out.sort();
    out
}

/// Scanner losslessness: concatenated token texts reproduce the source.
#[test]
fn corpus_scan_is_lossless() {
    for path in corpus_files() {
        let src = std::fs::read_to_string(&path).expect("read");
        let out = tokenize(&src);
        let mut prev = 0;
        for t in &out.tokens {
            assert_eq!(t.span.start, prev, "gap in {}", path.display());
            prev = t.span.end;
        }
        assert_eq!(prev, src.len(), "missing tail in {}", path.display());
    }
}

/// Formatting terminates, is deterministic, and is idempotent.
#[test]
fn corpus_format_deterministic_and_idempotent() {
    let opts = FormatOptions::default();
    for path in corpus_files() {
        let src = std::fs::read_to_string(&path).expect("read");
        let once = format(&src, &opts);
        let once_again = format(&src, &opts);
        assert_eq!(
            once.text,
            once_again.text,
            "nondeterministic on {}",
            path.display()
        );
        let twice = format(&once.text, &opts);
        assert_eq!(
            once.text,
            twice.text,
            "not idempotent on {}",
            path.display()
        );
    }
}

/// Semantic fingerprint of a source: significant tokens with protected text
/// byte-exact and other text case-normalized (casing rules may legitimately
/// change case).
fn fingerprint(src: &str) -> Vec<(String, String)> {
    tokenize(src)
        .tokens
        .iter()
        .filter(|t| !t.kind.is_trivia())
        .map(|t| {
            let kind_class = match t.kind {
                // Classifications that may legitimately shift when layout
                // changes lexing context are merged.
                TokenKind::Identifier | TokenKind::Generic | TokenKind::Keyword(_) => {
                    "word".to_owned()
                }
                other => format!("{other:?}"),
            };
            let text = t.text(src);
            let norm = if t.kind.is_string() {
                text.to_owned()
            } else {
                text.to_lowercase()
            };
            (kind_class, norm)
        })
        .collect()
}

/// Protected content (strings, here-strings, comments) survives formatting
/// byte-for-byte, and the significant token stream is preserved modulo
/// permitted casing changes.
#[test]
fn corpus_preserves_protected_content_and_token_stream() {
    let opts = FormatOptions::default();
    for path in corpus_files() {
        let src = std::fs::read_to_string(&path).expect("read");
        let result = format(&src, &opts);
        if !result.formatted {
            assert_eq!(result.text, src);
            continue;
        }
        let strings_before: Vec<String> = tokenize(&src)
            .tokens
            .iter()
            .filter(|t| t.kind.is_string())
            .map(|t| t.text(&src).to_owned())
            .collect();
        let strings_after: Vec<String> = tokenize(&result.text)
            .tokens
            .iter()
            .filter(|t| t.kind.is_string())
            .map(|t| t.text(&result.text).to_owned())
            .collect();
        assert_eq!(
            strings_before,
            strings_after,
            "string content changed in {}",
            path.display()
        );

        let comments_before: Vec<String> = tokenize(&src)
            .tokens
            .iter()
            .filter(|t| t.kind.is_comment())
            .map(|t| t.text(&src).to_owned())
            .collect();
        let comments_after: Vec<String> = tokenize(&result.text)
            .tokens
            .iter()
            .filter(|t| t.kind.is_comment())
            .map(|t| t.text(&result.text).to_owned())
            .collect();
        assert_eq!(
            comments_before,
            comments_after,
            "comments changed in {}",
            path.display()
        );

        assert_eq!(
            fingerprint(&src),
            fingerprint(&result.text),
            "token fingerprint changed in {}",
            path.display()
        );
    }
}

/// Every corpus file formats identically under each preset without panicking.
#[test]
fn corpus_survives_all_presets() {
    for opts in [
        FormatOptions::default(),
        FormatOptions::otbs(),
        FormatOptions::allman(),
        FormatOptions {
            use_tabs: true,
            ignore_one_line_block: false,
            ..FormatOptions::default()
        },
    ] {
        for path in corpus_files() {
            let src = std::fs::read_to_string(&path).expect("read");
            let once = format(&src, &opts);
            let twice = format(&once.text, &opts);
            assert_eq!(
                once.text,
                twice.text,
                "not idempotent under preset on {}",
                path.display()
            );
        }
    }
}
