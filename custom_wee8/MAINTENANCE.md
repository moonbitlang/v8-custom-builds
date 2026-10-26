# Maintaining the wee8 runtime

This directory owns the native runtime and its C adapter. Review implementation
changes here as ordinary C++ source. Use upstream patches only for build
integration, engine insertion points, capability cuts and backported engine fixes.
Do not add another patch that edits the upstream C adapter alongside this copy.

## Source ownership and provenance

| Source | Owner and update rule |
| --- | --- |
| `src/c-api.cc` | Maintained V8 C adapter; reconcile upstream changes at every V8 update |
| `src/c-api.h`, `src/wasm.hh` | Private declarations used only by this adapter; update with its implementation |
| `include/wasm.h` | Unmodified standard C API header; update only to an explicit upstream revision |
| `include/wee8.h` | Our additive C API; review declaration, implementation and Rust binding together |
| Other `src/` files | Runtime policy and implementation selections owned by this repository |
| `patches/` | Changes that must remain inside pinned upstream dependencies |

[`upstream.json`](upstream.json) records the exact repository, revision, path and
SHA-256 of each copied upstream baseline. The private headers belong with the
adapter because they describe its handles and extension types. The standard
header and private C++ interface are different interfaces, not interchangeable
copies. Preserve the original copyright notices; V8's license is in
[`src/LICENSE`](src/LICENSE), and the Wasm C API license is in
[`../licenses/wasm-c-api.txt`](../licenses/wasm-c-api.txt).

GN selects the owned adapter only for `v8_custom_wee8`. V8's original `c-api.cc`,
`c-api.h`, `wasm.h` and `wasm.hh` stay untouched. Exactly one implementation of the
C API goes into an archive. The two implementations must never be linked together.

To see our complete adapter delta against the recorded baseline:

```sh
python3 tools/compare-adapter.py
```

This reads the pinned Git objects and checks their digests; it does not fetch,
patch or compile anything. Review this diff on every adapter or V8 update, in
addition to the ordinary PR diff. The comparison is generated, not a second
manually maintained source file.

## Patch series

The arrangement follows [workerd's V8 patch maintenance workflow](https://github.com/cloudflare/workerd/blob/7caf6dee5bfe9af1b7710ddf66222900007af080/docs/v8-updates.md):
keep a reviewable commit series and regenerate Git patches from it. Our
[`patches/series`](../patches/series) is the single application order. Numbered
filenames make each dependency's sequence easy to read; filesystem order is not
an input to the build.

| Directory under `patches/` | Apply relative to | Source revision |
| --- | --- | --- |
| `rusty_v8` | `rusty_v8` | This repository's `rusty_v8` gitlink |
| `chromium-build` | `rusty_v8/build` | Pinned rusty_v8's `build` gitlink |
| `v8` | `rusty_v8/v8` | Pinned rusty_v8's `v8` gitlink |

Each patch must explain:

- `Upstream-Status`: local policy/integration, or the upstream fix being backported.
- `Maintenance`: what must be checked on upgrade, and when a backport or workaround can be removed.
- `Tests`: the behavioral or build contract protecting the change.

Keep one purpose per patch. Preserve upstream commit links for backports. Local
policy patches have a continuing review obligation; they do not need a fictitious
upstream-removal date. The recipe check requires every patch exactly once in the
series, these metadata fields, the pinned dependency revisions and source digests.
It fails on tracked dependency edits instead of resetting them.

To prepare a fresh source checkout for inspection without compiling:

```sh
git submodule update --init --checkout --depth=1 rusty_v8
git -C rusty_v8 submodule update --init --checkout --depth=1 build v8
python3 tools/prepare-source.py
```

Inspect `git diff` in each of the three dependencies. Preparation stages
`custom_wee8` under rusty_v8 for GN, but never writes those edits back to the
maintained source. Use a fresh checkout for another preparation; preserve any
edits before replacing a prepared tree.

To edit a patch series, start with a separate clean checkout of that dependency
at its recorded pin. Apply its entries from `patches/series` using
`git am --keep-non-patch`, in their listed order. Edit normal source files and
amend or rebase the appropriate commits. Regenerate with:

```sh
git format-patch --full-index -k --no-signature --no-stat --zero-commit <upstream-pin>
```

Replace that dependency's patch files and update `patches/series` together.
Review the regenerated diff and verify fresh application. Do not repair hunk
counts or offsets manually. The initial numbered series records the previous
recipe commit and patch name in `Original-Recipe` for history lookup.

## Adapter maintenance contract

The adapter delegates execution, GC, strings and exceptions to V8. It owns C API
conversion and lifetime rules, not a second implementation of those mechanisms.
Changes must preserve these invariants:

- Roots, callbacks, imported objects and traps retain their store/isolate for the
  period in which native code can use them. Store teardown notifies the default
  platform before disposing the isolate. Test cleanup while the engine stays alive.
- Function types preserve supported reference nullability. Null checks protect
  direct and callback boundaries; unsupported types are rejected before native
  conversions. Type changes require matching introspection and call-path tests.
- Native exceptions retain tag identity and payload ownership. Trap diagnostics
  inspect only Wasm frames; JS-string builtin failures must remain reportable.
- Compile options survive serialization and module reuse. Native strings remain
  V8 values; they do not become an embedder-side replacement for V8 builtins.
- Standard `wasm.h` stays unmodified. Additive declarations belong in `wee8.h`.
  Changing declarations alone cannot enable functionality in an older archive.

Every native correctness fix needs a focused regression that fails before the
fix and passes afterward through the real consumer path. Prefer the existing
C consumer and native contract suite over standalone probing programs. Lifetime
regressions assert ownership release or bounded storage after warmup, not an RSS
threshold. Keep genuine new canonical types alive while reclaiming duplicates.

## Upgrading and validating

1. Record the old dependency pins. Compare old upstream, new upstream and our
   owned adapter; reconcile each change and update `upstream.json` together.
   A digest update alone is not an adapter review.
2. Rebase each dependency's patch commits. Drop fixes now supplied upstream;
   review the remaining `Maintenance` obligations, generated inventories and
   features that became unconditional. Preserve enabled behavior in ordinary V8.
3. Run the recipe/header checks. Inspect GN's selected adapter and configurations;
   never compile both adapters into the same archive.
4. Run the native matrix for all five supported targets, including the native
   contracts, lifetime checks, packaged C consumer and Rust tests against the
   newly produced archive and matching headers. Published-release Rust CI checks
   wrapper compatibility only; it does not validate changed native source.
5. For behavior changes, also run the affected Moonrun consumers. Native changes
   require a matching archive release before a downstream pin can use them.
   Publishing and downstream default changes are separate reviewed actions.

See [runtime validation](README.md#validation) for the existing test contracts and
[CI and releases](../README.md#ci-and-releases) for the publication checks.
