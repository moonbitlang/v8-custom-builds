# Native Wasm profile

`custom_wee8` enables `v8_enable_wasm_native_context`. This single GN capability
selects the context initializer and all three implementation policies together.
Ordinary V8 defaults to `false`, retaining its original initialization, tables
and implementations. It is an alternative runtime profile, not a general JS
context with some methods hidden.

The profile preserves native Wasm compilation, execution and host callbacks.
There are no JS library properties on its global object. Private V8 object maps,
contexts, error objects and Wasm wrappers remain because the engine and C adapter
use them. Existing parser, asm.js and RegExp capability patches are independent;
shared compiler infrastructure and native wrappers are unchanged here.

## Selection boundaries

| Boundary | Positive policy | Why it remains |
| --- | --- | --- |
| Native context | `wasm-context-bootstrap.inc` | Function/object maps for callbacks; array and buffer maps for results, stacks and memory; WeakMap for host-info finalization; errors/CallSite for diagnostics; factory caches and serializer state |
| C++ builtins | `cpp-builtin-selection.h` | 33 entries: native function-template entry, core sentinels, Error/CallSite diagnostics |
| Runtime helpers | `runtime-selection.h` | 144 canonical entries: all declared Wasm helpers plus shared allocation, interrupts, error propagation, primitive conversion, diagnostics and host-info ownership |
| Generated JS entry bodies (TFJ) | `generated-js-builtin-selection.h` | 120 entries: native entry/coercion, primitive conversion, conservative sequence/Math/DataView fallbacks and Wasm wrappers/imported strings |

The counts describe the pinned non-Intl, non-RegExp profile. The compiled
[inventories](../../ci/wasm-profile) record every selected and omitted entry with
its signature. They are audited upgrade inputs, not another selection mechanism.
Runtime inline aliases retain their canonical entry and metadata; pair-return
runtime traps retain the original return convention.

Unselected C++ and runtime entries point to distinct typed functions that report
the original name and terminate. All three C++ entry consumers use the same
selection: builtin metadata, external references and generated adaptors. No
omitted implementation is retained merely through those function-pointer tables.
Other direct references may still keep it linked.

Unselected TFJ bodies, and the adaptors of unselected C++ entries, contain explicit
traps. The original builders still create separate Code objects with their
original builtin IDs, descriptors and argument metadata. They do not alias the
Illegal builtin. Generated machine-code offsets and sizes can change when the
blob is regenerated; preserving metadata does not imply byte-identical bodies.

Date, Promise, Temporal, collection, JSON and other unselected API bodies are
unsupported. RegExp and Intl were already disabled by separate profile settings.
The retained sequence/Math/DataView bodies are conservative allowances for shared
Wasm machinery, not promises of usable JS APIs: their globals and constructors
are not installed. The existing `WasmJs::PrepareForSnapshot` infrastructure is
retained, so this is not a complete whitelist of every process-wide allocation.

Other generated kinds (TFC/TFS/TFH/ASM/BCH) remain upstream implementations. They
include Wasm, allocation, dispatch and diagnostic machinery; JS linkage alone is
not grounds for removal. C++ source compilation, bytecode-handler removal and a
smaller Wasm registration layer remain separate work.

## Build and upgrade contract

`build.sh` copies these owned sources into `v8/src/wasm-profile` before GN runs.
`wasm-native-context.patch` contains the small insertion points into upstream
code. `wee8-profile.patch` declares the capability and supplies one common define
to the custom toolchain, including mksnapshot and the final archive. The normal GN
snapshot dependency graph regenerates the embedded blob and matching snapshot.
No post-build archive rewriting is part of the producer.

On a V8 update:

1. Reapply the insertion patch and review each native-context allocation against
   its factory/serializer consumers. Preserve upstream initialization as a
   separate branch.
2. Review the compiled inventory diff before accepting new/renamed builtin kinds,
   runtime entries or signatures. A new unselected CPP/TFJ/runtime entry defaults
   to unsupported; a new Wasm runtime group member is retained and requires review
   through the inventory diff.
3. Verify native entry tables, aliases, external references, deserialized Code
   identities, 767 generated unsupported traps and named native-entry failures with `ci/wasm-profile/check.py`.
4. Run source-rejection checks, native Wasm boundary guests and the packaged C/Rust
   suites, including exceptions, followed by the actual Moonrun wee8 backend.
5. Validate supported platforms before publishing. Never mix snapshots or headers
   between profiles or revisions.

This replaces the previous horizontal library-body omission patch, including the
Promise closures from the prototype branch. There is one active policy, not an
intersection of positive and negative lists. The separate Date source guard is
not needed for this integration; further source cuts must demonstrate an
additional linked-size benefit.
