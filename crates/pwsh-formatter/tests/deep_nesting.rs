//! The full pipeline — lex, structural parse, every formatting phase —
//! at 32k nesting levels on a 1 MiB stack. PowerShell's own parser gives
//! up around 10k–20k levels (`ScriptTooComplicated`), so there is no
//! oracle out here; the invariants are ours: no abort, determinism,
//! idempotence (see docs/formatting.md, "Intentional divergences").

use pwsh_formatter::{FormatOptions, format};

fn on_one_mib_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(f)
        .expect("spawn thread")
        .join()
        .expect("must not overflow a 1 MiB stack");
}

#[test]
fn deep_balanced_nesting_formats_idempotently() {
    on_one_mib_stack(|| {
        let depth = 32_768;
        let src = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
        let opts = FormatOptions::default();
        let once = format(&src, &opts);
        assert!(once.formatted, "32k levels format, not decline");
        let twice = format(&once.text, &opts);
        assert_eq!(once.text, twice.text, "idempotent at 32k levels");
    });
}

#[test]
fn deep_unbalanced_nesting_is_preserved() {
    on_one_mib_stack(|| {
        let depth = 32_768;
        let src = format!("{}1", "(".repeat(depth));
        let opts = FormatOptions::default();
        let out = format(&src, &opts);
        assert!(!out.formatted, "structurally uncertain input is declined");
        assert_eq!(out.text, src, "declined input is preserved byte-for-byte");
    });
}
