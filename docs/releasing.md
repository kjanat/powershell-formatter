# Releasing

All released artifacts are built from the same commit and share one version
(`workspace.package.version`).

## Artifacts

| Artifact           | Build                                                                    | Ship as                                                      |
| ------------------ | ------------------------------------------------------------------------ | ------------------------------------------------------------ |
| `psfmt` native CLI | `cargo build --release -p psfmt`                                         | platform binaries                                            |
| dprint plugin      | `cargo wasm-plugin` (alias)                                              | `plugin.wasm` + `deployment/schema.json` on a GitHub release |
| npm package        | `packages/formatter/build.sh` then `npm publish` in `packages/formatter` | `@kjanat/powershell-formatter`                               |

## dprint plugin conventions

- The release asset **must** be named `plugin.wasm` (copy
  `target/wasm32-unknown-unknown/wasm-release/dprint_plugin_powershell.wasm`).
- Regenerate and publish `schema.json`
  (`cargo run -p dprint-plugin-powershell --features schema --bin generate-schema`);
  its `$id` embeds the version.
- Tag releases with **bare semver** (`0.1.0`, no `v` prefix).
- Published versions are immutable (plugins.dprint.dev caches aggressively):
  never re-upload an asset to an existing release — bump and re-release.
- Users install with a URL to the released `plugin.wasm` (or
  `dprint add kjanat/powershell` once the proxy repo shape is in place).

## npm package

`packages/formatter` ships only `index.js` (browser), `index.node.js`
(Node), `index.d.ts`, and the two generated files in `dist/`. Keep the
package version in lockstep with the workspace; npm publishes are immutable
too.

## Pinned toolchain pieces

- `wasm-bindgen-cli` must match the `wasm-bindgen` version pinned in
  `crates/powershell-formatter-wasm/Cargo.toml` (currently 0.2.127).
- Oracle fixtures record their generators: pwsh version in
  `tests/powershell-oracle/fixtures/VERSION`, PSScriptAnalyzer version in
  `tests/pssa-parity/expected/VERSION`. Regenerate deliberately, review the
  diff, and update the pins together.

## Measured sizes (this commit, x86_64-linux)

```text
psfmt (release, stripped)             659 KB
dprint plugin.wasm (raw / gzip)       282 KB / 105 KB
browser wasm (raw / gzip)             188 KB / 74 KB
npm package tarball                    82 KB
```

CI enforces a 1 MB raw budget on both wasm artifacts.
