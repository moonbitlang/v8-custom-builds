// Copyright 2019 the V8 project authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// Maintained wee8 adapter copy; see ../MAINTENANCE.md and ../upstream.json.

#if !V8_ENABLE_WEBASSEMBLY
#error This header should only be included if WebAssembly is enabled.
#endif  // !V8_ENABLE_WEBASSEMBLY

#ifndef V8_WASM_C_API_H_
#define V8_WASM_C_API_H_

#include <memory>
#include <mutex>

#include "include/v8-isolate.h"
#include "include/v8-local-handle.h"
#include "src/common/globals.h"
#include "src/handles/handles.h"
#include "wasm.hh"

namespace v8 {
namespace internal {

class JSWeakMap;

}  // namespace internal
}  // namespace v8

namespace wasm {

// Interrupt requests and disposal share this lock. The handle may outlive the
// store, but does not keep its isolate (or any host state) alive.
struct IsolateControl {
  std::mutex mutex;
  v8::Isolate* isolate = nullptr;
};

class StoreImpl {
 public:
  ~StoreImpl();

  v8::Isolate* isolate() const { return isolate_; }
  i::Isolate* i_isolate() const {
    return reinterpret_cast<i::Isolate*>(isolate_);
  }

  v8::Local<v8::Context> context() const { return context_.Get(isolate_); }

  static StoreImpl* get(i::Isolate* isolate) {
    return static_cast<StoreImpl*>(
        reinterpret_cast<v8::Isolate*>(isolate)->GetData(0));
  }

  void SetHostInfo(i::Handle<i::Object> object, void* info,
                   void (*finalizer)(void*));
  void* GetHostInfo(i::Handle<i::Object> key);

  std::shared_ptr<IsolateControl> isolate_control() const { return control_; }
  i::Handle<i::JSReceiver> TerminationException();
  bool IsTerminationException(i::Tagged<i::Object> exception) const {
    return !termination_exception_.is_null() &&
           *termination_exception_ == exception;
  }

 private:
  friend own<Store> Store::make(Engine*);

  StoreImpl() = default;

  v8::Isolate::CreateParams create_params_;
  v8::Isolate* isolate_ = nullptr;
  v8::Eternal<v8::Context> context_;
  i::Handle<i::JSWeakMap> host_info_map_;
  std::shared_ptr<IsolateControl> control_ = std::make_shared<IsolateControl>();
  i::Handle<i::JSReceiver> termination_exception_;
};

}  // namespace wasm

#endif  // V8_WASM_C_API_H_
