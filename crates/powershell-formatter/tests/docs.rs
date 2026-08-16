//! The options table in docs/configuration.md is a public contract: this
//! test fails when an option is added, removed, or renamed without the
//! documentation following — the same treatment index.d.ts gets from the
//! npm package tests.

use powershell_formatter::FormatOptions;
use std::collections::BTreeSet;

#[test]
fn configuration_doc_covers_every_option() {
    let doc = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/configuration.md"
    ))
    .expect("read docs/configuration.md");

    // First backticked cell of each table row: `| `key` | ... |`.
    let documented: BTreeSet<String> = doc
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("| `")?;
            let (key, _) = rest.split_once('`')?;
            Some(key.to_owned())
        })
        .collect();

    let value = serde_json::to_value(FormatOptions::default()).expect("serialize");
    let actual: BTreeSet<String> = value
        .as_object()
        .expect("FormatOptions serializes to an object")
        .keys()
        .cloned()
        .collect();

    let missing: Vec<&String> = actual.difference(&documented).collect();
    let stale: Vec<&String> = documented.difference(&actual).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "docs/configuration.md is out of sync with FormatOptions:\n  undocumented: {missing:?}\n  documented but nonexistent: {stale:?}"
    );
}
