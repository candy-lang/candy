Prior experience with Mehl: two stages (LIR and bytecode) between HIR and VM

LIR: refcounting as well as some optimizations
Bytecode: Actual stack-based bytecode

---

VM:
- focus: simple
- stack-based
- all values heap-allocated
- work on flattened bytecode (or bytecode in loadable chunks), but not deal with bodies, lambdas, etc.
- reference-counted

facts:
- we probably don't want reference-counting stuff in the HIR
- we want to be able to do optimizations like inlining, constant folding, etc. without messing up needs and debuggability
  - some things like "this is a value that may be observed in the debugger" or "we're entering a function" should explicitly exist in the IR for optimizations to still work
- ownership analysis for reference-counting really removes a lot of the duplicates/drops
  - to do ownership analysis for reference-counting, we need to be able to shuffle duplicates and drops around, that needs structural programming without jumps
=> we also need two stages (maybe named MIR and LIR?)

proposal: MIR followed by LIR

MIR similar to HIR, but more concepts are explicit:
- ownership (reference counting, closures)
- context (important for needs)
- observability (like "although the program doesn't use this value, the dev might want to look at it")
- no special casing for builtins (instead of relying on "this ID is special", have sentinel values for builtin stuff so that they can be inlined)

enum Mir {
  Assignment { id, expression } where expression is like the HIR expressions or builtin

  // refcount
  Duplicate(Id),
  Drop(Id),

  // context stuff for needs
  EnterNeedsScope(HirId), # we started entering a function
  ExitNeedsScope, # we exited a function
  Needs(Id),

  // info for debugging and tracing
  LocalCalculated(HirId, Id),
  FunctionCallStarted(HirId, closure, parameters),
  FunctionCallEnded(return value),
}

LIR: list of instructions + labels that point to specific parts of the LIR

Instructions:
- Builtin(builtin function)
- CreateInt n: creates int in heap with refcount 1, pushes reference on stack
- CreateText text: similar
- CreateStruct numFields: pops 2*numFields parameters (keys and values), pushes struct
- Create...
- Duplicate n: Increases refcount
- Drop n: Decreases refcount
- Pop: Pops from stack
- PopMultipleBelowTop n: Leaves top-most stack entry untouched, but removes n below
- PushAddress address: Pushes an address into the bytecode
- PushFromStack offset: Pushes existing stack entry again
- Call: Pops target from stack, pushes current address on stack, jumps to target
- Return: Assumes return value is on stack. Pops address below, jumps there
- EnterNeedsScope HirId
- ExitNeedsScope
- Needs id
- ...

---

Example:

```candy
isInt a =
  builtinEquals (builtinTypeOf a) Int

double a =
  needs (isInt a)
  builtinMultiply a 2

main environment =
  builtinPrint (double 3)
  foo # AST error: Unknown foo
```

# HIR

```hir
# Note: IDs 1 – 11 are builtin
HirId(project-file:test.candy:11) = body { # isInt
  HirId(project-file:test.candy:11:0) = lambda { a ->
    HirId(project-file:test.candy:11:0:1) = call HirId(project-file:test.candy:9) with these arguments: # builtinTypeOf
      HirId(project-file:test.candy:11:0:0)
    HirId(project-file:test.candy:11:0:2) = symbol Int
    HirId(project-file:test.candy:11:0:3) = call HirId(project-file:test.candy:10) with these arguments: # builtinEquals
      HirId(project-file:test.candy:11:0:1)
      HirId(project-file:test.candy:11:0:2)
  }
}
HirId(project-file:test.candy:12) = body { # double
  HirId(project-file:test.candy:12:0) = lambda { a ->
    HirId(project-file:test.candy:12:0:1) = call HirId(project-file:test.candy:11) with these arguments: # isInt
      HirId(project-file:test.candy:12:0:0)
    HirId(project-file:test.candy:12:0:2) = needs HirId(project-file:test.candy:12:0:1)
    HirId(project-file:test.candy:12:0:3) = int 2
    HirId(project-file:test.candy:12:0:4) = call HirId(project-file:test.candy:5) with these arguments: # builtinMultiply
      HirId(project-file:test.candy:12:0:0)
      HirId(project-file:test.candy:12:0:3)
  }
}
HirId(project-file:test.candy:13) = body { # main
  HirId(project-file:test.candy:13:0) = lambda { environment ->
    HirId(project-file:test.candy:13:0:1) = int 3
    HirId(project-file:test.candy:13:0:2) = call HirId(project-file:test.candy:12) with these arguments: # double
      HirId(project-file:test.candy:13:0:1)
    HirId(project-file:test.candy:13:0:3) = call HirId(project-file:test.candy:2) with these arguments: # builtinPrint
      HirId(project-file:test.candy:13:0:2)
    HirId(project-file:test.candy:13:0:4) = error
      CompilerError { span: 181..181, payload: Ast(ParenthesizedWithoutClosingParenthesis) }
  }
}
```

# MIR

without debug information (for my sanity)

```mir
# Note: IDs are not actually just numbers, but hierarchical
0 = Builtin::...
1 = Builtin::Print
2 = Builtin::...
3 = Builtin::...
4 = Builtin::Multiply
5 = Builtin::...
6 = Builtin::...
7 = Builtin::...
8 = Builtin::...
9 = Builtin::TypeOf
10 = Builtin::Equals
duplicate 9 10
11 = lambda [9 10] { 11 ->
  enterNeedsScope isInt
  duplicate 11
  12 = call 9 with 11
  13 = symbol Int
  duplicate 12 13
  14 = call 10 with 12 13
  expressionEvaluated(hirIdOf14, 14)
  drop 11 12 13
  exitNeedsScope
} -> 14
duplicate 11 4
12 = lambda [11 4] { 12 ->
  enterNeedsScope double
  duplicate 12
  13 = call 11 with 12
  duplicate 13
  needs 13
  14 = Nothing
  15 = int 2
  duplicate 12 15
  16 = call 4 with 12 15
  drop 12 13 14 15
  exitNeedsScope
} -> 16
duplicate 12 1
13 = lambda [12 1] { 13 ->
  enterNeedsScope main
  14 = int 3
  duplicate 14
  15 = call 12 with 14
  duplicate 15
  16 = call 1 with 15
  error AstError (with more info)
  17 = Never
  drop 13 14 15 16
  exitNeedsScope
} -> 17
drop 1 2 3 4 5 6 7 8 9 10 11 12
output is map with 15 (because main is public with :=), although we have to think more about this
```

With usage and ownership optimizations:


```mir
1 = Builtin::Print
4 = Builtin::Multiply
9 = Builtin::TypeOf
10 = Builtin::Equals
duplicate 9 10
11 = lambda [9 10] { 11 ->
  enterNeedsContext isInt
  12 = call 9 with 11
  13 = symbol Int
  14 = call 10 with 12 13
  exitNeedsContext
} -> 14
duplicate 11 4
12 = lambda [11 4] { 12 ->
  enterNeedsContext double
  duplicate 12
  13 = call 11 with 12
  needs 13
  15 = int 2
  16 = call 4 with 12 15
  exitNeedsContext
} -> 16
duplicate 12 1
13 = lambda [12 1] { 13 ->
  enterNeedsContext main
  drop 13
  14 = int 3
  15 = call 12 with 14
  16 = call 1 with 15
  drop 16
  error AstError (with more info)
  17 = Never
  exitNeedsContext
} -> 17
drop 1 2 3 4 5 6 7 8 9 10 11 12
output is map with 15 (because main is public with :=), although we have to think more about this
```

inlining etc. could also be done without losing information about the call context (used for the needs)
full debugging would still work even after inlining

# LIR

a = 3
c = createChannel 4
main environment = pipe c environment.stdout
blub = [
  Foo: {}
]

```lir
  builtin print         # [1:print]
  builtin multiply      # [1:print, 4:multiply]
  builtin typeof        # [1:print, 4:multiply, 9:typeof]
  builtin equals        # [1:print, 4:multiply, 9:typeof, 10:equals]
  jump afterLambda11Body
lambda11Body #isInt
  enterNeedsScope isInt # [1:print, 4:multiply, 9:typeof, 10:equals, ..., 11:parameter]
  pushFromStack -1      # [1:print, 4:multiply, 9:typeof, 10:equals, ..., 11:parameter, 11:parameter]
  call 2                # [1:print, 4:multiply, 9:typeof, 10:equals, ..., 11:parameter, 12:resultOfTypeOfCall]
  createSymbol Int      # [1:print, 4:multiply, 9:typeof, 10:equals, ..., 11:parameter, 12:resultOfTypeOfCall, 13:Int]
  pushFromStack -2      # [1:print, 4:multiply, 9:typeof, 10:equals, ..., 11:parameter, 12:resultOfTypeOfCall, 13:Int, 12:resultOfTypeOfCall]
  pushFromStack -3      # [1:print, 4:multiply, 9:typeof, 10:equals, ..., 11:parameter, 12:resultOfTypeOfCall, 13:Int, 12:resultOfTypeOfCall, 13:Int]
  call 3                # [1:print, 4:multiply, 9:typeof, 10:equals, ..., 11:parameter, 12:resultOfTypeOfCall, 13:Int, 14:resultOfEqualsCall]
  expressionEvaluated HirId
  exitNeedsContext      # [1:print, 4:multiply, 9:typeof, 10:equals, ..., 11:parameter, 12:resultOfTypeOfCall, 13:Int, 14:resultOfEqualsCall]
  popMultipleBelowTop 3 # [1:print, 4:multiply, 9:typeof, 10:equals, ..., 14:resultOfEqualsCall]
  return                # [1:print, 4:multiply, 9:typeof, 10:equals, ..., 14:resultOfEqualsCall]
afterLambda11Body
  duplicate 2           # [1:print, 4:multiply, 9:typeof, 10:equals]
  duplicate 3           # [1:print, 4:multiply, 9:typeof, 10:equals]
  createLambda lambda11Body # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt]
  duplicate 4           # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt]
  duplicate 1           # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt]
  jump afterLambda12
lambda12Body # double
  enterNeedsScope double # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, ..., 12:parameter]
  duplicate -1          # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, ..., 12:parameter]
  pushFromStack -1      # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, ..., 12:parameter, 12:parameter]
  call 4                # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, ..., 12:parameter, 13:resultOfIsIntCall]
  pushFromStack -1      # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, ..., 12:parameter, 13:resultOfIsIntCall, 13:resultOfIsIntCall]
  needs                 # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, ..., 12:parameter, 13:resultOfIsIntCall]
  createInt 2           # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, ..., 12:parameter, 13:resultOfIsIntCall, 15:2]
  pushFromStack -3      # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, ..., 12:parameter, 13:resultOfIsIntCall, 15:2, 12:parameter]
  pushFromStack -2      # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, ..., 12:parameter, 13:resultOfIsIntCall, 15:2, 12:parameter, 15:2]
  call 1                # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, ..., 12:parameter, 13:resultOfIsIntCall, 15:2, 16:resultOfMultiplyCall]
  exitNeedsContext      # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, ..., 12:parameter, 13:resultOfIsIntCall, 15:2, 16:resultOfMultiplyCall]
  popMultipleBelowTop 3 # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, ..., 16:resultOfMultiplyCall]
  return                # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, ..., 16:resultOfMultiplyCall]
afterLambda12Body
  createLambda lambda12Body # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double]
  duplicate 5           # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double]
  duplicate 0           # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double]
  jump afterLambda13Body
lambda13Body # main
  enterNeedsScope main   # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double, ..., 13:parameter]
  drop -1                # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double, ..., 13:parameter]
  createInt 3            # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double, ..., 13:parameter, 14:3]
  pushFromStack -1       # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double, ..., 13:parameter, 14:3, 14:3]
  call 5                 # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double, ..., 13:parameter, 14:3, 15:resultOfDoubleCall]
  pushFromStack -1       # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double, ..., 13:parameter, 14:3, 15:resultOfDoubleCall, 15:resultOfDoubleCall]
  call 0                 # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double, ..., 13:parameter, 14:3, 15:resultOfDoubleCall, 16:resultOfPrintCall]
  drop -1                # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double, ..., 13:parameter, 14:3, 15:resultOfDoubleCall, 16:resultOfPrintCall]
  error AstError         # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double, ..., 13:parameter, 14:3, 15:resultOfDoubleCall, 16:resultOfPrintCall]
  createSymbol Never     # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double, ..., 13:parameter, 14:3, 15:resultOfDoubleCall, 16:resultOfPrintCall, 17:Never]
  exitNeedsScope         # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double, ..., 13:parameter, 14:3, 15:resultOfDoubleCall, 16:resultOfPrintCall, 17:Never]
  popMultipleBelowTop 4  # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double, ..., 17:Never]
  return                 # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double, ..., 17:Never]
afterLambda13Body
  createLambda lambda13Body # [1:print, 4:multiply, 9:typeof, 10:equals, 11:isInt, 12:double, 13:main]
  drop 0
  drop 1
  drop 2
  drop 3
  drop 4
  drop 5
  # TODO: return 6 (main)
```
