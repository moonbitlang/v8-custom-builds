# Isolated wee8 profile without JavaScript RegExp

This optional source profile removes the RegExp implementation at its registration
and call-site boundaries. It is intended for Wasm C API consumers. It deliberately
changes JavaScript semantics and must not produce a general-purpose rusty_v8
artifact. Moonrun's rusty_v8 backend should continue using upstream Deno artifacts.

The profile is separate from `patches/` and the default `build.sh`. Applying it is
an explicit choice for a dedicated source checkout and GN output directory. This
keeps the feature cut reviewable before changing release packaging.

## Boundary

- `src/init/bootstrapper.cc` no longer installs `RegExp`, or String's `match`,
  `matchAll`, `search`, `replace`, `replaceAll`, and `split` methods.
- Builtin IDs, runtime IDs, external references, Torque inputs and native engine
  sources are removed together. A fresh builtin image and snapshot are required;
  IDs must not be mixed with artifacts from the ordinary build.
- The JavaScript parser rejects regexp literals. The existing `JSRegExp::New`
  boundary reports an unsupported-feature syntax error before accessing the
  missing constructor context slot. This also covers the public C++ API.
- Compiler reductions and the `JSRegExpTest` operator are removed, along with
  JavaScript value-serializer regexp dispatch and RegExp-only debug/test entries.
  Wasm module serialization remains separate.
- `includes`, `startsWith` and `endsWith` retain their actual `IsRegExp` protocol
  check, extracted into `regexp-predicate.tq`; they do not need a matching engine.

There are no replacement parser, compiler, matcher or interpreter definitions,
trap aliases, or placeholder builtin IDs. This is not the earlier engine shim.
Existing RegExp object layouts, GC metadata, stack/cache lifecycle and flag
formatting remain to avoid widening the change into heap-schema surgery. Those
are real retained implementations, not fake engine entry points. Some unused
internal declarations also remain in upstream headers.

Wasm compilation tiers, traps, stack guards, interrupts and Liftoff debug support
are outside this cut. Retaining them leaves room for a future Wasm debugger and
execution controls; this profile does not implement either debugging or fuel.

## Apply and build

Use the producer's pinned rusty_v8 checkout and its ordinary prerequisite source
patches. In a **dedicated wee8 checkout**, apply:

```sh
git -C rusty_v8 apply ../experiments/regexp-clean/wee8-no-regexp.patch
```

Generate a fresh GN output directory using the producer's non-ICU configuration,
and build only the `wee8` target. Do not reuse a preexisting snapshot, generated
Torque files, builtin tables or compiled objects. Do not run the combined default
release script against this source tree: it also packages rusty_v8.

The recipe CI checks that this optional patch applies after the ordinary patches.
It does not establish native correctness across the release target matrix.

## Checks

`test.py BASELINE_GN CANDIDATE_GN OUTPUT --compiler CXX --nm NM` compares two
completed independent native builds. Each output directory must have its source
checkout in the sibling `rusty_v8` directory. It runs the maintained C API and
Rust wrapper checks, an additional numeric/serialization consumer, and a native
registration/rejection probe. It records artifact sizes, hashes, exit codes,
link maps and selected engine/builtin symbol counts. Its linker-map and stripping
commands currently target Mach-O; port that measurement harness before using it
on other object formats.

`tiers.py OUTPUT CANDIDATE_SOURCE` repeats the maintained C API smoke body under
Liftoff, optimizing, lazy-compilation and synchronous tier-up settings for both
archives. The numeric consumer additionally exercises all four numeric types,
start callbacks, independent stores, serialization, memory growth and recovery
after a trap. All checks select explicit native archives instead of downloading
the default release.

These probes are not full Wasm conformance or a Moonrun integration suite.
Cross-platform builds and debug/sanitizer configurations remain promotion gates.
The profile also does not disable the entire JavaScript parser or object runtime.
Temporal, JSON and Date need separate registration/dependency cuts; disabling a
runtime feature flag alone is not proof that their code disappears.
