// Expand the target's complete builtin catalog, including descriptor arguments.
#include <cstdio>
#include "cpp-builtin-selection.h"
#include "generated-js-builtin-selection.h"

int main() {
  using namespace v8::internal;
#define PRINT(Kind, Name, Selection, ...) \
  std::printf("%s\t%s\t%d\t%s\t%s\n", #Kind, #Name, \
              static_cast<int>(Builtin::k##Name), Selection, \
              sizeof(#__VA_ARGS__) > 1 ? #__VA_ARGS__ : "-");
#define CPP(Name, ...) \
  PRINT(CPP, Name, WasmContextKeepsCppBuiltin(Builtin::k##Name) ? "retain" : "omit", __VA_ARGS__)
#define TFJ(Name, ...) \
  PRINT(TFJ, Name, WasmContextKeepsJsBuiltin(Builtin::k##Name) ? "retain" : "omit", __VA_ARGS__)
#define TSJ(Name, ...) PRINT(TSJ, Name, "unchanged", __VA_ARGS__)
#define TSC(Name, ...) PRINT(TSC, Name, "unchanged", __VA_ARGS__)
#define TFC(Name, ...) PRINT(TFC, Name, "unchanged", __VA_ARGS__)
#define TFS(Name, ...) PRINT(TFS, Name, "unchanged", __VA_ARGS__)
#define TFH(Name, ...) PRINT(TFH, Name, "unchanged", __VA_ARGS__)
#define BCH(Name, ...) PRINT(BCH, Name, "unchanged", __VA_ARGS__)
#define ASM(Name, ...) PRINT(ASM, Name, "unchanged", __VA_ARGS__)
  BUILTIN_LIST(CPP, TSJ, TFJ, TSC, TFC, TFS, TFH, BCH, ASM)
}
