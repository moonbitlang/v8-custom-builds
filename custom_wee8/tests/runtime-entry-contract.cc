#include "include/libplatform/libplatform.h"
#include "include/v8.h"
#include "src/codegen/external-reference-table.h"
#include "src/execution/isolate.h"
#include "src/runtime/runtime-utils.h"
#include "custom_wee8/src/runtime-selection.h"
#include <cstdio>
#include <cstring>
#include <memory>
#include <set>
#include <type_traits>

int main(int argc, char **argv) {
  using namespace v8::internal;
  auto platform = v8::platform::NewDefaultPlatform();
  v8::V8::InitializePlatform(platform.get());
  CHECK(v8::V8::Initialize());
  auto allocator = std::unique_ptr<v8::ArrayBuffer::Allocator>(
      v8::ArrayBuffer::Allocator::NewDefaultAllocator());
  v8::Isolate::CreateParams params;
  params.array_buffer_allocator = allocator.get();
  auto *isolate = v8::Isolate::New(params);
  {
    v8::Isolate::Scope entered(isolate);
    v8::HandleScope handles(isolate);
    auto context = v8::Context::New(isolate);
    CHECK(!context.IsEmpty());
    v8::Context::Scope entered_context(context);
    auto *internal = reinterpret_cast<Isolate *>(isolate);
    auto *references = internal->external_reference_table();
    constexpr int first =
        ExternalReferenceTable::kSpecialReferenceCount +
        ExternalReferenceTable::kExternalReferenceCountIsolateIndependent +
        ExternalReferenceTable::kBuiltinsReferenceCount;
    struct Entry {
      Runtime::FunctionId id;
      const char *name;
      int args;
      int results;
      Address unsupported;
    };
#define ENTRY(Name, Args, Results)                                             \
  {Runtime::k##Name, #Name, Args, Results,                                     \
   WasmContextKeepsRuntime(Runtime::k##Name)                                   \
       ? 0                                                                     \
       : FUNCTION_ADDR(                                                        \
             (WasmContextUnsupportedRuntime<                                   \
                 Runtime::k##Name,                                             \
                 std::conditional_t<Results == 1, Address, ObjectPair>>))},
    const Entry entries[] = {FOR_EACH_INTRINSIC(ENTRY)};
#undef ENTRY
    std::set<Address> addresses;
    int retained = 0;
    int omitted = 0;
    int pair_entries = 0;
    for (const Entry &expected : entries) {
      const auto *function = Runtime::FunctionForId(expected.id);
      CHECK_EQ(function->function_id, expected.id);
      CHECK_EQ(function->intrinsic_type, Runtime::RUNTIME);
      CHECK_EQ(std::strcmp(function->name, expected.name), 0);
      CHECK_EQ(function->nargs, expected.args);
      CHECK_EQ(function->result_size, expected.results);
      CHECK_NE(function->entry, kNullAddress);
      CHECK(addresses.insert(function->entry).second);
      int index = first + retained + omitted;
      CHECK_EQ(references->address(index), function->entry);
      CHECK_EQ(std::strncmp(references->name(index), "Runtime::", 9), 0);
      CHECK_EQ(std::strcmp(references->name(index) + 9, expected.name), 0);
      pair_entries += expected.results == 2;
      if (expected.unsupported == kNullAddress) {
        ++retained;
      } else {
        ++omitted;
        CHECK_EQ(function->entry, expected.unsupported);
        if (argc == 2 && std::strcmp(argv[1], expected.name) == 0) {
          if (expected.results == 2) {
            auto call =
                reinterpret_cast<ObjectPair (*)(int, Address *, Isolate *)>(
                    function->entry);
            call(0, nullptr, internal);
          } else {
            auto call =
                reinterpret_cast<Address (*)(int, Address *, Isolate *)>(
                    function->entry);
            call(0, nullptr, internal);
          }
          FATAL("Unsupported runtime returned");
        }
      }
    }
    int aliases = 0;
#define CHECK_ALIAS(Name, Args, Results)                                       \
  {                                                                            \
    const auto *alias = Runtime::FunctionForId(Runtime::kInline##Name);        \
    CHECK_EQ(alias->function_id, Runtime::kInline##Name);                      \
    CHECK_EQ(alias->intrinsic_type, Runtime::INLINE);                          \
    CHECK_EQ(std::strcmp(alias->name, "_" #Name), 0);                          \
    CHECK_EQ(alias->entry, Runtime::FunctionForId(Runtime::k##Name)->entry);   \
    CHECK_EQ(alias->nargs, Args);                                              \
    CHECK_EQ(alias->result_size, Results);                                     \
    ++aliases;                                                                 \
  }
    FOR_EACH_INLINE_INTRINSIC(CHECK_ALIAS)
#undef CHECK_ALIAS
    CHECK_EQ(retained + omitted + aliases, Runtime::kNumFunctions);
    CHECK_EQ(argc, 1);
    std::printf(
        "runtime_entries=%zu retained=%d omitted=%d aliases=%d pairs=%d\n",
        addresses.size(), retained, omitted, aliases, pair_entries);
  }
  isolate->Dispose();
  v8::V8::Dispose();
  v8::V8::DisposePlatform();
}
