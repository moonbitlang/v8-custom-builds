#include "include/libplatform/libplatform.h"
#include "include/v8.h"
#include <cstdio>
#include <memory>

int main(int argc, char**) {
  auto platform = v8::platform::NewDefaultPlatform();
  v8::V8::InitializePlatform(platform.get());
  if (!v8::V8::Initialize()) return 1;
  auto allocator = std::unique_ptr<v8::ArrayBuffer::Allocator>(
      v8::ArrayBuffer::Allocator::NewDefaultAllocator());
  v8::Isolate::CreateParams params;
  params.array_buffer_allocator = allocator.get();
  auto* isolate = v8::Isolate::New(params);
  {
    v8::Isolate::Scope entered(isolate);
    v8::HandleScope handles(isolate);
    auto context = v8::Context::New(isolate);
    if (context.IsEmpty()) return 2;
    v8::Context::Scope entered_context(context);
    if (argc > 1) {
      // Assert the resulting interface, not the initializer's implementation.
      auto names = context->Global()->GetOwnPropertyNames(
          context, v8::ALL_PROPERTIES, v8::KeyConversionMode::kConvertToString)
          .ToLocalChecked();
      for (uint32_t i = 0; i < names->Length(); ++i) {
        auto name = names->Get(context, i).ToLocalChecked();
        v8::String::Utf8Value text(isolate, name);
        std::fprintf(stderr, "Unexpected global: %s\n", *text);
        return 3;
      }
    }
    v8::TryCatch caught(isolate);
    if (!v8::Script::Compile(
            context, v8::String::NewFromUtf8Literal(isolate, "1 + 1")).IsEmpty()
        || !caught.HasCaught()) return 4;
    caught.Reset();
    isolate->LowMemoryNotification();
    v8::HeapStatistics stats;
    isolate->GetHeapStatistics(&stats);
    std::printf("used_heap_size=%zu total_heap_size=%zu\n",
                stats.used_heap_size(), stats.total_heap_size());
  }
  isolate->Dispose();
  v8::V8::Dispose();
  v8::V8::DisposePlatform();
}
