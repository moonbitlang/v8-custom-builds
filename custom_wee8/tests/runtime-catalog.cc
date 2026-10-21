#include "custom_wee8/src/runtime-selection.h"
#include <cstdio>

int main() {
  using namespace v8::internal;
#define PRINT(Name, Args, Results)                                             \
  std::printf("%s\t%d\t%d\t%s\n", #Name, Args, Results,                        \
              WasmContextKeepsRuntime(Runtime::k##Name) ? "retain" : "omit");
  FOR_EACH_INTRINSIC(PRINT)
#undef PRINT
}
