# Language X



```
val instance = FieldClass()

users.map(User::toJson)
val method: (param1: Param1Type) -> ReturnType = instance.doFoo
method(param1)
```


- [1. Visibility Modifiers](#1-visibility-modifiers)
- [2. Properties](#2-properties)
- [3. Functions](#3-functions)
- [4. Classes](#4-classes)
  - [4.1. Abstract classes](#41-abstract-classes)
  - [4.2. Traits](#42-traits)
  - [4.3. `impl`](#43-impl)
- [5. Enums](#5-enums)
- [6. Generics](#6-generics)
- [7. Annotations](#7-annotations)
- [8. Expressions](#8-expressions)
  - [8.1. Operators](#81-operators)
  - [8.2. Literals](#82-literals)
    - [8.2.1. Strings](#821-strings)
    - [8.2.2. Collections](#822-collections)
  - [8.3. Labels](#83-labels)
  - [8.4. If](#84-if)
- [9. Statements](#9-statements)
  - [9.1. For?](#91-for)
  - [9.2. While?](#92-while)
  - [9.3. Do-While?](#93-do-while)
  - [9.4. Rethrow?](#94-rethrow)
  - [9.5. Return](#95-return)
  - [9.6. Labels](#96-labels)
  - [9.7. Break](#97-break)
  - [9.8. Continue](#98-continue)
  - [9.9. Yield & Yield-Each](#99-yield--yield-each)
  - [9.10. Assert](#910-assert)
- [10. Patterns](#10-patterns)
- [11. Modules & Scripts](#11-modules--scripts)
- [12. Types](#12-types)
  - [12.1. Function Types](#121-function-types)
  - [12.2. Value Constraints](#122-value-constraints)
  - [12.3. Implicit Casts](#123-implicit-casts)
- [13. Comments](#13-comments)
- [14. Decisions](#14-decisions)
  - [14.1. Differentiate between immutable list & immutable view](#141-differentiate-between-immutable-list--immutable-view)
- [15. Ideas for the future](#15-ideas-for-the-future)


## 1. Visibility Modifiers

- `private`: functions/methods only callable in the same `class` and `impl`s
- `internal`: visible in the same file
- `protected`: visible in the class, subclasses, and impls
- `public`: visible everywhere

| keyword     | same `class`/`impl` | same file | direct outer scope | subclasses | everywhere |
| :---------- | :-----------------: | :-------: | :----------------: | :--------: | :--------: |
| `private`   |          ✅          |     ❌     |         ✅          |     ❌      |     ❌      |
| `protected` |          ✅          |     ❌     |         ✅          |     ✅      |     ❌      |
| `public`    |          ✅          |     ✅     |         ✅          |     ✅      |     ✅      |

By default, everything has the lowest possible visibility modifier (usually `private`).


## 2. Properties

```rust
const constant: Int = 0
let readonly: Int = 0
let mut mutable: Int = 0

let mut whoosh: Int = bazfoo
  private get
let mut floop: Int
  private set
let mut blub: Int
  private get: Int => blub * 5
  private get: Float => blub / 2 // recursion?
  private get: String => blub.toString()
  private get: Bytes => '${(blub + 1) / 2.0 * blub}$blub'.toUtf8()
  private get: Int => blub > 5 ? blub.length : blub.sum();
  private set => field = value + 1
let blab: Int
  get: Float => field

lateinit let baz: Int

let computed: Int
  private get => foo.length
```


## 3. Functions

TODO:
  fun doBar() => computeFoo()
  fun doBar() = methodReference


```kotlin
fun abc(a: Int, b: String = "abc"): Foo {
  /// Parameters may have default values that don't need to be `const`.

  /// After a parameter with a default value, all following parameters need to
  /// have a default value as well.

  // Can be called like:
  abc(0)
  abc(0, "abc")
  abc(a: 0, b: "abc")
  abc(0, b: "abc")
  abc(a: 0, "abc")

  return Foo()
}
```

The default return type (if not specified) is `Unit`. When using an expression body or delegating to a different function, the return type is inferred.

Overloading is supported based on parameters and return type.

Infix methods are supported:

```kotlin
class Matrix {
  infix fun dot(other: Matrix): Matrix
}

// Use as `matrix dot matrix`.
```


## 4. Classes

```kotlin
const class VerySimpleClass

const class SimpleClass1 {
  let foo: String
  let bar: Int = 0
}

// Implicitly generates constructor with parameters foo and bar, where foo is optional.
class VerySimpleClass {
  let foo: Int = 0
  let bar: String
}

// Defines a constructor, so there's no default constructor any more.
class SimpleClass2 {
  constructor(this.foo, unnecessaryOther: Int = 3) {
    // This is an optional body.
    // Every readonly property without a default value must be set exactly once.
    // Every read-and-write property must be set at least once.
  }

  let foo: Int = 0
  let bar: Int = 0
    get => field as Double
}

class SimpleClass3 {
  let foo: String = "blub"
  let bar: Int = -1
}
class SimpleClass4 {
  private constructor
  // This uses the default constructor, but changes its visibility.

  let foo: String = "0"
  let bar: Int = 0
}

class Class5 {
  class NestedClass
}
```


```kotlin
class FieldClass {
  // constructors:
  constructor(this.foo, this.bar, bazfoo: Int)

  // properties:
  let foo: Int
  let mut bar: Int
  let mut withDefault: Int = foo

  // methods:
  fun doFoo(param1: Param1Type): ReturnType {
  }
}
```


```rust
let unit: VerySimpleClass = VerySimpleClass()
let field: FieldClass = FieldClass(foo: 1, bar: 2)
```

### 4.1. Abstract classes

```kotlin
abstract class Foo
```

- cannot be instantiated

### 4.2. Traits

```kotlin
trait Foo
```

- cannot be instantiated

### 4.3. `impl`

`trait`s can be implemented:

```rust
impl Foo: Bar {
  // Implement trait [Bar] for type [Foo].

  fun baz() {}
}

// Implementations for all cases must be provided. The same goes for abstract
// classes.
//
// Note that you can't require an impl to be provided for a type defined by
// some other package without providing a default implementation for it.
impl MyEnum: Foo

// Existing methods matching the trait will be reused → the implementation can
// potentially be empty.
impl MyClass: Foo
```

`impl`s may only contain the functions specified by the implemented `trait`s.

You can also overload the `impl` based on the `trait`s type parameters – so `Foo` can implement both `List<Int>` and `List<String>`.

Visibility (can't have an explicit modifier): that of the base class, as long as the package defining the trait is a dependency (no `use` for the trait necessary)

You can also implement `trait`s anonymously inline. The following creates an anonymous class implementing the trait `Foo` and passes it to the method `doWithFoo`:

```
dooWithFoo(impl : Foo {
  fun foo() {}
  fun bar() {}
})
```

For implementing multiple `trait`s, shorten your code like the following:

```rust
impl Int: Add<Int, Int>, Subtract<Int, Int> {
  fun add(other: Int): Int {}
  fun subtract(other: Int): Int {}
}
```

Is the same as:

```rust
impl Int: Add<Int, Int> {
  fun add(other: Int): Int {}
}
impl Int: Subtract<Int, Int> {
  fun subtract(other: Int): Int {}
}
```

For defining an `impl` without a trait (visible on the base type, but limited to the current package), write this:

```rust
impl String {
  let doubled: This
    get => this + this
}
```


## 5. Enums

```kotlin
enum Foo1 { // implicitly extends `Enum<Void>`
  Bar,
  Baz,
  FooBar,
}
enum Foo2: Int {
  Bar, // implicitly 0
  Baz, // implicitly 1
  FooBar = Bar | Baz, // You can access enum values defined above.
}
enum Foo3: String {
  Bar = "abc",
  Baz = "def",
  FooBar = Bar + Baz,
}
enum Barcode {
  // generates: class Barcode<T : (Int, Int, Int, Int) | String> : Enum<T>
  Upc = (Int, Int, Int, Int),
  // generates: class Upc : Barcode<(Int, Int, Int, Int)>
  QrCode = String,
  // generates: class QrCode : Barcode<String>
}
```

(with `class Enum<T>(let value: T, name: String)`)

example usage: `Foo1.Bar`, `Foo2.FooBar.value`, `Barcode.Upc((1, 2, 3, 4))`


## 6. Generics

```rust
trait Abc<T1, T2, …, Tn: Foo = Bar>
  where <ValueConstraints> {}

impl Abc<Foo, Tn: Bar, T2: Baz> for MyStruct
  where <ValueConstraints> {}
```

The behavior of named/positional type arguments is the same as that of function calls.


## 7. Annotations

## 8. Expressions

- Implicit member access (see Swift)


### 8.1. Operators

| Precedence   | Description         | Operators                                                                              | Associativity |
| :----------- | :------------------ | :------------------------------------------------------------------------------------- | :-----------: |
| 22 (highest) | primitive           | literals, identifiers                                                                  |       —       |
| 21           | grouping            | `(expr)`                                                                               |       —       |
| 20           | postfix             | `expr++` `expr--` `?` `!` `.identifier` `expr<types>(args)` `expr<types>[args]`        |               |
| 19           | unary prefix        | `-expr` `!expr` `~expr` `++expr` `--expr` label                                        |       —       |
| 18           | multiplicative      | `*` `/` `~/` `%`                                                                       | left to right |
| 17           | additive            | `+` `-`                                                                                | left to right |
| 16           | shift               | `<<` `>>` `>>>`                                                                        | left to right |
| 15           | bitwise and         | `&`                                                                                    | left to right |
| 14           | bitwise xor         | `^`                                                                                    | left to right |
| 13           | bitwise or          | `|`                                                                                    | left to right |
| 12           | type check          | `as!` `as?`                                                                            | left to right |
| 11           | range               | `..`, `..=`                                                                            |               |
| 10           | infix function      | `simpleIdentifier`                                                                     | left to right |
| 9            | null coalescing     | `??`                                                                                   | left to right |
| 8            | named checks        | `in` `!in` `is` `!is`                                                                  |               |
| 7            | comparison          | `<` `<=` `>` `>=` `<=>`                                                                |       —       |
| 6            | equality            | `==` `!=` `===` `!==`                                                                  | left to right |
| 5            | logical and         | `&&`                                                                                   | left to right |
| 4            | logical or          | `||`                                                                                   | left to right |
| 3            | logical implication | `->` `<-`                                                                              | left to right |
| 2            | spread              | `...`                                                                                  |               |
| 1 (lowest)   | assignment          | `=` `*=` `/=` `~/=` `%=` `+=` `-=` `<<=` `>>=` `>>>=` `&=` `|=` `^=` `??=` `&&=` `||=` | right to left |

Spread in function calls:

```rust
let tuple = (x, y)
Point(tuple.x, tuple.y)
Point(...tuple)
```

### 8.2. Literals

#### 8.2.1. Strings

```rust
"foo" // foo
"foo {bar}" // foo <bar's value>
"foo {bar.baz}" // foo <bar.baz's value>
r"foo {bar.baz}" // foo {bar.baz}
##"foo " "# bar {bar}"## // foo " "# bar <bar's value>
r##"foo " "# bar {bar}"## // foo " "# bar {bar}
```

Line breaks within multi-line string literals get normalized to a single line feed each.

#### 8.2.2. Collections

List literal: `[1, 2, 3]`
Set literal: `{1, 2, 3}`
Map literal: `{1 to 1, 2 to 2, 3 to 3}`

If the values are compile-time inferred to be `Map.Entry`s, the literal creates a map. You can change it to be a set by:
- explicitly specifying type arguments (`<Map.Entry<Key, Value>>{ … }`)
- explicitly specifying its type (`let a: Set<Map.Entry<Key, Value>> = { … }`)

You can also use `if` expressions without an `else` branch, as well as safe/unsafe unwrapping (`?`/`!`).

```dart
let a: Optional<Iterable<Int>>
let b: Iterable<Optional<Int>>
let c: Iterable<Int> = [
  if (true) 1 else 2,
  if (false) 2,
  if (false) ...[1, 2],
  ...myList.indices.map { it * it },
  if (true) ...myList.indices.map { it * it } else 2,
  ...a?,
  ...b.whereNotNone,
]
```


### 8.3. Labels

Loops and lambdas can be prefixed by an optional label. This can then be used by `continue`, `break` and `return` statements:

```kotlin
fun foo() {
  outer@while (true) {
    while (true) {
      break@outer
    }
  }
}

fun bar() {
  list.map myMap@{
    return@myMap
  }
}
```

### 8.4. If

TODO: If-let

Single-line `if`s/`else`s use parentheses around the condition, while multiline bodies require curly braces anyway, which means you don't need parentheses around the condition.

The return value of an `if`-expression is the last expression of each branch.

```rust
let a = if (…)
  …
else if … {
  …
  …
} else
  …
```

```kotlin
if (…) … else …
```



## 9. Statements

### 9.1. For?
### 9.2. While?
### 9.3. Do-While?
### 9.4. Rethrow?
### 9.5. Return
### 9.6. Labels
### 9.7. Break
### 9.8. Continue
### 9.9. Yield & Yield-Each
### 9.10. Assert

## 10. Patterns

```rust
match x {
  1 => "exactly 1"
  2 | 3 => "2 or 3"
  a: Int if a.isEven => "is even"
  (1, a) => "tuple of 1 and {b}"
  ("abc", a = 1 | 2) => #"("abc", 1) or ("abc", 2) (and a captures the value)"#
  ("abc", a: Int) => #"Tuple of "abc" and an integer ({a})"#
  a = 4 | 5 => "is 4 or 5 and captured in a"
  a in 6..8 => "is within 6 and 8 and captured in a"
  a: Int => "is of type Int and captured in a"
  Option.Some(a) => "Some of {a}"
  _: Option<a = Int | UInt> => "Option<{a}>"
  _: Option<T> => "Option<{T}>"
  _: ((Int) => String) => "Function from Int to String"
  _ => "default"
}

let (a, b) = (1, 2)
if let .Some(a) = x { … }
for k, v in myMap { … }
for .Some(a) in myList { … }
```


## 11. Modules & Scripts

- `use`: import a module
- `public use`: import & export a module

```rust
use date_time
use firebase/core show FirebaseError
use firebase/firestore as fs
use firebase/firestore.queries
use logging
use math hide BigInt
```

File structure of a project `foo`:

- `src`: folder with all source code
  - `main.candy`: default executable
  - `lib.candy`: default library export → `use foo`
  - `plugin.candy`: default compiler plugin export
- package config
- `README.md`
- `.git`, etc.


```yaml
# specifying targets isn't necessary unless you want to configure them
libraries:
  default: lib
  json: json
  yaml: yaml
binaries:
  default: main
plugins:
  my_plugin:
    module: plugins.my_plugin
    isReadOnly: false
    functionVisitor: myFunctionVisitingFunction
    after:
      - data_classes
    before:
      - serialization
```

---

<details>
<summary>Example library exports & imports</summary>

**`serializable` package**

- `README.md`
- `src`: folder with all source code
  - `default.candy`: `const class Serializable`
  - `json`
    - `mod.candy`: `const class JsonName`
    - `plugin.candy`
  - `yaml`
    - `mod.candy`: `const class YamlName`
    - `plugin.candy`
  - `config.candy`:
    ```kotlin
    @Serializable()
    const class Config {
      let json: JsonConfig = JsonConfig()
    }

    @Serializable()
    const class JsonConfig {
      let defaultCasing: Casing = Casing.lowerPascal
    }
    ```
- `example.candy` (or `examples/main.candy`)
- package config:
  ```yaml
  libraries:
    default
    json
    yaml
  config: config.Config
  plugins:
    json: json.plugin
    yaml: yaml.plugin
  ```

**Usage**

```yaml
dependencies:
  serializable:
    version: ^1.0.0
    plugin:
      permissions:
        file:
          - assets/private/**: deny
          - assets/**: read
          - generated/**: write
        network:
          - https://evil.dev: false
          - https://*.dev: true
        environment:
          - CANDY_SERIALIZABLE_*: true
      config:
        json:
          defaultCasing: snake
```


```rust
use serializable

@Serializable()
class Foo {
  @json.JsonName("foo_bar")
  let mut fooBar
}
```

```rust
use serializable
use serializable.json

@Serializable()
class Foo {
  @JsonName("foo_bar")
  let mut fooBar
}
```

```rust
use serializable
use other_serializable

@serializable.Serializable()
class Foo
```

```rust
use serializable
use other_serializable hide Serializable

@Serializable()
class Foo
```
</details>

Compiler Plugin:
- runs in separate process; communication via stdin/stdout → TODO: protocol
- can request execution per class, per function
  - can be filtered to classes/methods with specific annotations
- runs only on the target module and not on its dependencies
- when providing a configuration, `impl Json: TryTo<Config>` and `impl Config: To<Json>` must be available


## 12. Types

Primitive types:
- `Bool`
- `Number`:
  - integers:
    - probably: `Int8`, `UInt8`, `Int16`, `UInt16`, `Int32`, `UInt32`, `Int64`, `UInt64` & `Int`, `UInt`
    - alternative: `Byte`, `UByte`, `Short`, `UShort`, `Int`, `UInt`, `Long`, `ULong`
  - floating point:
    - probably: `Float32`, `Float64` & `Float`
    - alternative: `Float`, `Double`
- `String`: `ByteArray`, `Iterable<CodePoint>`, `Iterable<GraphemeCluster>`
- `Unit`
- `Nothing`, `Never` or `None`
- `(T1, …, Tn) ≡ Tuple<T1, …, Tn>` (with `n in 2..*`)
- `(P1, …, Pn) -> R ≡ Function<P1, …, Pn, R>` ()
- `Type<T>` (potentially)

### 12.1. Function Types

```kotlin
R.(T1 t1, T2 t2, …, Tn tn = dn) -> T
```

### 12.2. Value Constraints

```kotlin
fun a(Pair<Int, Int> pair)
    where pair.first <= pair.second {
  
}
```


### 12.3. Implicit Casts

By implementing `As<T>` for `Foo`, you can implicitly (or explicitly) use `Foo` as `T`. This doesn't work transitively, though you could write `Foo as T as R`.

This also provides what is known as Interface Delegation in Kotlin.


## 13. Comments

- automatic line wrapping


## 14. Decisions

### 14.1. Differentiate between immutable list & immutable view

- just provide an immutable trait, since anybody could still implement an immutable list trait on a mutable one


## 15. Ideas for the future

- allow dependencies in default parameter values in any order, as long as these dependencies form a DAG (i.e., they don't contain any cycles)
- syntactic sugar for the `As<T>` trait
- `.=` operator (e.g., `email .= trim()` means `email = email.trim()`)
- allow values (not only types) as type arguments
  - this would enable parameter names as part of function signatures as well
- pre-/postcondition for functions: possibly with a similar syntax to Kotlin's contracts
- secondary constructors
- module aliases in imports
- `impl` directly in `class`
- cross-boundary lazy
- write-only property (`let settable onlyWriteable: Int`)
- named, factory and delegating constructors
- permissions for the executable and dependencies (network, file, etc.)
- overloading classes/traits based on type parameters
- use abstract class as trait by ignoring implementations depending on private fields
- static initializer block
- overflow operators
- chained comparison
- implicit multiplication: a literal number before an identifier creates an implicit multiplication: `2 apples` is equivalent to `2 * apples`
  - `2 to -2` is equivalent to `2.to(-2)`, not `2 * to - 2`?
  - what about `1 / 2 foo`?
  - or rather postfix functions? `2 seconds + 3 minutes` and `2.50 euro`
- for in collection literals:
  ```dart
  [
    for (i in myList.indices) i * i,
    if (true)
      for (i in myList.indices) i
    else 2,
  ]
  ```
- optional lifetimes → for compilation to Rust/LLVM/etc.
