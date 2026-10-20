// Inspect all deserialized identities; invoke omitted JS entries without
// installing the libraries or relying on public property registration.
#include "include/libplatform/libplatform.h"
#include "include/v8.h"
#include "src/api/api-inl.h"
#include "src/builtins/builtins-inl.h"
#include "src/execution/isolate.h"
#include "src/heap/factory.h"
#include "src/objects/code-inl.h"
#include "src/objects/js-function-inl.h"
#include "src/objects/shared-function-info-inl.h"
#include "src/wasm-profile/cpp-builtin-selection.h"
#include "src/wasm-profile/generated-js-builtin-selection.h"
#include <cstdio>
#include <cstring>
#include <memory>
#include <set>

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
    std::set<Address> identities;
    for (Builtin id = Builtins::kFirst; id <= Builtins::kLast; ++id) {
      auto code = internal->builtins()->code(id);
      CHECK_EQ(code->builtin_id(), id);
      CHECK(identities.insert(code.ptr()).second);
      bool js = Builtins::KindOf(id) == Builtins::TFJ;
      if (js || Builtins::KindOf(id) == Builtins::CPP) {
        CHECK_EQ(code->kind(), CodeKind::BUILTIN);
      }
      if (js)
        CHECK_EQ(code->parameter_count(), Builtins::GetStackParameterCount(id));
      bool omitted = (js && !WasmContextKeepsJsBuiltin(id)) ||
                     (Builtins::KindOf(id) == Builtins::CPP &&
                      !WasmContextKeepsCppBuiltin(id));
      if (argc == 1) {
        std::printf("%d\t%s\t%d\t%u\t%d\t%d\n", static_cast<int>(id),
                    Builtins::name(id), static_cast<int>(Builtins::KindOf(id)),
                    code->parameter_count(), code->instruction_size(), omitted);
      } else if (std::strcmp(argv[1], Builtins::name(id)) == 0) {
        CHECK(omitted);
        auto name =
            internal->factory()->NewStringFromAsciiChecked(Builtins::name(id));
        auto info =
            internal->factory()->NewSharedFunctionInfoForBuiltin(name, id);
        info->set_language_mode(LanguageMode::kStrict);
        const int parameters = js ? Builtins::GetStackParameterCount(id)
                                  : kDontAdaptArgumentsSentinel;
        if (parameters == kDontAdaptArgumentsSentinel) {
          info->DontAdaptArguments();
        } else {
          info->set_internal_formal_parameter_count(parameters);
        }
        auto function = Factory::JSFunctionBuilder(internal, info,
                                                   internal->native_context())
                            .Build();
        std::fprintf(stderr, "invoking:%s\n", Builtins::name(id));
        std::fflush(stderr);
        auto result = v8::Utils::ToLocal(function)->Call(
            context, v8::Undefined(isolate), 0, nullptr);
        USE(result);
        FATAL("Unsupported generated entry returned");
      }
    }
    CHECK_EQ(argc, 1);
    CHECK_EQ(identities.size(), Builtins::kBuiltinCount);
  }
  isolate->Dispose();
  v8::V8::Dispose();
  v8::V8::DisposePlatform();
}
