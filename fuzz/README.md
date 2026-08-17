# Fuzzing

Three `cargo-fuzz` (libFuzzer) targets, seeded from the real-world corpus:

- `lexer` — arbitrary bytes never panic; valid UTF-8 tokenizes losslessly
  with in-bounds, character-aligned spans.
- `parser` — structural analysis never panics; delimiter matches stay
  symmetric and in-bounds.
- `formatter` — formatting never panics, is deterministic and idempotent,
  and preserves string/comment content byte-for-byte.

## Running

```sh
cargo +nightly fuzz run formatter               # indefinitely
cargo +nightly fuzz run formatter -- -max_total_time=60
cargo +nightly fuzz run lexer -- -runs=1000000
```

CI runs a short smoke pass of each target on every push (see
`.github/workflows/ci.yml`); longer runs are manual.
