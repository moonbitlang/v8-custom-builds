// Compile with the target's definitions so conditional builtin lists match.
#include <cstdio>
#include "cpp-builtin-selection.h"

int main() {
  using namespace v8::internal;
#define PRINT(Name, ...) \
  std::printf("%s\t%s\n", #Name, \
              WasmContextKeepsCppBuiltin(Builtin::k##Name) ? "retain" : "omit");
  BUILTIN_LIST_C(PRINT)
#undef PRINT
}
