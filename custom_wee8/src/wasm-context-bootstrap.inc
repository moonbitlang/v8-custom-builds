// Native Wasm context for V8 12.9.202.13. Included inside bootstrapper.cc's
// namespace, after the Genesis helpers. This is an allocation allowlist, not a
// filter over InitializeGlobal: new upstream JS libraries cannot enter this
// path by default.
//
// Keep the existing NativeContext layout. Unallocated optional slots remain
// undefined; accessing them is unsupported. The selection headers handle
// builtin/runtime entry bodies separately. This initializer also owns the
// private Wasm allocation maps; it does not install the JavaScript Wasm API.

namespace {

// Reuse Genesis's function/map factory, but provide no callable JavaScript
// constructor or prototype methods. V8's Wasm factories use the initial map.
Handle<JSFunction>
CreateWasmAllocationConstructor(Isolate *isolate, const char *name,
                                const char *tag_name, InstanceType type,
                                int size, int in_object_properties = 0) {
  auto *factory = isolate->factory();
  Handle<JSFunction> constructor =
      CreateFunction(isolate, name, type, size, in_object_properties,
                     factory->the_hole_value(), Builtin::kIllegal, 1, false);
  Handle<JSObject> prototype(Cast<JSObject>(constructor->instance_prototype()),
                             isolate);
  JSObject::AddProperty(isolate, prototype, factory->to_string_tag_symbol(),
                        factory->InternalizeUtf8String(tag_name),
                        static_cast<PropertyAttributes>(DONT_ENUM | READ_ONLY));
  return constructor;
}

constexpr wasm::ValueType kWasmContextJsTagParams[] = {wasm::kWasmExternRef};
constexpr wasm::FunctionSig kWasmContextJsTagSignature{
    0, arraysize(kWasmContextJsTagParams), kWasmContextJsTagParams};

} // namespace

void Genesis::InitializeWasmContext(Handle<JSGlobalObject> global_object,
                                    Handle<JSFunction> empty_function) {
  Factory *factory = isolate()->factory();
  native_context()->set_extension(*global_object);
  native_context()->set_security_token(*global_object);

  // ContextSerializer resets this state unconditionally. Keep the state until
  // that serializer contract is split; this does not create Math or its
  // methods.
  MathRandom::InitializeContext(isolate(), native_context());

  // Function calls, stack inspection, and API templates still share V8's
  // context and function machinery. No script binding or JS library is added.
  Handle<Map> meta_map(native_context()->meta_map(), isolate());
  for (auto [type, index] :
       {std::pair{FUNCTION_CONTEXT_TYPE, Context::FUNCTION_CONTEXT_MAP_INDEX},
        std::pair{CATCH_CONTEXT_TYPE, Context::CATCH_CONTEXT_MAP_INDEX},
        std::pair{DEBUG_EVALUATE_CONTEXT_TYPE,
                  Context::DEBUG_EVALUATE_CONTEXT_MAP_INDEX}}) {
    DirectHandle<Map> map =
        factory->NewMapWithMetaMap(meta_map, type, kVariableSizeSentinel);
    map->set_native_context(*native_context());
    native_context()->set(index, *map, UPDATE_WRITE_BARRIER, kReleaseStore);
  }
  native_context()->set_script_context_table(*factory->NewScriptContextTable());
  native_context()->set_embedder_data(*factory->NewEmbedderDataArray(0));

  // Private allocation constructors: these own maps used by V8 factories.
  // They are not installed as global properties.
  Handle<JSFunction> function = CreateFunction(
      isolate(), "Function", JS_FUNCTION_TYPE, JSFunction::kSizeWithPrototype,
      0, empty_function, Builtin::kIllegal, 0, false);
  function->set_prototype_or_initial_map(*isolate()->sloppy_function_map(),
                                         kReleaseStore);
  native_context()->set_function_function(*function);
  native_context()->set_function_prototype(*empty_function);
  for (auto map : {isolate()->sloppy_function_map(),
                   isolate()->sloppy_function_with_name_map(),
                   isolate()->sloppy_function_with_readonly_prototype_map(),
                   isolate()->sloppy_function_without_prototype_map(),
                   isolate()->strict_function_map(),
                   isolate()->strict_function_with_name_map(),
                   isolate()->strict_function_with_readonly_prototype_map(),
                   isolate()->strict_function_without_prototype_map(),
                   isolate()->class_function_map()}) {
    map->SetConstructor(*function);
  }

  // Internal arrays hold export results and captured stack frames. Their maps
  // and length descriptor are required; Array.prototype methods are not.
  Handle<JSFunction> array = CreateFunction(
      isolate(), "Array", JS_ARRAY_TYPE, JSArray::kHeaderSize, 0,
      isolate()->initial_object_prototype(), Builtin::kIllegal, 0, false);
  Handle<Map> array_map(array->initial_map(), isolate());
  Map::EnsureDescriptorSlack(isolate(), array_map, 1);
  Descriptor length = Descriptor::AccessorConstant(
      factory->length_string(), factory->array_length_accessor(),
      static_cast<PropertyAttributes>(DONT_ENUM | DONT_DELETE));
  array_map->AppendDescriptor(isolate(), &length);
  native_context()->set_array_function(*array);
  InitializeJSArrayMaps(isolate(), native_context(), array_map);
  Handle<JSArray> array_prototype =
      factory->NewJSArray(0, TERMINAL_FAST_ELEMENTS_KIND, AllocationType::kOld);
  JSFunction::SetPrototype(array, array_prototype);
  native_context()->set_initial_array_prototype(*array_prototype);
  InitializeJSArrayMaps(isolate(), native_context(),
                        handle(array->initial_map(), isolate()));

  // Wasm memory owns JSArrayBuffer objects even without the JS memory API.
  // Avoid CreateArrayBuffer(), which also installs the JS methods.
  for (auto [name, index] :
       {std::pair{factory->ArrayBuffer_string(),
                  Context::ARRAY_BUFFER_FUN_INDEX},
        std::pair{factory->SharedArrayBuffer_string(),
                  Context::SHARED_ARRAY_BUFFER_FUN_INDEX}}) {
    Handle<JSFunction> constructor =
        CreateFunction(isolate(), name, JS_ARRAY_BUFFER_TYPE,
                       JSArrayBuffer::kSizeWithEmbedderFields, 0,
                       factory->the_hole_value(), Builtin::kIllegal, 0, false);
    native_context()->set(index, *constructor, UPDATE_WRITE_BARRIER,
                          kReleaseStore);
  }

  // The existing C API uses a WeakMap for host-info ownership/finalization.
  // Keep only its allocation map. A custom adapter can replace this separately.
  Handle<JSFunction> weakmap = CreateFunction(
      isolate(), "WeakMap", JS_WEAK_MAP_TYPE, JSWeakMap::kHeaderSize, 0,
      factory->the_hole_value(), Builtin::kIllegal, 0, false);
  native_context()->set_js_weak_map_fun(*weakmap);

  // Runtime traps and compilation/linking diagnostics reuse JSError layout and
  // stack accessors. Install into a private holder, never the global object.
  Handle<JSObject> errors = factory->NewJSObject(isolate()->object_function());
  InstallError(isolate(), errors, factory->Error_string(),
               Context::ERROR_FUNCTION_INDEX);
  InstallError(isolate(), errors, factory->TypeError_string(),
               Context::TYPE_ERROR_FUNCTION_INDEX);
  InstallError(isolate(), errors, factory->RangeError_string(),
               Context::RANGE_ERROR_FUNCTION_INDEX);
  // The existing public Script API must still reject disabled text compilation
  // cleanly. This is an error object, not a parser or evaluator.
  InstallError(isolate(), errors, factory->SyntaxError_string(),
               Context::SYNTAX_ERROR_FUNCTION_INDEX);
  InstallError(isolate(), errors, factory->CompileError_string(),
               Context::WASM_COMPILE_ERROR_FUNCTION_INDEX);
  InstallError(isolate(), errors, factory->LinkError_string(),
               Context::WASM_LINK_ERROR_FUNCTION_INDEX);
  InstallError(isolate(), errors, factory->RuntimeError_string(),
               Context::WASM_RUNTIME_ERROR_FUNCTION_INDEX);
  InitializeCallSiteBuiltins();

  // Factory and API-template caches are part of native object allocation.
  native_context()->set_fast_template_instantiations_cache(
      *factory->NewFixedArrayWithHoles(
          TemplateInfo::kFastTemplateInstantiationsCacheSize));
  native_context()->set_slow_template_instantiations_cache(
      *SimpleNumberDictionary::New(isolate(),
                                   ApiNatives::kInitialFunctionCacheSize));
  native_context()->set_wasm_debug_maps(*factory->empty_fixed_array());
  Handle<JSObject> object_prototype = isolate()->initial_object_prototype();
  native_context()->set_object_function_prototype(*object_prototype);
  native_context()->set_object_function_prototype_map(object_prototype->map());
  native_context()->set_extras_binding_object(
      *factory->NewJSObjectWithNullProto());

  // The C adapter allocates these objects through V8's ordinary Wasm
  // factories. Keep their layouts and names, without constructing a private
  // WebAssembly namespace or retaining JavaScript methods on their prototypes.
  native_context()->set_wasm_module_constructor(*CreateWasmAllocationConstructor(
      isolate(), "Module", "WebAssembly.Module", WASM_MODULE_OBJECT_TYPE,
      WasmModuleObject::kHeaderSize));
  native_context()->set_wasm_instance_constructor(
      *CreateWasmAllocationConstructor(
          isolate(), "Instance", "WebAssembly.Instance",
          WASM_INSTANCE_OBJECT_TYPE, WasmInstanceObject::kHeaderSize));
  native_context()->set_wasm_table_constructor(*CreateWasmAllocationConstructor(
      isolate(), "Table", "WebAssembly.Table", WASM_TABLE_OBJECT_TYPE,
      WasmTableObject::kHeaderSize));
  native_context()->set_wasm_memory_constructor(
      *CreateWasmAllocationConstructor(
          isolate(), "Memory", "WebAssembly.Memory", WASM_MEMORY_OBJECT_TYPE,
          WasmMemoryObject::kHeaderSize));
  native_context()->set_wasm_global_constructor(
      *CreateWasmAllocationConstructor(
          isolate(), "Global", "WebAssembly.Global", WASM_GLOBAL_OBJECT_TYPE,
          WasmGlobalObject::kHeaderSize));
  native_context()->set_wasm_tag_constructor(*CreateWasmAllocationConstructor(
      isolate(), "Tag", "WebAssembly.Tag", WASM_TAG_OBJECT_TYPE,
      WasmTagObject::kHeaderSize));
  native_context()->set_wasm_exported_function_map(
      *isolate()->sloppy_function_without_prototype_map());

  // The exception package stores its tag and payload in named in-object fields.
  // Preserve the upstream WasmJs::PrepareForSnapshot layout used by the native
  // exception implementation, without its JavaScript constructor/getArg/is.
  Handle<JSFunction> exception = CreateWasmAllocationConstructor(
      isolate(), "Exception", "WebAssembly.Exception",
      WASM_EXCEPTION_PACKAGE_TYPE, WasmExceptionPackage::kSize,
      WasmExceptionPackage::kInObjectFieldCount);
  native_context()->set_wasm_exception_constructor(*exception);
  DirectHandle<Map> exception_map(exception->initial_map(), isolate());
  Map::EnsureDescriptorSlack(isolate(), exception_map, 2);
  Descriptor tag = Descriptor::DataField(
      isolate(), factory->wasm_exception_tag_symbol(),
      WasmExceptionPackage::kTagIndex, DONT_ENUM, Representation::Tagged());
  exception_map->AppendDescriptor(isolate(), &tag);
  Descriptor values = Descriptor::DataField(
      isolate(), factory->wasm_exception_values_symbol(),
      WasmExceptionPackage::kValuesIndex, DONT_ENUM, Representation::Tagged());
  exception_map->AppendDescriptor(isolate(), &values);

  // Wasm exception handling recognizes V8-originated exceptions through JSTag.
  // Its canonical index belongs to the running engine and is refreshed after
  // snapshot restoration, as in the upstream context initialization.
  auto js_tag = WasmExceptionTag::New(isolate(), 0);
  native_context()->set_wasm_js_tag(
      *WasmTagObject::New(isolate(), &kWasmContextJsTagSignature, 0, js_tag,
                          Handle<WasmTrustedInstanceData>()));
}

void Genesis::InitializeWasmContextAfterSnapshot(
    Isolate *isolate, DirectHandle<NativeContext> native_context) {
  if (native_context->is_wasm_js_installed() != Smi::zero())
    return;
  DirectHandle<WasmTagObject> js_tag(
      Cast<WasmTagObject>(native_context->wasm_js_tag()), isolate);
  js_tag->set_canonical_type_index(
      wasm::GetWasmEngine()->type_canonicalizer()->AddRecursiveGroup(
          &kWasmContextJsTagSignature));
  native_context->set_is_wasm_js_installed(Smi::FromInt(1));
}
