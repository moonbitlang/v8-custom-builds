// Exercise repeated C API stores while their process-wide engine stays alive.
#include "include/v8.h"
#include "src/base/logging.h"
#include "src/init/v8.h"
#include "src/wasm/canonical-types.h"
#include "src/zone/zone.h"
#include "wasm.h"
#include <cstdio>
#include <fstream>
#include <iterator>
#include <memory>
#include <string>
#include <vector>

// Reach the isolate through a real host call, without casting opaque C handles.
// A weak reference observes whether store teardown releases the platform queue.
static wasm_trap_t *RememberRunner(void *env, const wasm_val_vec_t *,
                                   wasm_val_vec_t *) {
  auto *runner = static_cast<std::weak_ptr<v8::TaskRunner> *>(env);
  auto *isolate = v8::Isolate::GetCurrent();
  CHECK_NOT_NULL(isolate);
  *runner =
      v8::internal::V8::GetCurrentPlatform()->GetForegroundTaskRunner(isolate);
  return nullptr;
}

static std::weak_ptr<v8::TaskRunner>
RunInFreshStore(wasm_engine_t *engine, const wasm_byte_vec_t &serialized) {
  auto *store = wasm_store_new(engine);
  CHECK_NOT_NULL(store);
  auto *module = wasm_module_deserialize(store, &serialized);
  CHECK_NOT_NULL(module);
  std::weak_ptr<v8::TaskRunner> runner;
  auto *type = wasm_functype_new_0_0();
  auto *host =
      wasm_func_new_with_env(store, type, RememberRunner, &runner, nullptr);
  wasm_functype_delete(type);
  CHECK_NOT_NULL(host);
  wasm_extern_t *entries[] = {wasm_func_as_extern(host)};
  wasm_extern_vec_t imports = {1, entries};
  wasm_trap_t *trap = nullptr;
  auto *instance = wasm_instance_new(store, module, &imports, &trap);
  CHECK_NOT_NULL(instance);
  CHECK_NULL(trap);
  wasm_extern_vec_t exports;
  wasm_instance_exports(instance, &exports);
  CHECK_EQ(exports.size, 1);
  auto *run = wasm_extern_as_func(exports.data[0]);
  CHECK_NOT_NULL(run);
  wasm_val_vec_t args = {0, nullptr};
  wasm_val_t value = WASM_I32_VAL(0);
  wasm_val_vec_t results = {1, &value};
  CHECK_NULL(wasm_func_call(run, &args, &results));
  CHECK_EQ(value.of.i32, 42);
  CHECK(!runner.expired());
  wasm_extern_vec_delete(&exports);
  wasm_instance_delete(instance);
  wasm_func_delete(host);
  wasm_module_delete(module);
  wasm_store_delete(store);
  return runner;
}

static void CheckZoneRollback() {
  namespace i = v8::internal;
  i::AccountingAllocator allocator;
  i::Zone zone(&allocator, "store-lifetimes");
  const auto empty = zone.Snapshot();
  auto *retained = zone.New<int>(42);
  const size_t allocation = zone.allocation_size();
  const size_t segments = allocator.GetCurrentMemoryUsage();
  const auto snapshot = zone.Snapshot();
  for (size_t size : {size_t{16}, size_t{1024 * 1024}}) {
    // Cover rollback inside one segment and freeing additional segments.
    auto *temporary = zone.AllocateArray<char>(size);
    temporary[size - 1] = 1;
    snapshot.Restore(&zone);
    CHECK_EQ(zone.allocation_size(), allocation);
    CHECK_EQ(allocator.GetCurrentMemoryUsage(), segments);
    CHECK_EQ(*retained, 42);
  }
  {
    i::ZoneScope scope(&zone);
    zone.AllocateArray<char>(1024 * 1024);
  }
  CHECK_EQ(zone.allocation_size(), allocation);
  CHECK_EQ(allocator.GetCurrentMemoryUsage(), segments);
  empty.Restore(&zone);
  CHECK_EQ(zone.allocation_size(), 0);
  CHECK_EQ(allocator.GetCurrentMemoryUsage(), 0);
}

int main(int argc, char **argv) {
  CHECK_EQ(argc, 3);
  // Synchronous compilation makes resource ownership checks independent of
  // background compilation timing. No event-loop pumping hides teardown bugs.
  v8::V8::SetFlagsFromString("--no-wasm-lazy-compilation --no-wasm-tier-up");
  const std::string check = argv[1];
  CHECK(check == "shutdown" || check == "types");
  std::ifstream input(argv[2], std::ios::binary);
  CHECK(input.good());
  std::vector<char> data((std::istreambuf_iterator<char>(input)), {});
  wasm_byte_vec_t bytes = {data.size(), data.data()};
  auto *engine = wasm_engine_new();
  CHECK_NOT_NULL(engine);
  auto *store = wasm_store_new(engine);
  CHECK_NOT_NULL(store);
  auto *module = wasm_module_new(store, &bytes);
  CHECK_NOT_NULL(module);
  wasm_byte_vec_t serialized;
  wasm_module_serialize(module, &serialized);
  CHECK_GT(serialized.size, 0);
  wasm_module_delete(module);
  wasm_store_delete(store);

  if (check == "shutdown") {
    for (int n = 0; n < 32; ++n) {
      CHECK(RunInFreshStore(engine, serialized).expired());
    }
    // Teardown must also work when no module or callback was ever created.
    wasm_store_delete(wasm_store_new(engine));
    std::puts("Store teardown released foreground task runners");
  } else {
    CheckZoneRollback();
    RunInFreshStore(engine, serialized);
    auto *types = v8::internal::wasm::GetTypeCanonicalizer();
    const size_t count = types->GetCurrentNumberOfTypes();
    const size_t memory = types->EstimateCurrentMemoryConsumption();
    for (int n = 0; n < 256; ++n)
      RunInFreshStore(engine, serialized);
    CHECK_EQ(types->GetCurrentNumberOfTypes(), count);
    // Check the engine's live canonical storage, not allocator/RSS heuristics.
    CHECK_EQ(types->EstimateCurrentMemoryConsumption(), memory);
    std::puts("Repeated deserialization retained no duplicate canonical types");
  }
  wasm_byte_vec_delete(&serialized);
  wasm_engine_delete(engine);
}
