// Native guest coverage for runtime helpers; no JavaScript compilation.
#undef NDEBUG
#include "include/v8.h"
#include "wasm.h"
#include <cassert>
#include <cstdio>
#include <fstream>
#include <iterator>
#include <vector>

struct InterruptState {
  int armed = 0;
  int delivered = 0;
};

int main(int argc, char **argv) {
  assert(argc == 4);
  v8::V8::SetFlagsFromString(argv[1]);
  auto *engine = wasm_engine_new();
  assert(engine);
  for (int path = 2; path < argc; ++path) {
    std::ifstream input(argv[path], std::ios::binary);
    assert(input);
    std::vector<char> data((std::istreambuf_iterator<char>(input)), {});
    wasm_byte_vec_t bytes = {data.size(), data.data()};
    auto *store = wasm_store_new(engine);
    assert(store);
    auto *module = wasm_module_new(store, &bytes);
    assert(module);
    // Both modules exercise their helpers after deserializing compiled code.
    wasm_byte_vec_t serialized;
    wasm_module_serialize(module, &serialized);
    assert(serialized.size);
    wasm_module_delete(module);
    module = wasm_module_deserialize(store, &serialized);
    wasm_byte_vec_delete(&serialized);
    assert(module);
    InterruptState interrupts;
    auto *type = wasm_functype_new_0_0();
    auto *arm = wasm_func_new_with_env(
        store, type,
        [](void *data, const wasm_val_vec_t *,
           wasm_val_vec_t *) -> wasm_trap_t * {
          auto *state = static_cast<InterruptState *>(data);
          ++state->armed;
          auto *isolate = v8::Isolate::GetCurrent();
          assert(isolate);
          // The GC guest holds a struct in a Wasm global across this callback.
          isolate->LowMemoryNotification();
          isolate->RequestInterrupt(
              [](v8::Isolate *, void *data) {
                ++static_cast<InterruptState *>(data)->delivered;
              },
              state);
          return nullptr;
        },
        &interrupts, nullptr);
    wasm_functype_delete(type);
    assert(arm);
    wasm_extern_t *entries[] = {wasm_func_as_extern(arm)};
    wasm_extern_vec_t imports = {1, entries};
    wasm_trap_t *trap = nullptr;
    auto *instance = wasm_instance_new(store, module, &imports, &trap);
    assert(instance && !trap);
    wasm_extern_vec_t exports;
    wasm_instance_exports(instance, &exports);
    assert(exports.size == 2);
    auto *run = wasm_extern_as_func(exports.data[0]);
    auto *fail = wasm_extern_as_func(exports.data[1]);
    assert(run && fail);
    wasm_val_t count = WASM_I32_VAL(200000);
    wasm_val_t result = WASM_I32_VAL(0);
    wasm_val_vec_t arguments = {1, &count};
    wasm_val_vec_t results = {1, &result};
    trap = wasm_func_call(run, &arguments, &results);
    if (trap) {
      wasm_message_t message;
      wasm_trap_message(trap, &message);
      std::fprintf(stderr, "Unexpected trap: %.*s\n",
                   static_cast<int>(message.size), message.data);
      wasm_byte_vec_delete(&message);
    }
    assert(!trap && result.of.i32 == 42);
    assert(interrupts.armed == 1 && interrupts.delivered == 1);
    wasm_val_vec_t empty = {0, nullptr};
    trap = wasm_func_call(fail, &empty, &empty);
    assert(trap);
    wasm_message_t message;
    wasm_trap_message(trap, &message);
    assert(message.size > 1);
    wasm_byte_vec_delete(&message);
    wasm_trap_delete(trap);
    wasm_extern_vec_delete(&exports);
    wasm_instance_delete(instance);
    wasm_func_delete(arm);
    wasm_module_delete(module);
    wasm_store_delete(store);
    std::printf("runtime boundary guest %d passed\n", path - 1);
  }
  wasm_engine_delete(engine);
}
