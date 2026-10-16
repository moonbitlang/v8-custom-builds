// Copyright 2026 the V8 project authors. All rights reserved.
// Use of this source code is governed by the BSD-style license in V8's LICENSE.
#include <cstdio>
#include <memory>
#include "include/libplatform/libplatform.h"
#include "include/v8.h"

int main() {
  auto platform = v8::platform::NewDefaultPlatform();
  v8::V8::InitializePlatform(platform.get());
  if (!v8::V8::Initialize()) return 1;
  auto allocator = std::unique_ptr<v8::ArrayBuffer::Allocator>(
      v8::ArrayBuffer::Allocator::NewDefaultAllocator());
  v8::Isolate::CreateParams params;
  params.array_buffer_allocator = allocator.get();
  v8::Isolate* isolate = v8::Isolate::New(params);
  int status = 0;
  {
    v8::Isolate::Scope entered(isolate);
    v8::HandleScope handles(isolate);
    auto context = v8::Context::New(isolate);
    v8::Context::Scope entered_context(context);
    // Check real registration removal, independently of the Wasm consumer.
    auto source = v8::String::NewFromUtf8Literal(isolate,
        "typeof RegExp === 'undefined' && "
        "['match','matchAll','search','replace','replaceAll','split'].every("
        "name => typeof String.prototype[name] === 'undefined') && "
        "'abc'.includes('b') && 'abc'.startsWith('a') && 'abc'.endsWith('c') && "
        "['includes','startsWith','endsWith'].every(name => { "
        "try { 'abc'[name]({[Symbol.match]: true}); return false; } "
        "catch (error) { return error instanceof TypeError; } }) && "
        "'abc'.includes({[Symbol.match]: false, toString() { return 'b'; }}) && "
        "new Uint8Array([1,2]).includes(2) && "
        "!new Uint8Array([1,2]).includes(3) && "
        "typeof Uint8Array.prototype.startsWith === 'undefined' && "
        "typeof Uint8Array.prototype.endsWith === 'undefined'");
    v8::Local<v8::Script> script;
    v8::Local<v8::Value> result;
    if (!v8::Script::Compile(context, source).ToLocal(&script) ||
        !script->Run(context).ToLocal(&result) || !result->IsTrue()) status = 2;
    {
      v8::TryCatch caught(isolate);
      auto literal = v8::String::NewFromUtf8Literal(isolate, "/x/");
      if (!v8::Script::Compile(context, literal).IsEmpty() || !caught.HasCaught())
        status = 3;
    }
    {
      v8::TryCatch caught(isolate);
      auto pattern = v8::String::NewFromUtf8Literal(isolate, "x");
      if (!v8::RegExp::New(context, pattern, v8::RegExp::kNone).IsEmpty() ||
          !caught.HasCaught()) status = 4;
    }
  }
  isolate->Dispose();
  v8::V8::Dispose();
  v8::V8::DisposePlatform();
  if (status == 0) std::puts("RegExp registration absent; syntax and API reject");
  return status;
}
