// Validate the ABI tables and deserialized code identities, not just startup.
#include <cstdio>
#include <cstring>
#include <memory>
#include <set>
#include "include/libplatform/libplatform.h"
#include "include/v8.h"
#include "src/builtins/builtins-inl.h"
#include "src/codegen/external-reference-table.h"
#include "src/execution/isolate.h"
#include "src/objects/code-inl.h"
#include "cpp-builtin-selection.h"

int main(int argc, char** argv) {
  using namespace v8::internal;
  auto platform = v8::platform::NewDefaultPlatform();
  v8::V8::InitializePlatform(platform.get());
  CHECK(v8::V8::Initialize());
  auto allocator = std::unique_ptr<v8::ArrayBuffer::Allocator>(
      v8::ArrayBuffer::Allocator::NewDefaultAllocator());
  v8::Isolate::CreateParams params;
  params.array_buffer_allocator = allocator.get();
  auto* isolate = v8::Isolate::New(params);
  {
    v8::Isolate::Scope entered(isolate);
    v8::HandleScope handles(isolate);
    auto context = v8::Context::New(isolate);
    CHECK(!context.IsEmpty());
    v8::Context::Scope entered_context(context);
    auto* internal = reinterpret_cast<Isolate*>(isolate);
    auto* builtins = internal->builtins();
    auto* references = internal->external_reference_table();
    for (Builtin id = Builtins::kFirst; id <= Builtins::kLast; ++id) {
      CHECK_EQ(builtins->code(id)->builtin_id(), id);
    }
    constexpr int first = ExternalReferenceTable::kSpecialReferenceCount +
                          ExternalReferenceTable::kExternalReferenceCountIsolateIndependent;
    struct Entry {
      Builtin id;
      Address unsupported;
    };
    // Expand only data, keeping the validation loop small even with hundreds
    // of entries. Retained functions must not acquire unsupported addresses.
#define ENTRY(Name, ...) \
    {Builtin::k##Name, WasmContextKeepsCppBuiltin(Builtin::k##Name) ? 0 : \
        FUNCTION_ADDR(WasmContextUnsupportedCppBuiltin<Builtin::k##Name>)},
    const Entry entries[] = {BUILTIN_LIST_C(ENTRY)};
#undef ENTRY
    std::set<Address> addresses;
    int retained = 0;
    int omitted = 0;
    for (const Entry& expected : entries) {
      Builtin id = expected.id;
      CHECK_EQ(Builtins::KindOf(id), Builtins::CPP);
      Address entry = Builtins::CppEntryOf(id);
      CHECK_NE(entry, kNullAddress);
      CHECK(addresses.insert(entry).second);
      int index = first + retained + omitted;
      CHECK_EQ(references->address(index), entry);
      CHECK_EQ(std::strncmp(references->name(index), "Builtin_", 8), 0);
      CHECK_EQ(std::strcmp(references->name(index) + 8, Builtins::name(id)), 0);
      if (expected.unsupported == kNullAddress) {
        ++retained;
      } else {
        ++omitted;
        CHECK_EQ(entry, expected.unsupported);
        if (argc == 2 && std::strcmp(argv[1], Builtins::name(id)) == 0) {
          auto call = reinterpret_cast<Address (*)(int, Address*, Isolate*)>(entry);
          call(0, nullptr, internal);
          FATAL("Unsupported entry returned");
        }
      }
    }
    CHECK_EQ(retained + omitted, ExternalReferenceTable::kBuiltinsReferenceCount);
    CHECK_EQ(argc, 1);  // A requested negative case must terminate above.
    std::printf("identities=%d retained=%d omitted=%d unique_cpp_entries=%zu\n",
                Builtins::kBuiltinCount, retained, omitted, addresses.size());
  }
  isolate->Dispose();
  v8::V8::Dispose();
  v8::V8::DisposePlatform();
}
