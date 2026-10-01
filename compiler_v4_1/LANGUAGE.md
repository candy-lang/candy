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

Type parameters everywhere use `[T, U: Bound]`, and a parameter can have
several bounds joined by `+`: `[T: Equal + Hash]`.

### Trait objects and `Any`

A trait's name used as a type is a *trait object*: a value of any type that
implements the trait, with the concrete type only known at runtime. Calls on it
dispatch through a vtable.

```candy
trait Widget { fun describe(self: Self) Text }
struct Column { children: List[Widget] }

Column(List.of(Label("hi"), Counter(3)))
```

A value is boxed into a trait object implicitly wherever one is expected: a
`let` with an annotation, a function's or lambda's result, a struct field, an
enum payload, or a call argument. The expected type also flows into a call
with a single candidate, which is how `List.of(...)` above becomes a
`List[Widget]`. Without an expected type, `List.of(Label("hi"), Counter(3))`
is an error, since `T` can only be one type. There is no variance: a
`List[Label]` is not a `List[Widget]`.

A trait can be used as a type unless one of its functions without a default
body can't be called through a vtable. A function can be called that way if:

- its first parameter is `self: Self`,
- it has no type parameters of its own,
- no other parameter mentions `Self`,
- its result mentions `Self` at most as exactly `Self` (which is boxed back into
  the object), and
- it never mentions an associated type of `Self`.

A function with a default body that breaks these rules doesn't disqualify the
trait; it just can't be called on a trait object. A trait object implements its
own trait, so it can be passed where `[T: Widget]` is expected. Nothing can
implement a trait object type itself.

Every type implements `Core.Any` without an impl, and writing one is an error,
so the type ID it reports can be trusted:

```candy
let state: Any = CounterState(0)
state.typeId().equals(TypeId.of[CounterState]())  # true
state.is[CounterState]()                          # true
state.downcast[CounterState]()                    # Maybe.some(...)
```

On a trait object these see the concrete type, not the object type. Any trait
object can be used as an `Any`; other conversions between object types aren't
supported.

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
the expected function type at the call site. When only one function can match
a call, its parameter types are the expected types of the arguments, and the
call's own expected type pins down its type parameters first. An overload is
resolved to the most specific applicable candidate, where a parameter of a
concrete type beats a trait object it implements; genuinely ambiguous or
unresolvable cases are compile errors.

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
