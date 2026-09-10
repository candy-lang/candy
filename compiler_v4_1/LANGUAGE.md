# Candy Language Overview (v4.1)

This describes the subset of Candy currently implemented by this compiler
(`compiler_v4_1`). It is deliberately minimal — many things you'd expect from
a general-purpose language (operators, `if`, loops, lists literals) don't
exist yet and are built from what's below.

## Lexical structure

- Comments: `#` to end of line. No block comments.
- Identifiers: `[A-Za-z_][A-Za-z0-9_]*`. Casing isn't enforced, but by
  convention types are `UpperCamelCase` and values/functions are
  `lowerCamelCase`.
- Integer literals: decimal digits only (no `_` separators, no hex/float).
- Text literals: double-quoted, with `\r \n \t \" \\` escapes and
  `"...{expression}..."` interpolation (arbitrary expressions, not just
  identifiers).
- Keywords: `builtin struct enum trait impl where fun let switch use package public`
  (`where` is reserved but not yet wired into the grammar).
- No arithmetic/comparison/logical operator tokens exist at all — `+`, `==`,
  `&&`, etc. are just ordinary function/method calls.

## Files, packages, modules

A package is a directory containing a `_package.candy` marker file. Every
`.candy` file under it is a module; `_.candy` is that directory's root
module. `Core` is implicitly available to every package.

```candy
use core.text
use package.collections.list
```

`use` lines must come before all declarations in a file and are not
transitive (importing a module doesn't re-export its own imports). Only
`public` declarations are visible across packages; `package`-visible
declarations are visible within the same package.

## Visibility

Declarations default to **private**. `package` and `public` modifiers go
immediately before the declaration keyword:

```candy
struct Point {}        # private
package trait Named {} # visible within this package
public fun square(n: Int) Int { n.multiply(n) }
```

Struct fields can carry their own visibility. Functions in an inherent
`impl` can too, and are checked at the call site. `impl`s themselves,
associated types, and functions in a `trait` or a trait `impl` cannot have a
visibility modifier: a trait method is callable wherever its receiver is.

## Declarations

### `struct`

```candy
struct Point { x: Int, y: Int }
struct Box[T] { value: T }
struct Int = builtin       # a builtin type, no fields
```

Construct with positional arguments matching field order: `Point(1, 2)`.

### `enum`

```candy
enum Ordering { less, equal, greater }
enum Maybe[T] { none, some: T }
```

Variants may carry a single payload type. Construct via
`EnumType.variantName(payload)`, e.g. `Maybe.some(1)`, `Ordering.less()`.

### `trait` and `impl`

```candy
trait ToText { fun toText(self: Self) Text }

impl Int: ToText {
  fun toText(self: Int) Text { builtinIntToText(self) }
}

# inherent impl (no trait)
impl[T] Maybe[T] {
  fun orElse(self: Maybe[T], fallback: T) T {
    switch self { none => fallback, some(value) => value }
  }
}

# blanket impl
impl[T: Compare] T: Equal {
  fun equals(self: T, other: T) Bool { self.compareTo(other).isEqual() }
}
```

Trait functions may have a default body. An `impl` may be over a generic,
bounded type parameter (`impl[T: Bound] ...`), but an impl with no trait
needs a concrete type head — `impl[T] T { ... }` is rejected. Inherent
methods take priority over trait methods of the same name; the most
specific applicable impl is preferred over a blanket one.

An inherent impl's functions can also be called on the type itself, without a
receiver:

```candy
impl Int {
  public fun parse(text: Text) Result[Int, Text] { builtinIntParse(text) }
}

impl[T] Maybe[T] {
  fun empty() Maybe[T] { Maybe.none() }
}

Int.parse("42")
let none: Maybe[Int] = Maybe.empty()
```

Nothing marks a function as static: `Type.name(...)` passes its arguments
straight through, so `Int.divideTruncating(6, 3)` and `6.divideTruncating(3)`
both resolve, and the overload is picked from the argument types as usual. The
impl's own type parameters are inferred from the call site. An enum variant of
the same name wins over a function, since `Maybe.none()` has to keep building a
value.

Type parameters everywhere use `[T, U: Bound]`, with at most one trait bound
per parameter.

### `fun`

```candy
fun add(a: Int, b: Int) Int { builtinIntAdd(a, b) }
fun log(message: Text) { builtinPrint(message) }   # omitted return type = Nothing
```

No `return` keyword — a function's (and any block's) value is its last
expression. `main` must take no parameters and return `Int`.

### `let`

```candy
let x = 1
let y: Int = 2
public let answer = 42
```

Valid at top level and as a statement inside any block; type annotations are
optional and inferred bidirectionally from usage.

## Expressions

- Calls: `f(a, b)`, with optional explicit type arguments `f[Int](a)`.
- Navigation + call is how methods are invoked: `value.method(arg)`. There is
  **no UFCS** — a free function can't be called with dot syntax.
- Lambdas: `(a, b: Int) { a.add(b) }`. Parameter types are optional here
  (unlike named functions) and inferred from context.
- Blocks `{ ... }` are expressions in their own right.
- `switch` is the only branching construct, and is checked for
  exhaustiveness over all enum variants:

```candy
switch color {
  red => "r",
  green => "g",
  blue => "b",
}

switch maybeValue {
  none => 0,
  some(value) => value,
}
```

There is no `if`/`else`, `while`, `for`, `loop`, `break`/`continue`, operator
syntax, or list/tuple literal syntax. Iteration and booleans are ordinary
library code built from `switch` and recursive functions.

## Type inference

Types are inferred bidirectionally: `let` bindings without annotations take
the type implied by later use, and lambda parameter types are inferred from
the expected function type at the call site. An overload is resolved to the
most specific applicable candidate; genuinely ambiguous or unresolvable
cases are compile errors.

## Builtins

`use Builtins` brings in the compiler's builtin functions. There is no
built-in `Bool`, `Ordering`, `Maybe`, `Result`, or operator sugar — those are
ordinary declarations a program (or an imported package) must provide.
Builtin types: `Int`, `Text`, `List[T]`. Builtin functions include
`builtinIntAdd/Subtract/Multiply/DivideTruncating/Remainder/BitwiseAnd/Or/Xor/ShiftLeft/ShiftRight/CompareTo/Parse/ToText`,
`builtinListOf/Filled/Generate/Get/Insert/Length/RemoveAt/Replace`,
`builtinTextConcat/CompareTo/GetRange/IndexOf/Length`, `builtinPrint`,
`builtinPanic`.

## A complete example

```candy
use Builtins

struct Nothing {}
enum Never {}
struct Int = builtin
struct Text = builtin

fun main() Int {
  builtinPrint(builtinIntToText(42))
  0
}
```

## CLI

- `candy check <path>` — lex/parse/lower, report errors only.
- `candy compile <path>` — full pipeline down to C, then invokes `gcc`.
- `candy debug ast|hir|mono|c <path>` — dump an intermediate representation.
