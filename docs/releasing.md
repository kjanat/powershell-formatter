# Releasing

All released artifacts are built from the same commit and share one version
(`workspace.package.version` in [`Cargo.toml`]).

## Artifacts

| Artifact           | Build                                                                 | Ship as                                                      |
| ------------------ | --------------------------------------------------------------------- | ------------------------------------------------------------ |
| `psfmt` native CLI | `cargo build --release -p psfmt`                                      | platform binaries                                            |
| dprint plugin      | `cargo wasm-plugin` (alias in [`.cargo/config.toml`])                 | `plugin.wasm` + `schema.json` + `latest.json` GitHub release |
| npm package        | [`packages/formatter/build.sh`] then `npm publish` in the package dir | `@kjanat/powershell-formatter`                               |

## dprint plugin conventions

- The release asset **must** be named `plugin.wasm` (copy
  `target/wasm32-unknown-unknown/wasm-release/dprint_plugin_powershell.wasm`).
- Regenerate and publish [`schema.json`]
  (`cargo run -p dprint-plugin-powershell --features schema --bin generate-schema`);
  its `$id` embeds the version.
- Also attach a `latest.json` (`{ "schemaVersion": 1, "url": "…/plugin.wasm",
  "version": "X.Y.Z" }`): the plugin's `update_url` points at
  `releases/latest/download/latest.json`, which GitHub keeps aimed at the
  newest release automatically.
- Tag releases with **bare semver** (`0.1.0`, no `v` prefix) — the tag must
  equal the crate version, because the plugin derives its release-asset URLs
  from `CARGO_PKG_VERSION`.
- Published versions are immutable (dprint installs cache aggressively):
  never re-upload an asset to an existing release — bump and re-release.
- Users install with the URL of a released `plugin.wasm`. The
  `dprint add kjanat/<name>` shorthand does **not** apply: the
  plugins.dprint.dev proxy only resolves repositories literally named
  `dprint-plugin-<name>`, and this plugin lives inside the
  `powershell-formatter` monorepo. If the shorthand ever matters, the plugin
  crate has to move (or be mirrored) to a `kjanat/dprint-plugin-powershell`
  repository with its own releases.

## npm package

The published package contains the entry points (`index.js`,
`index.node.js`, `index.d.ts`), the two generated files in `dist/`, and the
usual npm metadata ([`package.json`], `LICENSE`). Keep the package version
in lockstep with the workspace; npm publishes are immutable too.

## Pinned toolchain pieces

- `wasm-bindgen-cli` must match the `wasm-bindgen` version pinned in
  [`Cargo.toml`]'s `[workspace.dependencies]` (currently 0.2.127);
  [`packages/formatter/build.sh`] refuses to build on a mismatch.
- Oracle fixtures record their generators: pwsh version in
  [`tests/powershell-oracle/fixtures/VERSION`], PSScriptAnalyzer version in
  [`tests/pssa-parity/expected/VERSION`] (the generators and CI install
  exactly that analyzer build). Regenerate deliberately, review the diff,
  and update the pins together.

## Size expectations (x86_64-linux baseline)

Measured while this was written; re-measure at the release tag rather than
trusting these numbers:

```text
psfmt (release, stripped)             659 KB
dprint plugin.wasm (raw / gzip)       282 KB / 105 KB
browser wasm (raw / gzip)             188 KB / 74 KB
npm package tarball                    82 KB
```

CI enforces a 1 MB raw budget on both wasm artifacts (see [`ci.yml`]).

[`Cargo.toml`]: ../Cargo.toml
[`.cargo/config.toml`]: ../.cargo/config.toml
[`packages/formatter/build.sh`]: ../packages/formatter/build.sh
[`package.json`]: ../packages/formatter/package.json
[`schema.json`]: ../crates/dprint-plugin-powershell/deployment/schema.json
[`tests/powershell-oracle/fixtures/VERSION`]: ../tests/powershell-oracle/fixtures/VERSION
[`tests/pssa-parity/expected/VERSION`]: ../tests/pssa-parity/expected/VERSION
[`ci.yml`]: ../.github/workflows/ci.yml
