# dprint-plugin-pwsh

`dprint-plugin-pwsh` packages `pwsh-formatter` as a dprint WebAssembly plugin.
It resolves dprint configuration, formats PowerShell source, and can generate
the plugin's configuration schema.

The crate is an internal build target and is not published directly to
crates.io. Release builds produce the `plugin.wasm` artifact distributed by the
project.

Example `dprint.json` configuration:

```json
{
	"pwsh": { "indentWidth": 4, "braceStyle": "sameLine" },
	"plugins": ["<url-or-path-to>/plugin.wasm"]
}
```

See the [project README](../../README.md) and the [dprint package documentation](../../packages/dprint-plugin/README.md) for installation and
build details.
