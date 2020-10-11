# Language X



- [1. About](#1-about)
  - [1.1. Goals](#11-goals)
  - [1.2. Vision](#12-vision)
  - [1.3. Paradigms](#13-paradigms)
- [2. Visibility Modifier](#2-visibility-modifier)
- [3. Properties](#3-properties)
- [4. Functions](#4-functions)
- [5. Classes](#5-classes)
  - [5.1. Traits](#51-traits)
  - [5.2. `impl`](#52-impl)
- [6. Enums](#6-enums)
- [7. Generics](#7-generics)
- [8. Annotations](#8-annotations)
  - [8.1. Keywords](#81-keywords)
- [9. Expressions](#9-expressions)
  - [9.1. Operators](#91-operators)
  - [9.2. Literals](#92-literals)
    - [9.2.1. Strings](#921-strings)
    - [9.2.2. Collections](#922-collections)
  - [9.3. Labels](#93-labels)
  - [9.4. If](#94-if)
  - [9.5. Match](#95-match)
  - [9.6. Return](#96-return)
  - [9.7. Break](#97-break)
  - [9.8. Continue](#98-continue)
  - [9.9. Throw, Try & Catch](#99-throw-try--catch)
  - [9.10. Yield & Yield-Each](#910-yield--yield-each)
  - [9.11. Embedded languages](#911-embedded-languages)
- [10. Statements](#10-statements)
  - [10.1. For](#101-for)
  - [10.2. Loop](#102-loop)
  - [10.3. While](#103-while)
- [11. Patterns](#11-patterns)
- [12. Modules & Scripts](#12-modules--scripts)
- [13. Types](#13-types)
  - [13.1. Function Types](#131-function-types)
  - [13.2. Value Constraints](#132-value-constraints)
  - [13.3. Implicit Casts](#133-implicit-casts)
- [14. Comments](#14-comments)
- [15. Decisions](#15-decisions)
  - [15.1. Differentiate between immutable list & immutable view](#151-differentiate-between-immutable-list--immutable-view)
- [16. Ideas for the future](#16-ideas-for-the-future)


## 1. About

### 1.1. Goals

- compiles to/supports:
  - Flutter (Dart)
  - native (LLVM)
  - web (WASM; JS via Dart)
  - Kotlin\
  → interoperability between programming languages
- no manual memory management
- can run on microcontrollers
  - memory management via compiler-inferred alloc/free plus GC for remaining cases
- easy to learn, but easily extensible
- rich standard library
- curated package ecosystem including recommended packages for common use-cases (logging, serialization, etc.)
- concise syntax
- great static analysis
- compiler plugin support
- epic tooling: hot reload, website with APIs, editor extensions, GitHub Actions, CLI tools, automatic diagrams & stats

### 1.2. Vision

A simple, extensible language enabling everyone to focus on getting things done.

### 1.3. Paradigms

OOP Pros:

- familiar
- typical inheritance examples / intuitive

Rust-like Pros:

- no "one size fits all"
  - forces you to define better interfaces
- mix and match / composition over inheritance
- simple (not so simple to abuse)

Hence: OOP without inheritance for classes, and with traits/impls


## 2. Visibility Modifier

`public`: can be exported from the module. By default, everything is private and only visible within the containing module.


## 3. Properties

Properties are named storage slots, whether global, in classes or in functions.

```rust
let readonly: Int = 0
mut let mutable: Int = 0

// Custom Getters:
let foo1: Int => 1
let foo2: Int {
  return 1
}
let foo3: Int
  get => 1
mut let blub: Int
  get => field * 5
  set => field = value + 1

// Delegation:
let bar: Int by vetoable { old, new => TODO() }

// Default value:
mut let whoosh: Int = bazfoo
let yumminess: Int = impl { // Trait is inferred
  fun foo() => 1
}

lateinit let baz: Int
```


## 4. Functions

Functions contain code that can be executed.

```kotlin
fun abc(a: Int, b: String = "abc"): Foo {
  /// After a parameter with a default value, all following parameters need to
  /// have a default value as well.

  // Can be called like:
  abc(0)
  abc(a = 0)
  abc(0, "abc")
  abc(a = 0, b = "abc")
  abc(0, b = "abc")
  abc(a = 0, "abc")

  return Foo()
}
```

Expression bodies:

```kotlin
fun doBar() => computeFoo()
fun doBar() = methodReference
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


## 5. Classes

```kotlin
class VerySimpleClass

// Implicitly generates constructor with parameters foo and bar, where bar is optional.
class SimpleClass1 {
  let foo: String
  let bar: Int = 0
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

class SimpleClass5 {
  class NestedClass
}
```

```kotlin
class FieldClass {
  // constructors:
  constructor(this.foo, this.bar, bazfoo: Int)

  // properties:
  let foo: Int
  mut let bar: This // Inside classes, you can use `This` to refer to the class itself.
  mut let withDefault: Int = foo

  // methods:
  fun doFoo(param1: Param1Type): ReturnType {
  }
}
```

```rust
let unit: VerySimpleClass = VerySimpleClass()
let field: FieldClass = FieldClass(foo = 1, bar = 2)
```

### 5.1. Traits

Traits can define an API (available properties and functions), but they cannot be instantiated.

```kotlin
trait Foo {
  fun baz(): Int
}
```

### 5.2. `impl`

`trait`s can be implemented:

```rust
impl Foo: Bar {
  // Implement trait [Bar] for type [Foo].

  fun baz() => 5
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

You can also overload the `impl` based on the `trait`s type parameters – so `Foo` can implement both `List<Int>` and `List<String>`.

Visibility (can't have an explicit modifier): that of the base class, as long as the package defining the trait is a dependency (no `use` for the trait necessary)

You can also implement `trait`s anonymously inline. The following creates an anonymous class implementing the trait `Foo` and passes it to the method `doWithFoo`:

```kotlin
dooWithFoo(impl : Foo {
  fun foo() {}
  fun bar() {}
})
```

For implementing multiple (usually related) `trait`s, you can shorten your code like the following:

```rust
// Implement algebra stuff.
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


## 6. Enums

```kotlin
enum Foo1 { // implicitly extends `Enum<Unit>`
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
  Upc: (Int, Int, Int, Int),
  // generates: class Upc : Barcode<(Int, Int, Int, Int)>
  QrCode: String,
  // generates: class QrCode : Barcode<String>
}
```

with:

```rust
trait Enum<T> {
  let value: T
  let name: String
}
impl<T> Enum<T>: As<T> { … }
```

example usage: `Foo1.Bar`, `Foo2.FooBar.value`, `Barcode.Upc((1, 2, 3, 4))`


## 7. Generics

```rust
trait Abc<T1, T2, …, Tn: Foo = Bar>
  where <ValueConstraints> {}

impl Abc<Foo, Tn: Bar, T2: Baz> for MyStruct
  where <ValueConstraints> {}
```

The behavior of named/positional type arguments is the same as that of function calls.


## 8. Annotations

You can define annotations using the `annotation` keyword before a `class` or `let` declaration.

```kotlin
annotation class MyAnnotationClass
annotation let myAnnotationProperty = MyAnnotationClass()

@MyAnnotationClass()
@myAnnotationProperty
class Foo
```

### 8.1. Keywords

You can define custom keywords like this:

```kotlin
// Annotation & keyword declaration:
annotation class DataClass
keyword let data = DataClass()

// Usage:
data class MyDataClass
// Instead of:
@DataClass()
class MyDataClass
```


## 9. Expressions

- Implicit member access (see Swift)


### 9.1. Operators

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

Safe unwrap in arguments:

```kotlin
let value: Option<Int> = Option.None
let point /* Option<Point> */ = Point(value?, 0)
```


### 9.2. Literals

#### 9.2.1. Strings

```rust
"foo" // foo
"foo {bar}" // foo <bar's value>
#"foo {bar}"# // foo {bar}
#"foo {{bar}}"# // foo <bar's value> (unnecessarily nested)
"foo {bar.baz}" // foo <bar.baz's value>
##"foo " "# bar {{{bar}}}"## // foo " "# bar <bar's value>
```

Line breaks within multi-line string literals get normalized to a single line feed each.

#### 9.2.2. Collections

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

### 9.3. Labels

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

### 9.4. If

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

### 9.5. Match

### 9.6. Return

### 9.7. Break

### 9.8. Continue

### 9.9. Throw, Try & Catch

```kotlin
try {
  problematicFunction()
} catch MyError { // it: CaughtError<MyError>
  if (!canHandle(it.error)) throw e
  handleMyError(it)
} catch Any { // it: CaughtError<Any>
  handle(it)
}
```

With the following types:

```kotlin
public class CaughtError<E> {
  external constructor

  let error: E
  let stackTrace: StackTrace

  let cause: Option<CaughtError<E>>
}

public class StackTrace {
  external constructor

  let items: List<Item>

  class Item {
    let fileName: String
    let position: Position // line + column
    let typeName: String
    let methodName: String
  }
}
```

### 9.10. Yield & Yield-Each

### 9.11. Embedded languages

```rust
let json: Json = {
  "foo": 123,
  "null": null, // with `let null = Null()` and `pub class Null { constructor }`
}
let css: Css = `css:
  body {
    color: rgba(0, 0, 0, 0);
  }
`
let sql: Sql = `sql:SELECT * FROM people WHERE name=@name`
let regex: RegEx = `regex:abc[\w\d]+`
```


## 10. Statements

### 10.1. For

### 10.2. Loop

### 10.3. While


## 11. Patterns

```rust
match x {
  1 => "exactly 1"
  2 | 3 => "2 or 3: {it}"
  a: Int if a.isEven => "is even"
  (1, a) => "tuple of 1 and {a}"
  ("abc", a = 1 | 2) => #"("abc", 1) or ("abc", 2) (and a captures the value)"#
  ("abc", a: Int) => #"Tuple of "abc" and an integer ({{a}})"#
  a = 4 | 5 => "is 4 or 5 and captured in a"
  a in 6..8 => "is within 6 and 8 and captured in a"
  a: Int => "is of type Int and captured in a"
  Option.Some(a) => "Some of {a}"
  _: Option<T = Int | UInt> => "Option<{T}>"
  _: Option<T> => "Option<{T}>"
  _: ((Int) => String) => "Function from Int to String"
  _ => "default"
}

let (a, b) = (1, 2)
if let .Some(a) = x { … }
for k, v in myMap { … }
for .Some(a) in myList { … }
```


## 12. Modules & Scripts

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
  - `default.candy`: `class Serializable`
  - `json`
    - `module.candy`: `class JsonName`
    - `plugin.candy`
  - `yaml`
    - `module.candy`: `class YamlName`
    - `plugin.candy`
  - `config.candy`:
    ```kotlin
    @Serializable()
    class Config {
      let json: JsonConfig = JsonConfig()
    }

    @Serializable()
    class JsonConfig {
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
  mut let fooBar
}
```

```rust
use serializable
use serializable.json

@Serializable()
class Foo {
  @JsonName("foo_bar")
  mut let fooBar
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


## 13. Types

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

### 13.1. Function Types

```kotlin
R.(T1 t1, T2 t2, …, Tn tn = dn) => T
```

### 13.2. Value Constraints

```kotlin
fun a(Pair<Int, Int> pair)
    where pair.first <= pair.second {

}
```

### 13.3. Implicit Casts

By implementing `As<T>` for `Foo`, you can implicitly (or explicitly) use `Foo` as `T`. This doesn't work transitively, though you could write `Foo as T as R`.

This also provides what is known as Interface Delegation in Kotlin.


## 14. Comments

- automatic line wrapping

```kotlin
fun foo(bar: Int): Result<Int, MyError> {
  /// A short sentence describes this item.
  ///
  /// Longer paragraphs may be written below. They can describe the function in
  /// more detail. They can also reference parameters like `bar`.
  ///
  /// # Complexity/Performance
  ///
  /// This function runs in $O(bar)$ and consumes constant space.
  ///
  /// # Errors
  ///
  /// `MyError` is returned when an even number is passed in.
  ///
  /// # Exceptions
  ///
  /// The following exceptions may be thrown:
  ///
  /// - `IndexOutOfRangeException`: If `bar` is `-1`.
  ///
  /// # Examples
  ///
  /// ```
  /// let result = foo(1)
  /// assert(result == .Ok(2))
  /// ```
}
```


## 15. Decisions

### 15.1. Differentiate between immutable list & immutable view

- just provide an immutable trait, since anybody could still implement an immutable list trait on a mutable one


## 16. Ideas for the future

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
- implicit multiplication: a literal number before an identifier creates an implicit multiplication: `2 apples` is equivalent to `2 * apples`. Works gracefully for units: `2 days + 3 minutes`, `2.50 euro`
  - `2 to -2` is equivalent to `2.to(-2)`, not `2 * to - 2`?
  - what about `1 / 2 foo`?
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
