# Unsupported JS library bodies in custom wee8

The custom wee8 profile replaces 149 explicitly selected JS library entry bodies
with intentional machine traps. Calling one of these methods terminates the
process; it does not return a fake result or a catchable JS exception. This is a
supported profile boundary, not preservation of the omitted APIs' functionality.

`patches/wee8-without-library-bodies.patch` is part of the production patch stack.
The GN argument `v8_moonrun_omit_library_builtin_bodies` defaults to the custom
wee8 marker, so ordinary V8 keeps its original bodies. Override it to `false`
for a matching control build. Its define is confined to `v8_initializers`.
The selected names remain explicit and independent of prefixes or linkage.

## Generation boundary

`SetupIsolateDelegate::SetupBuiltinsInternal` selects the replacement bodies:

- The 115 TFJ entries use the ordinary CodeStubAssembler builder with the same
  builtin ID, name and argc, but generate `Unreachable` (an intentional trap).
- The 34 CPP entries use the ordinary adaptor builder and Code metadata, but
  preserve the architecture-specific entry landing pad and emit `DebugBreak`
  instead of entering the C++ function. Their implementation
  functions and the external-reference/function-pointer tables remain intact.

Kind checks reject a version upgrade that changes a selected entry's linkage.
Every entry has its own Code object and builtin identity; none aliases Illegal.
Registrations, layouts, bootstrap references, shared helpers and continuations
remain intact. The embedded blob and snapshot are regenerated together.
Parser removal, Wasm wrappers and shared compiler infrastructure are unchanged.

## Selection and dependency rationale

The authoritative lists are `MOONRUN_OMITTED_TFJ_BUILTINS` and
`MOONRUN_OMITTED_CPP_BUILTINS` in the patch. `omitted-builtins.md` enumerates every
name and its measured instruction size. The pinned-source audit covers both
Torque references and C++ builtin enum references, plus macro-generated uses.
Reflective JS calls are intentionally unsupported; textual reference searches
alone cannot establish that arbitrary JS programs will never call these APIs.

| Surface | Selected | Boundary and retained dependencies |
| --- | ---: | --- |
| Array methods | 26 | Callback/search/copy/sort/mutation conveniences; keep constructors, iterators, push/pop, join and shared element machinery. |
| Array statics | 3 | from/fromAsync/of entry points; keep constructor and async/internal continuation bodies. |
| TypedArray methods | 17 | Callback/search/slice/sort conveniences; keep constructors, buffers, element access, set/subarray and shared conversion/copy machinery. Wasm memory is unaffected. |
| Set algebra | 7 | Set combinations and relation checks; keep construction, add/delete/has, iteration and hash-table machinery. |
| Iterator conveniences | 11 | map/filter/take/drop/flatMap and terminal conveniences; keep next/return, Iterator.from, helper resume bodies and concrete collection iterators. |
| Promise conveniences | 5 | all/allSettled/any/race/withResolvers; keep constructor, resolution, then/catch/finally, reactions, combinator closures and microtasks. |
| Object statics | 6 | assign/entries/values/keys/fromEntries/groupBy; keep object creation, property primitives and conversions. These JS methods are separate from internal enumeration helpers. |
| Reflect | 11 | Property/prototype convenience entries; keep apply/construct and the underlying property, prototype and call machinery. |
| Date | 43 | Calendar getters/setters, formatting and now/parse/UTC; keep constructor, valueOf, toString and Symbol.toPrimitive for conversion. Native clocks and Date C++ implementations remain. |
| Number formatting | 4 | toExponential/toFixed/toPrecision/toLocaleString; keep number conversion, toString/valueOf and arithmetic. |
| JSON | 2 | JS parse/stringify entry adaptors; keep internal JSON parsing/stringification and the public C++ JSON API. |
| Map grouping | 1 | groupBy; keep Map storage, lookup, mutation and iteration. |
| String HTML wrappers | 13 | Legacy HTML-generation methods only; keep character access, encoding, concatenation, iteration and all Wasm string operations. |

Important retained paths:

- Wasm JS host-import multi-result conversion calls `IterableToFixedArrayForWasm`.
  That builtin and the core iterator protocol remain.
- Async Wasm uses Promise resolvers; promise integration calls `PerformPromiseThen`.
  Both remain, along with the runtime's job and microtask infrastructure.
- Wasm import resolution recognizes Math builtin IDs, including macro-generated
  cases. All Math methods remain, including their ordinary-call fallbacks.
- `JsonStringify` references in API/tracing code name a C++ implementation helper,
  not a call through the omitted JS entry adaptor. The helper remains.
- RegExp and Intl are already disabled by the production profile. There are no
  corresponding library bodies to add here. Their existing absent-registration
  and unsupported-operation checks continue to run.

This is a broad entry-body cut, not a claim that every unused generated helper
is gone. DataView, ArrayBuffer, shared-memory/Atomics, core strings, constructors,
conversions, Math and execution machinery remain. Date implementation removal
and C++ dispatch-table pruning are separate work.

## Validation

`wee8-build` builds `wee8-library-check` in the same custom toolchain as the
archive. Every native candidate CI run invokes `check.py --after PROBE`:

- All 149 property registrations resolve to their expected builtin IDs and
  distinct Code objects, with no Illegal alias. CPP dispatch entries remain.
- Each omitted method runs in its own process and must reach its named entry
  before an intentional trap. POSIX and Windows trap statuses are explicit.
- Retained Array iteration, Promise resolution/reactions/microtasks and a Wasm
  host import returning an iterator for two results are exercised.
- Those retained checks and the shared runtime probe run in Liftoff-only,
  optimizing-only, lazy and synchronous tier-up modes.
- The existing C API and Rust adapter suites run against the same candidate.

For a before/after comparison, build `wee8-build` first with the GN argument
`false`, preserve the artifacts, then rebuild with the custom default. Preserve
`libwee8.a`, `wee8-library-check` as `probe`, `embedded.S`, `snapshot.cc` and an
identically built/stripped consumer from each configuration. Never mix blobs
and snapshots from different configurations.

```sh
python3 ci/library-builtins/check.py --before BEFORE/probe --after AFTER/probe \
  --output identities.json
python3 ci/library-builtins/measure.py --before BEFORE --after AFTER \
  --identities identities.json --output sizes.json
```

The comparison also verifies original IDs/argument metadata and instruction-size
reduction for every selected entry. Tiny CPP adaptor replacements may occupy the
same aligned embedded slot; unchanged slot size is not a failure. All unselected
embedded body lengths must remain unchanged.

## Measured result and landing gates

The recorded artifact target is `aarch64-apple-darwin`, against the merged
asm.js-cut baseline. This is a matched test consumer, not a complete moonrun
executable. `results.json` records the scope, sizes and archive hashes.

| Component | Before | After | Saved |
| --- | ---: | ---: | ---: |
| Embedded code | 1,741,728 B | 1,469,888 B | 271,840 B (15.61%) |
| Embedded metadata | 88,844 B | 81,684 B | 7,160 B (8.06%) |
| Snapshot payload | 406,456 B | 402,096 B | 4,360 B (1.07%) |
| Static archive | 60,774,168 B | 60,484,120 B | 290,048 B (0.48%) |
| Stripped consumer | 15,640,144 B | 15,359,312 B | 280,832 B (1.80%) |

All 149 comparison/trap tests, retained protocol checks in four Wasm modes and
C API/Rust adapter tests passed for the measured artifact. Native JSON, property enumeration and Date coercion also passed. The disabled
profile produces a byte-identical pre-cut archive, and the production-default
archive matches the measured candidate. Targeted native CI must validate the
final PR head before landing; the earlier 66-entry run is not evidence for this
149-entry version. Windows trap handling and other release platforms still need
native CI validation before publishing their artifacts. No runtime speedup is
claimed. The existing release workflow requires a successful five-platform run.

The requested moonrun main was inspected at
`efda7a5737998d5af78bdaafbd69a6966e8405d9`. It uses rusty_v8 and compiles JS glue;
full integration with that checkout is not claimed for this already-parser-free
profile. Validation uses this producer's wee8 Rust adapter and native Wasm probes.
