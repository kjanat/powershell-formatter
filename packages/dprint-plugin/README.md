# dprint-plugin-pwsh

The [PowerShell dprint plugin](https://github.com/kjanat/powershell-formatter)
as an npm package: `plugin.wasm` plus `getPath()`/`getBuffer()` accessors,
following the convention of `@dprint/json`, `@dprint/typescript`, and
friends. No runtime dependencies.

Using the dprint **CLI**? You don't need this package — run
`dprint add kjanat/pwsh` instead. This package is for hosting the plugin
from JavaScript via [`@dprint/formatter`](https://github.com/dprint/js-formatter),
and for a richer JS API (structured diagnostics, range formatting) see
[`pwsh-formatter`](https://www.npmjs.com/package/pwsh-formatter).

Also on JSR as
[`@kjanat/dprint-plugin-pwsh`](https://jsr.io/@kjanat/dprint-plugin-pwsh),
same version and API (`deno add jsr:@kjanat/dprint-plugin-pwsh`).

## Usage

```js
import { createFromBuffer } from "@dprint/formatter";
import { getBuffer } from "dprint-plugin-pwsh";

const formatter = createFromBuffer(getBuffer());
formatter.setConfig({}, { indentWidth: 4 });
console.log(formatter.formatText({
  filePath: "script.ps1",
  fileText: "function foo {\n'hi'\n  }\n",
}));
```

`@dprint/formatter` is your dependency, not this package's.

## As a dprint CLI install source

The packaged wasm also works directly in a `dprint.json` `plugins` array —
from the npm CDN mirrors, or offline from an installed `node_modules`:

```jsonc
{
	"pwsh": {},
	"plugins": [
		// pick one:
		"https://cdn.jsdelivr.net/npm/dprint-plugin-pwsh@x.y.z/plugin.wasm",
		"./node_modules/dprint-plugin-pwsh/plugin.wasm"
	]
}
```

The primary CLI path remains `dprint add kjanat/pwsh` (update notifications,
checksummed `latest.json`).
