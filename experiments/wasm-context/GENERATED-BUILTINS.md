# Replacing the generated JS omission lists

The private Wasm profile now selects generated JS-entry bodies from a positive
list: **120 retained, 312 unsupported**, out of 432 TFJ entries. This includes
all 115 entries from the earlier horizontal cut and its four Promise closures,
plus 193 additional omissions.

The candidate's `SetupBuiltinsInternal` contains no old omission-list include,
macro or selection rule. Its C++ selection already covers all 34 C++ entries
in the earlier horizontal cut. The generic builders for distinct trap bodies
remain; their selection now comes entirely from the whitelists.

The C API probe shrinks another **247,872 bytes (2.03%)**, from **12,199,264 to
11,951,392 bytes**, without native LTO. The context probe is 11,917,952 bytes:
50.43% below the upstream rusty_v8 0.102.0 context probe. That remains a
[build-profile comparison](LTO.md#common-context-probe), not a measured complete
Moonrun application.

## Selection and scope

[`generated-js-builtin-selection.h`](generated-js-builtin-selection.h) contains
the retained names and rationale. All other TFJ entries get the normal
CodeStubAssembler builder with an `Unreachable` body. Their IDs, argument
metadata, Code objects and registration order remain intact. Native wrappers
are explicitly retained regardless of JS linkage.

| Retained group | Count | Rationale |
| --- | ---: | --- |
| Entry/object helpers | 6 | Native API trampoline, receiver handling, object construction/coercion and a shared continuation. |
| Primitive helpers | 29 | Boolean, number, string and symbol conversion paths shared with native calls and diagnostics. |
| Sequence helpers | 7 | Ordinary array/string/typed-array iteration and coercion used by shared wrappers. No iterator convenience combinators. |
| Math | 36 | Conservatively preserve the bounded Math group used by Wasm well-known-import recognition and original-call fallbacks. |
| DataView accessors | 21 | Preserve the accessors recognized by Wasm import specialization and their fallbacks. |
| Wasm wrappers/imported strings | 21 | Keep JS-to-Wasm/JS-to-JS wrappers and imported-string builtins. |

These bodies do not install corresponding public globals or promise arbitrary
JS operation. The Math and DataView groups are conservative retention, not a
proof that every body is necessary for the native C API. `WasmPromising` remains
as an unchanged wrapper; public Promise integration remains unsupported.

Omitted groups include Date methods, Promise convenience and reaction entries,
async/generator entries, collection library methods, iterator conveniences,
typed-array conveniences, JS Atomics methods, reflection conveniences and
remaining selected JS library operations. Primitive representations, native
Wasm atomics and backing-store allocation remain separate.

This is a complete whitelist for **TFJ bodies**, not every generated builtin
kind. The catalog records the full target:

| Kind | Selection |
| --- | --- |
| CPP | Existing whitelist: 33 retained, 455 unsupported |
| TFJ | New whitelist: 120 retained, 312 unsupported |
| TFC | 516 original generators unchanged |
| TFS | 59 original generators unchanged |
| TFH | 85 original generators unchanged |
| ASM | 90 original generators unchanged |
| BCH | 499 bytecode-handler generators unchanged |
| TSJ/TSC | No entries in the measured configuration |

The 1,249 unchanged generated stubs/handlers are explicitly recorded as
`unchanged`, not described as individually audited whitelist entries. Extending
selection to them is separate work.

[`generated-builtins.tsv`](generated-builtins.tsv) records all 2,169 names,
kinds, IDs, selections and macro descriptor arguments under the actual build
definitions. The build rejects catalog drift, including additions to currently
empty kinds. Runtime selection remains at 144 retained / 475 unsupported.

## Measurements

Matched ARM64 macOS stripped probes, the same LLVM 18 linker, dead stripping,
and no native LTO. Baseline: runtime-whitelist revision `4ae08b0`.

| Measurement | Before | After | Saved |
| --- | ---: | ---: | ---: |
| Context probe | 12,182,336 | 11,917,952 | 264,384 |
| C API probe | 12,199,264 | 11,951,392 | 247,872 |
| C API WasmGC probe | 12,199,248 | 11,951,392 | 247,856 |
| Embedded code | 1,464,800 | 1,234,528 | 230,272 |
| Embedded metadata | 81,588 | 75,520 | 6,068 |
| Snapshot payload | 316,920 | 310,932 | 5,988 |

All 312 unsupported TFJ bodies have 24-byte instructions in this configuration.
The native implementations and source compilation inputs remain in place;
archive shrinkage comes from the regenerated embedded code/metadata/snapshot.
No runtime speed or build-time improvement has been measured.

The smaller constant table also changes emitted code in **29 unselected
generators**, saving 180 instruction bytes in total. For example,
`BooleanPrototypeToString` shrinks from 192 to 184 bytes: disassembly shows
indexed constant-table loads becoming single-instruction immediate-offset loads.
The generator and calling metadata are unchanged. Therefore equal instruction
sizes are not a valid universal invariant when regenerating the snapshot.
The evidence records every such size change instead of claiming all retained
machine bodies are byte-identical.

## Validation

Passed against the candidate:

- Context lifecycle, empty global properties and explicit JS text rejection.
- Native calls, callbacks, ownership, finalization, traps and invalid inputs.
- Wasm memory and table operations, bulk memory, indirect calls, internal
  exception tags, shared-memory atomics/growth, interrupts and serialization.
- WasmGC across forced collections and the 200,000-object allocation case.
- All native Wasm probes in Liftoff-only, optimizing-only, lazy compilation
  and synchronous-tier-up modes.
- Rust wrapper: 20 unit/integration tests and four doctests.
- Existing CPP and runtime registry checks and deliberate unsupported calls.
- All 2,169 Code objects have distinct identities matching their builtin IDs.
  IDs, kinds and parameter counts match the preceding candidate.
- Every one of the 312 omitted TFJ entries is invoked in a separate process,
  using a synthetic function with the correct argument-adaptation metadata.
  Each reaches its own entry and traps; no installed JS property is required.
- All 119 earlier generated omissions remain omitted, and the candidate setup
  source contains no reference to the old omission policy.

The negative-call harness rejects a normal return, its own generic fatal
fallback, and memory-access faults. It validates the generated entry, not merely
the selector's boolean result.

The [previous limitations](RUNTIME-ENTRIES.md#validation) remain: shared memory
stays inside the guest, there is no multithreaded wakeup stress, and the guest
exception case does not validate the newly merged native exception-tag API.
This prototype still needs integration with current main, the actual Moonrun
consumer and the supported-platform matrix.

Exact results are in
[`generated-builtin-results.json`](generated-builtin-results.json).

## Reproduction and retiring the old patches

Start with the context-only output from `build.py`. The new option includes
runtime selection and replaces the old setup policy in the isolated source
copy; it does not layer an allowlist over an active omission list.

```sh
python3 experiments/wasm-context/build-entry-allowlists.py \
  --source "$RUSTY_V8_SOURCE" --out "$GN_OUT" \
  --context "$CONTEXT_OUT" --work "$GENERATED_OUT" \
  --cxx "$CXX" --ar "$LLVM_AR" --generated-js-allowlist

python3 experiments/wasm-context/check.py \
  --source "$RUSTY_V8_SOURCE" --out "$GN_OUT" \
  --baseline "$RUNTIME_OUT/libwee8.a" --candidate "$GENERATED_OUT/libwee8.a" \
  --work "$GENERATED_CHECKS" --cxx "$CXX" --linker "$LLVM_LINKER" \
  --probes context c-api c-api-gc --cpp-entry-contract --runtime-checks

python3 experiments/wasm-context/check-generated-builtins.py \
  --out "$GN_OUT" --baseline "$RUNTIME_OUT/libwee8.a" \
  --candidate "$GENERATED_OUT/libwee8.a" --work "$GENERATED_ENTRY_CHECKS" \
  --cxx "$CXX" --linker "$LLVM_LINKER"
```

[`generated-js-entry-selection.patch`](generated-js-entry-selection.patch)
records the setup-policy replacement relative to the preceding prototype.
The driver still relies on the earlier build layout and generic trap-builder
plumbing. It remains experimental build machinery.

The production patch stack and its published profile are unchanged by this
experiment. On landing the combined GN profile:

1. Carry the generic trap-builder plumbing into the whitelist patch and remove
   `wee8-without-library-bodies.patch` and its Promise-closure follow-up.
2. Remove their omission-list header and configuration flag. Preserve the
   upstream implementation when the new profile is off.
3. Replace public-property-registration tests with the new identity/metadata
   checks and native retained-behavior tests. Keep useful behavior coverage.
4. Integrate the context, CPP/runtime/TFJ selection and snapshot generation into
   one supported profile; make inventory validation mandatory.
5. Reconcile current main's exception support, validate the consumer, and run
   the supported native matrix before release.

Removing the production patches before wiring the replacement would restore
bodies or break the current recipe. This experiment establishes their
replacement policy and evidence; production deletion belongs in the same
change that enables the new profile.
