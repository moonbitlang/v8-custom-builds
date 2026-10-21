// Trace capture and rethrow must work through the packaged C ABI alone.
#undef NDEBUG
#include "wasm.h"

struct TraceState {
  wasm_store_t* store;
  wasm_tag_t* tag;
  wasm_trap_t* original = nullptr;
};

static wasm_trap_t* raise(void* data, const wasm_val_vec_t*, wasm_val_vec_t*) {
  auto* state = static_cast<TraceState*>(data);
  wasm_val_vec_t empty = {0, nullptr};
  auto* exception = wee8_exception_new(state->store, state->tag, &empty);
  assert(exception && !state->original);
  state->original = wasm_trap_copy(exception);
  return exception;
}

extern "C" void wee8_exception_trace_probe(wasm_store_t* store,
                                           const uint8_t* bytes, size_t len) {
  wasm_byte_vec_t binary = {
      len, reinterpret_cast<char*>(const_cast<uint8_t*>(bytes))};
  auto* module = wasm_module_new(store, &binary);
  assert(module);
  auto* tag_type = wasm_tagtype_new(wasm_functype_new_0_0());
  TraceState state{store, wasm_tag_new(store, tag_type)};
  wasm_tagtype_delete(tag_type);
  assert(state.tag);

  wasm_val_vec_t empty = {0, nullptr};
  auto* outside = wee8_exception_new(store, state.tag, &empty);
  assert(outside && !wasm_trap_origin(outside));
  wasm_frame_vec_t frames;
  wasm_trap_trace(outside, &frames);
  assert(frames.size == 0);
  wasm_frame_vec_delete(&frames);
  wasm_trap_delete(outside);

  auto* type = wasm_functype_new_0_0();
  auto* host = wasm_func_new_with_env(store, type, raise, &state, nullptr);
  wasm_functype_delete(type);
  wasm_extern_t* entries[] = {wasm_func_as_extern(host)};
  wasm_extern_vec_t imports = {1, entries};
  wasm_trap_t* trap = nullptr;
  auto* instance = wasm_instance_new(store, module, &imports, &trap);
  assert(instance && !trap);
  wasm_extern_vec_t exports;
  wasm_instance_exports(instance, &exports);
  assert(exports.size == 1);
  trap = wasm_func_call(wasm_extern_as_func(exports.data[0]), &empty, &empty);
  assert(trap && state.original && wee8_exception_is(trap, state.tag));
  wasm_frame_vec_t original;
  wasm_trap_trace(trap, &frames);
  wasm_trap_trace(state.original, &original);
  assert(frames.size >= 3 && frames.size == original.size);
  assert(wasm_frame_func_index(frames.data[0]) == 1);
  for (size_t n = 0; n < frames.size; ++n) {
    assert(wasm_frame_func_index(frames.data[n]) ==
           wasm_frame_func_index(original.data[n]));
    assert(wasm_frame_func_offset(frames.data[n]) ==
           wasm_frame_func_offset(original.data[n]));
    assert(wasm_frame_module_offset(frames.data[n]) ==
           wasm_frame_module_offset(original.data[n]));
    assert(wasm_frame_module_offset(frames.data[n]) > 0);
  }
  auto* origin = wasm_trap_origin(trap);
  assert(origin && wasm_frame_func_index(origin) ==
                       wasm_frame_func_index(frames.data[0]));
  wasm_frame_delete(origin);
  wasm_frame_vec_delete(&frames);
  wasm_frame_vec_delete(&original);
  wasm_trap_delete(trap);
  wasm_trap_delete(state.original);
  wasm_extern_vec_delete(&exports);
  wasm_instance_delete(instance);
  wasm_func_delete(host);
  wasm_tag_delete(state.tag);
  wasm_module_delete(module);
}
