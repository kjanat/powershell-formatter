ifeq ($(OS),Windows_NT)
SHELL := C:/Program Files/Git/bin/bash.exe
else
SHELL := /bin/bash
endif

.SHELLFLAGS := -eu -o pipefail -c
.DEFAULT_GOAL := check

CARGO ?= cargo
DPRINT ?= $(shell command -v dprint >/dev/null 2>&1 && printf dprint || printf 'npx -y dprint')
JSR ?= $(shell command -v jsr >/dev/null 2>&1 && printf jsr || printf 'npx -y jsr')
JSR_PACKAGES := packages/formatter packages/dprint-plugin
NPM ?= npm
WASM_BINDGEN ?= wasm-bindgen
YQ ?= yq
PACK_DIR ?= npm-tarballs
PACK_DESTINATION := $(abspath $(PACK_DIR))
WASM_TARGET ?= wasm32-unknown-unknown
FUZZ_TARGET ?= formatter
FUZZ_SECONDS ?= 60
FUZZ_HOST ?= $(shell rustc +nightly --print host-tuple)

.PHONY: \
	fmt fmt-check lint rust-build rust-test rust-check doc-test bench \
	schema schema-check wasm-plugin wasm-formatter wasm \
	dprint-e2e oracle-test oracles fuzz fuzz-smoke \
	install npm-build npm-test typecheck npm-check \
	jsr-check jsr-publish \
	build test check ci pack release clean

fmt:
	$(DPRINT) fmt

fmt-check:
	$(DPRINT) check

lint:
	$(CARGO) lint

rust-build:
	$(CARGO) build --workspace --all-targets --all-features

rust-test:
	$(CARGO) nextest run --workspace
	$(CARGO) test --workspace --doc

doc-test:
	$(CARGO) test --workspace --doc

rust-check: fmt-check lint rust-test

bench:
	$(CARGO) bench -p pwsh-formatter

schema:
	$(CARGO) run -p dprint-plugin-pwsh --features schema --bin generate-schema

schema-check:
	@before="$$(mktemp)"; \
	trap 'rm -f "$${before}"' EXIT; \
	cp crates/dprint-plugin-pwsh/deployment/schema.json "$${before}"; \
	$(MAKE) schema; \
	cmp -s "$${before}" crates/dprint-plugin-pwsh/deployment/schema.json || { \
		diff -u "$${before}" crates/dprint-plugin-pwsh/deployment/schema.json; \
		exit 1; \
	}

wasm-plugin:
	$(CARGO) build -p dprint-plugin-pwsh --profile wasm-release --target $(WASM_TARGET)
	cp "target/$(WASM_TARGET)/wasm-release/dprint_plugin_pwsh.wasm" \
		"packages/dprint-plugin/plugin.wasm"
	ls -la "packages/dprint-plugin/plugin.wasm"

wasm-formatter:
	@pinned="$$($(YQ) '.workspace.dependencies.wasm-bindgen' Cargo.toml)"; \
	pinned="$${pinned#=}"; \
	actual="$$($(WASM_BINDGEN) --version | awk '{print $$2}')"; \
	if [ -z "$${pinned}" ] || [ "$${pinned}" != "$${actual}" ]; then \
		echo "wasm-bindgen-cli $${actual} does not match the pinned wasm-bindgen $${pinned:-<unparsed>}" >&2; \
		exit 1; \
	fi
	rm -rf -- "packages/formatter/dist"
	$(CARGO) build -p pwsh-formatter-wasm --profile wasm-release --target $(WASM_TARGET)
	$(WASM_BINDGEN) \
		--target web \
		--out-dir "packages/formatter/dist" \
		"target/$(WASM_TARGET)/wasm-release/pwsh_formatter_wasm.wasm"
	ls -la "packages/formatter/dist"

wasm: wasm-plugin wasm-formatter

dprint-e2e:
	crates/dprint-plugin-pwsh/tests/e2e.sh

oracle-test:
	$(CARGO) nextest run -p pwsh-parser --test oracle
	$(CARGO) nextest run -p pwsh-formatter --test pssa_parity

oracles:
	pwsh -NoProfile -File tests/powershell-oracle/generate.ps1
	pwsh -NoProfile -File tests/pssa-parity/generate.ps1
	pwsh -NoProfile -File tests/pssa-parity/generate-catalog.ps1

fuzz:
	$(CARGO) +nightly fuzz run "$(FUZZ_TARGET)"

fuzz-smoke:
	$(CARGO) +nightly fuzz run "$(FUZZ_TARGET)" --target "$(FUZZ_HOST)" -- "-max_total_time=$(FUZZ_SECONDS)"

install:
	$(NPM) ci

npm-build:
	$(NPM) run build --workspaces

npm-test:
	$(NPM) test --workspaces

typecheck:
	$(NPM) run typecheck

npm-check: npm-build typecheck npm-test

# Everything a real publish verifies (file set, ESM rules, slow types),
# minus the upload. Needs the wasm artifacts, so run after npm-build.
jsr-check:
	for pkg in $(JSR_PACKAGES); do \
		(cd "$${pkg}" && $(JSR) publish --dry-run --allow-dirty); \
	done

# Publishes from CI over OIDC; locally it opens a browser to authorize.
# Skips versions already on the registry.
jsr-publish:
	for pkg in $(JSR_PACKAGES); do \
		(cd "$${pkg}" && $(JSR) publish); \
	done

build: rust-build npm-build

test: rust-test dprint-e2e npm-test

check: fmt-check lint schema-check build typecheck test

ci: check

pack:
	mkdir -p "$(PACK_DESTINATION)"
	$(NPM) pack --workspaces --pack-destination "$(PACK_DESTINATION)"

release: check pack

clean:
	$(CARGO) clean
	rm -rf -- "$(PACK_DESTINATION)" node_modules/ packages/formatter/dist/ packages/dprint-plugin/plugin.wasm
