# Native profile checks

These checks link only `custom_wee8`. They do not execute JavaScript source or
require JS global constructors. The C API and Rust tests continue to validate the
packaged archive and matching header, including the native exception extension.

| Check | Boundary it verifies |
| --- | --- |
| `ci/runtime/probe.cc` | Snapshot initialization, an empty global object, script/module/function/streaming rejection, asm.js rejection and RegExp API rejection |
| Compiled catalogs vs `*-inventory.tsv` | Every CPP/TFJ selection and descriptor signature; canonical runtime arguments/results; new entries require review on version bumps |
| `cpp-entry-contract.cc` | All CPP metadata entries and external-reference slots select the same distinct addresses |
| `runtime-entry-contract.cc` | Canonical runtime table entries, argument/result counts, 19 inline aliases, distinct addresses and two pair-return signatures |
| `generated-builtin-contract.cc` | All deserialized builtin identities; CPP/TFJ code kind and TFJ argument count; every omitted TFJ body and CPP adaptor traps |
| `runtime-boundaries` guest | Table grow/fill/copy/init, indirect calls, bulk memory, guest throw/catch, GC roots across forced native collection, allocation pressure, interrupts and serialized modules |
| `runtime-shared-memory` guest | Guest-owned shared memory, atomic wait32/wait64 with zero timeout, notify and growth |

`check.py` executes the compiled probes from `target/native-checks/<target>`.
Unsupported calls run in separate processes, must identify the requested entry,
and must terminate at the intentional trap. A null dereference, ordinary return,
or unrelated failure is not accepted. Runtime guests run in Liftoff-only,
optimizing-only, lazy and synchronous-tier-up modes.

The checked-in Wasm binaries have matching WAT sources. The existing C API test
harness reassembles those sources with its pinned `wat` dependency and compares
the bytes, so changes cannot silently leave the readable fixture out of sync.

The inventories intentionally exclude machine-code sizes and architecture-local
builtin IDs. IDs are checked against the target's enum and deserialized Code
objects; sizes change with instruction selection and constant-pool layout.
Other generated builtin kinds retain upstream bodies and do not use these
selection lists.

Moonrun consumer validation uses `crates/moonrun` in `moonbitlang/moon`, with its
wee8 feature and the selected native archive. Default rusty_v8 tests are not
candidate validation. A consumer wrapper upgrade can change validation rules or
diagnostics independently of this profile; compare the same wrapper against the
control archive before attributing such differences to the native cut.
