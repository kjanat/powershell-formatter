# pwsh-parser

`pwsh-parser` is the lossless, formatter-oriented PowerShell lexer and
structural parser used by `pwsh-formatter`.

It preserves the original source through byte-accurate spans and treats
strings, here-strings, and comments as protected content. The parser identifies
formatting-relevant structure without evaluating PowerShell or requiring a
PowerShell installation.

## Usage

```rust
use pwsh_parser::{parse, tokenize};

let source = "if($value){'yes'}";
let tokens = tokenize(source);
let structure = parse(source);

assert!(!tokens.tokens.is_empty());
assert!(!structure.root.children.is_empty());
```

This crate exposes lexical and structural data rather than a general-purpose
PowerShell AST or execution engine.

See the [project README](../../README.md) for the complete formatter project.
