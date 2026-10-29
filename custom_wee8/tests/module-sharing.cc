// Sharing must retain V8's native module and its lazy compilation state.
#include "include/v8.h"
#include "src/base/logging.h"
#include "src/execution/frames-inl.h"
#include "src/init/v8.h"
#include "src/wasm/wasm-code-manager.h"
#include "wasm.h"

#include <cstdio>
#include <fstream>
#include <iterator>
#include <memory>
#include <string>
#include <vector>

struct Observation {
  v8::internal::wasm::NativeModule* native = nullptr;
  std::weak_ptr<v8::TaskRunner> runner;
  bool require_liftoff = true;
};

static wasm_trap_t* Observe(void* env, const wasm_val_vec_t*, wasm_val_vec_t*) {
  namespace i = v8::internal;
  auto& observation = *static_cast<Observation*>(env);
  auto* isolate = v8::Isolate::GetCurrent();
  observation.runner =
      i::V8::GetCurrentPlatform()->GetForegroundTaskRunner(isolate);
  i::wasm::WasmCodeRefScope code_scope;
  for (i::StackFrameIterator frames(reinterpret_cast<i::Isolate*>(isolate));
       !frames.done(); frames.Advance()) {
    if (!frames.frame()->is_wasm()) continue;
    auto* native = i::WasmFrame::cast(frames.frame())->native_module();
    if (observation.native) CHECK_EQ(native, observation.native);
    observation.native = native;
    // The called function is baseline compiled. The second, uncalled function
    // must stay lazy, including after share/obtain and source-store teardown.
    if (observation.require_liftoff) {
      CHECK_EQ(native->GetCode(1)->tier(), i::wasm::ExecutionTier::kLiftoff);
    }
    CHECK(!native->HasCode(2));
    return nullptr;
  }
  FATAL("No guest frame in the actual C API host callback");
}

static void Run(wasm_store_t* store, wasm_module_t* module, Observation& observed,
                int calls = 1) {
  auto* type = wasm_functype_new_0_0();
  auto* host = wasm_func_new_with_env(store, type, Observe, &observed, nullptr);
  wasm_functype_delete(type);
  wasm_extern_t* entries[] = {wasm_func_as_extern(host)};
  wasm_extern_vec_t imports = {1, entries};
  wasm_trap_t* trap = nullptr;
  auto* instance = wasm_instance_new(store, module, &imports, &trap);
  CHECK_NOT_NULL(instance);
  CHECK_NULL(trap);
  wasm_extern_vec_t exports;
  wasm_instance_exports(instance, &exports);
  CHECK_EQ(exports.size, 1);
  wasm_val_vec_t args = {0, nullptr};
  wasm_val_t value = WASM_I32_VAL(0);
  wasm_val_vec_t results = {1, &value};
  for (int n = 0; n < calls; ++n) {
    CHECK_NULL(wasm_func_call(wasm_extern_as_func(exports.data[0]), &args, &results));
    CHECK_EQ(value.of.i32, 42);
  }
  wasm_extern_vec_delete(&exports);
  wasm_instance_delete(instance);
  wasm_func_delete(host);
}

int main(int argc, char** argv) {
  CHECK(argc == 2 || argc == 3);
  const bool tiering = argc == 3;
  if (tiering) CHECK_EQ(std::string(argv[2]), "tiering");
  // Eliminate background tiering as a source of timing-dependent assertions.
  // The old share implementation forces TurboFan even with this flag.
  v8::V8::SetFlagsFromString(
      tiering ? "--wasm-lazy-compilation --wasm-sync-tier-up --wasm-tiering-budget=1000"
              : "--wasm-lazy-compilation --no-wasm-tier-up");
  std::ifstream input(argv[1], std::ios::binary);
  CHECK(input.good());
  std::vector<char> data((std::istreambuf_iterator<char>(input)), {});
  wasm_byte_vec_t bytes = {data.size(), data.data()};
  auto* engine = wasm_engine_new();
  auto* store = wasm_store_new(engine);
  auto* module = wasm_module_new(store, &bytes);
  CHECK_NOT_NULL(module);
  Observation first;
  Run(store, module, first);
  std::puts("Before sharing: called function is Liftoff; uncalled function is lazy");
  std::fflush(stdout);
  auto* shared = wasm_module_share(module);
  CHECK_NOT_NULL(shared);
  Run(store, module, first);
  wasm_module_delete(module);
  wasm_store_delete(store);
  CHECK(first.runner.expired());

  auto* next = wasm_store_new(engine);
  auto* obtained = wasm_module_obtain(next, shared);
  CHECK_NOT_NULL(obtained);
  Observation second{first.native, {}};
  Run(next, obtained, second);
  // Obtained module objects independently retain the native module.
  wasm_shared_module_delete(shared);
  second.require_liftoff = !tiering;
  Run(next, obtained, second, tiering ? 10000 : 1);
  if (tiering) {
    // Only normal guest calls make the function hot. No explicit compilation
    // API or serialized cache is involved after the source store is gone.
    v8::internal::wasm::WasmCodeRefScope code_scope;
    CHECK_EQ(second.native->GetCode(1)->tier(),
             v8::internal::wasm::ExecutionTier::kTurbofan);
    std::puts("Shared code still tiers up through V8's normal hot-call mechanism");
  }
  wasm_module_delete(obtained);
  wasm_store_delete(next);
  CHECK(second.runner.expired());
  wasm_engine_delete(engine);
  std::puts("Sharing preserves native identity, lazy code and independent store lifetimes");
}
