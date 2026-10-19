# Moonrun generated builtin-body experiment

This is an opt-in experiment, outside the production patch stack. Recipe CI
checks that it still applies after the production patches, without enabling it. It replaces
50 explicitly selected JavaScript standard-library entry bodies with intentional
machine traps. It preserves their IDs, registrations, argument descriptors,
bootstrap references and distinct Code objects. Calling an omitted method is
unsupported and terminates the process; it does not return a fake result or a
catchable JavaScript exception.

The normal custom wee8 configuration remains the control. Apply
`omit-bodies.patch` **after** all production patches. The GN argument
`v8_moonrun_omit_library_builtin_bodies` defaults to `false`; even when enabled,
it only applies in the `v8_custom_wee8` toolchain scope. The macro is confined to
`v8_initializers`, where the builtin generators run. It does not alter the
runtime's global feature definitions.

`SetupIsolateDelegate::SetupBuiltinsInternal` selects the generator for each
listed TFJ builtin. It keeps the ordinary CodeStubAssembler builder, original
builtin ID, name and argc. The alternate generator emits `Unreachable`, which
contains an intentional debug-break instruction followed by an unreachable
terminator. Each builtin goes through the ordinary placeholder replacement and
embedded-code generation path; none aliases the `Illegal` code object. A kind
check rejects upgrades that move a selected builtin away from TFJ linkage.

Parser removal, asm.js removal, shared compiler code, native wrappers, builtin
numbering, layouts and registrations are unchanged. Shared implementation helpers,
deopt continuations, C++ builtins and their function-pointer tables remain. This
experiment measures generated bodies only.

## Selection and dependency rationale

The authoritative list is `MOONRUN_OMITTED_JS_BUILTINS` in the patch. The explicit
list and measured per-body sizes are recorded in `omitted-builtins.md`.

- **Array convenience methods (26):** iteration callbacks, searching,
  transformation, copying, sorting and selected mutations. Their callable
  entry points implement JS Array methods, rather than allocating native Wasm
  arrays or implementing GC array instructions. Keep Array constructors,
  iterators, values/keys/entries, push/pop, join/toString, species/conversion
  helpers and all internal continuation bodies.
- **Typed-array convenience methods (17):** iteration callbacks, searching,
  transformation, slicing and sorting. Keep typed-array constructors, element
  access, buffer getters, set/subarray, iterators, joining and shared copy and
  conversion machinery. Wasm linear memory and the C API buffer operations do
  not call these prototype methods.
- **Set algebra methods (7):** union/intersection/difference and relation tests.
  Keep Set construction, add/delete/has, iteration and the shared ordered-hash
  implementation. These methods are not Wasm table operations.

The pinned-source reference audit covers `src` and `third_party/v8/builtins`,
including both Torque names and C++ `Builtin::kName` forms. References fall into
library declarations/implementations, bootstrap registration, JS compiler and
Maglev handling, and debugger side-effect classification. No direct reference
was found in Wasm, C API or native wrapper code. The list does not contain
constructors, iterators, promises, string operations, conversion primitives,
Wasm wrappers, interpreter entries, or linkage/stack-management stubs.

This audit is evidence for this pinned configuration, not a general proof that
arbitrary JS or debugger evaluation can safely use the omitted methods. The
experiment intentionally does not preserve their functionality. Reflective JS
calls cannot be excluded by a textual reference audit alone.

## Reproduce

Use the pinned source checkout after applying the producer's normal patches.
The experiment uses the existing custom toolchain and test configuration:

```sh
git -C rusty_v8 apply ../experiments/moonrun-builtin-bodies/omit-bodies.patch
cp ci/runtime/probe.cc rusty_v8/wee8-runtime-check.cc
cp ci/runtime/gc-bytes.h rusty_v8/gc-bytes.h
cp experiments/moonrun-builtin-bodies/probe.cc rusty_v8/moonrun-builtin-check.cc
```

In the configured native GN output directory, first build
`moonrun-builtin-experiment` with the new argument `false`. Preserve
`custom_wee8/moonrun-builtin-check`, `custom_wee8/obj/v8/libwee8.a`, and the
`custom_wee8/gen/v8/embedded.S` and `snapshot.cc` outputs. Then set the argument
to `true`, regenerate GN and build the same target. This regenerates mksnapshot,
the embedded blob and its matching snapshot together. Do not combine a blob
from one configuration with a snapshot from the other. No ordinary wee8 target
is compiled by this experiment.

```sh
python3 experiments/moonrun-builtin-bodies/check.py \
  --before BEFORE_PROBE --after AFTER_PROBE --output identities.json
```

The comparison runner is POSIX-specific. It checks all 50 registrations and
unique identities, compares builtin IDs and argument metadata, and invokes each
method in a separate process: the control must return normally (including a
caught argument error), while the candidate must reach the identified method
and terminate with SIGTRAP or SIGILL. It also reuses the producer's runtime
probe in all four Wasm tier configurations. Core dumps are disabled for these
intentional trap tests.

Run the existing C API and Rust ownership tests against the candidate archive
using `WEE8_DIR`, `WEE8_ARCHIVE` and `WEE8_HEADER`. Compare the same release
consumer and stripping settings for both archives. Use the embedded blob's
reported code/data sizes and the snapshot's `blob_size`, rather than measuring
the textual `.S`/`.cc` files.

## Moonrun scope

The requested `moonbitlang/moonrun` main was inspected at
`efda7a5737998d5af78bdaafbd69a6966e8405d9`. It depends on rusty_v8 0.102.0,
compiles JS glue, and contains no wee8 adapter. It cannot run against this
already-parser-free profile without unrelated porting work. Validation therefore
uses this producer's wee8 Rust adapter and native Wasm probes. A full execution
of that moonrun-main checkout is **not** claimed. String builtins and stringref
remain unchanged; replacing or disabling them is a separate experiment.

## Measured result

The artifact target is `aarch64-apple-darwin`, against the merged asm.js-cut
baseline. Both archives use the same build configuration apart from the
experiment switch. The release consumer source, linking and stripping settings
are identical. These are artifact measurements, not a full moonrun executable.

| Artifact component | Before (bytes) | After (bytes) | Saved |
| --- | ---: | ---: | ---: |
| Embedded code | 1,741,728 | 1,553,280 | 188,448 (10.82%) |
| Embedded metadata | 88,844 | 85,176 | 3,668 (4.13%) |
| Snapshot payload | 406,456 | 404,478 | 1,978 (0.49%) |
| Static archive | 60,774,168 | 60,571,912 | 202,256 (0.33%) |
| Stripped consumer | 15,640,144 | 15,441,936 | 198,208 (1.27%) |

Validation passed: the disabled experiment produced a byte-identical control
archive; all 50 selected registrations/IDs/argument metadata matched; Code
objects were distinct and did not alias Illegal; all 50 control invocations
returned and all 50 candidate invocations trapped; baseline and candidate each
passed four runtime tier modes; candidate C API and Rust tests passed. Every
unselected builtin retained its embedded body length. The blob and snapshot were
regenerated together, and both isolate and context initialization passed.

No native CI experiment, full moonrun integration, Windows/x64 trap validation,
debug/sanitizer build, or runtime speedup is claimed. The experiment remains off
by default and is not part of production builds or releases.

To repeat the size audit, place `libwee8.a`, `consumer-stripped`, `embedded.S`
and `snapshot.cc` in each measurement directory, then run:

```sh
python3 experiments/moonrun-builtin-bodies/measure.py \
  --before BEFORE_DIRECTORY --after AFTER_DIRECTORY \
  --identities identities.json --output sizes.json
```

`results.json` records sizes, hashes, revisions and validation scope. The result
supports keeping this as a narrow optional body-generation cut. Removing shared
helpers, C++ dispatch targets, or string support would require separate dependency
and behavior checks; this experiment does not establish those cuts as safe.
