//! Benchmarks: scanner throughput, structural analysis, and full formatting
//! over tiny/medium/large/pathological inputs.
//!
//! Run: `cargo bench -p pwsh-formatter`

use pwsh_formatter::{FormatOptions, format};
use pwsh_parser::{parse, tokenize};

fn tiny() -> String {
    "if ($x) { Get-Item $x }\n".to_owned()
}

fn medium() -> String {
    // A realistic ~150-line real-world script.
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/files/pssa-resx.ps1"
    ))
    .expect("corpus file")
}

fn large() -> String {
    // ~700 lines of real-world PowerShell, repeated to ~10k lines.
    let base = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/files/pwsh-Parsing.Tests.ps1"
    ))
    .expect("corpus file");
    base.repeat(14)
}

fn pathological_nesting() -> String {
    let depth = 120;
    let mut src = String::new();
    for _ in 0..depth {
        src.push_str("if ($x) { @( @{ k = $(\n");
    }
    src.push_str("1\n");
    for _ in 0..depth {
        src.push_str(") } ) }\n");
    }
    src
}

fn heavy_herestrings() -> String {
    let body = "line of protected content with $variables and 'quotes'\n".repeat(200);
    format!("$a = @'\n{body}'@\n$b = @\"\n{body}\"@\n").repeat(10)
}

fn inputs() -> [(&'static str, String); 5] {
    [
        ("tiny", tiny()),
        ("medium", medium()),
        ("large", large()),
        ("nesting", pathological_nesting()),
        ("herestrings", heavy_herestrings()),
    ]
}

#[divan::bench(args = ["tiny", "medium", "large", "nesting", "herestrings"])]
fn scan(bencher: divan::Bencher<'_, '_>, name: &str) {
    let src = inputs()
        .into_iter()
        .find(|(n, _)| *n == name)
        .expect("input")
        .1;
    bencher
        .counter(divan::counter::BytesCount::new(src.len()))
        .bench(|| tokenize(divan::black_box(&src)));
}

#[divan::bench(args = ["tiny", "medium", "large", "nesting", "herestrings"])]
fn structural_parse(bencher: divan::Bencher<'_, '_>, name: &str) {
    let src = inputs()
        .into_iter()
        .find(|(n, _)| *n == name)
        .expect("input")
        .1;
    bencher
        .counter(divan::counter::BytesCount::new(src.len()))
        .bench(|| parse(divan::black_box(&src)));
}

/// Nesting-depth scaling: the structural parse holds one heap frame per
/// open delimiter, so time and memory should stay linear in depth.
#[divan::bench(args = [1_024, 8_192, 32_768])]
fn parse_depth(bencher: divan::Bencher<'_, '_>, depth: usize) {
    let src = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
    bencher
        .counter(divan::counter::BytesCount::new(src.len()))
        .bench(|| parse(divan::black_box(&src)));
}

#[divan::bench(args = ["tiny", "medium", "large", "nesting", "herestrings"])]
fn format_full(bencher: divan::Bencher<'_, '_>, name: &str) {
    let src = inputs()
        .into_iter()
        .find(|(n, _)| *n == name)
        .expect("input")
        .1;
    let opts = FormatOptions::default();
    bencher
        .counter(divan::counter::BytesCount::new(src.len()))
        .bench(|| format(divan::black_box(&src), &opts));
}

fn main() {
    divan::main();
}
