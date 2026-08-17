If you hate any of the reviews you get on PR's, and would like coderabbit to give more precise, detailed, helpful comments/reviews, then feel free to edit [it's config](./.coderabbit.yaml). The available settings are in the first line's jsonschema.

## Git tags

NEVER pass `--annotate`/`-a` to `git tag` — not in commands you run, not
in commands or scripts you suggest. The owner's git config signs tags;
`--annotate` overrides it and produces an UNSIGNED annotated tag, which
is unfixable once the tag has been published (release tags are
immutable). Create tags bare: `git tag <version>`.
