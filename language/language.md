# Language X

## TODO

- Compiler Plugins

```
val instance = FieldClass()

users.map(User::toJson)
val method: (param1: Param1Type) -> ReturnType = instance.doFoo
method(param1)
```


- [1. Visibility Modifiers](#1-visibility-modifiers)
- [2. Variables](#2-variables)
- [3. Functions](#3-functions)
  - [3.1. Extension Methods](#31-extension-methods)
- [4. Classes](#4-classes)
  - [4.1. Abstract classes](#41-abstract-classes)
  - [4.2. Traits](#42-traits)
  - [4.3. `impl`](#43-impl)
- [5. Enums](#5-enums)
- [6. Generics](#6-generics)
- [7. Annotations](#7-annotations)
- [8. Expressions](#8-expressions)
  - [8.1. Operators](#81-operators)
  - [8.2. Labels](#82-labels)
  - [8.3. If](#83-if)
  - [8.4. When/Match](#84-whenmatch)
  - [8.5. Try?](#85-try)
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
- [10. Modules & Scripts](#10-modules--scripts)
- [11. Types](#11-types)
  - [11.1. Function Types](#111-function-types)
  - [11.2. Value Constraints](#112-value-constraints)
  - [11.3. Comments](#113-comments)


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

## 2. Variables

```rust
const constant: Int = 0
let readonly: Int = 0
let mut mutable: Int = 0
```


## 3. Functions

TODO:
  fun doBar() => computeFoo()
  fun doBar() = methodReference


```kotlin
fun abc(a: Int, b: String = "abc") {
  /// Parameters may have default values that don't need to be `const`.

  /// After a parameter with a default value, all following parameters need to
  /// have a default value as well.

  // Can be called like:
  abc(0)
  abc(0, "abc")
  abc(a: 0, b: "abc")
  abc(0, b: "abc")
  abc(a: 0, "abc")
}
```

Overloading is supported based on parameters and return type.

### 3.1. Extension Methods

Syntax is equivalent to Kotlin.


## 4. Classes

```kotlin
class VerySimpleClass;

class SimpleClass1 {
  let foo: String
  let bar: Int
}
// equivalent to:
class SimpleClass2(this.foo, this.bar) {
  constructor(baz: Foobar) => this(baz.foo, baz.bar)
  constructor(baz: Foobar) {
    return this(baz.foo, baz.bar)
  }

  let foo: String
  let bar: Int
}

class SimpleClass3(baz: Foobar) {
  let foo: String = baz.foo
  let bar: Int = baz.bar
}
class SimpleClass4 private constructor {
  let foo: String = baz.foo
  let bar: Int = baz.bar
}
```

- secondary constructors might come in a later version if they seem necessary

// TODO: const constructors?



```kotlin
class FieldClass private constructor(this.foo, this.bar, bazfoo: Int) {
  // constructors:
  constructor() => this(foo, 0, 0)

  init {
    bar = 0
  }

  // properties:
  let foo: Int
  let mut bar: Int
  let mut withDefault: Int = foo

  let mut whoosh: Int = bazfoo
    private get
  let mut floop: Int
    private set
  let settable onlyWriteable: Int = 0
    private set
  let mut blub: Int
    private get: int => blub * 5
    private get: double => blub / 2 // recursion?
    private get: String => blub.toString()
    private get: Bytes => '${(blub + 1) / 2.0 * blub}$blub'.toUtf8()
    private get: int => blub > 5 ? blub.length : blub.sum();
    private get: T => blub.as<T>()
    private set => field = value + 1
    private set(value: T) => blub = value / 2
  let blab: Int
    get: double => field

  lateinit let baz: Int

  let computed: Int
    private get => foo.length

  private get computed: Int => foo.length;

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

Abstract classes and traits can be implemented:

```rust
impl Foo: Bar {
  // Implement trait [Bar] for type [Foo].

  override fun baz() {}
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

Visibility (can't have an explicit modifier): intersection of `class`/`enum` and abstract class/trait visibilities


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

| Precedence   | Description             | Operators                                                                        | Associativity |
| :----------- | :---------------------- | :------------------------------------------------------------------------------- | :-----------: |
| 21 (highest) | grouping                | `(expr)`                                                                         |       —       |
| 20           | unary postfix           | `expr++` `expr--` `.` `?.` `expr(args)` `expr?(args)` `expr[args]` `expr?[args]` |               |
| 19           | unary prefix            | `-expr` `!expr` `~expr` `++expr` `--expr` label                                  |       —       |
| 18           | implicit multiplication | `number expr`                                                                    |       —       |
| 17           | multiplicative          | `*` `/` `~/` `%`                                                                 | left to right |
| 16           | additive                | `+` `-`                                                                          | left to right |
| 15           | shift                   | `<<` `>>` `>>>`                                                                  | left to right |
| 14           | bitwise and             | `&`                                                                              | left to right |
| 13           | bitwise xor             | `^`                                                                              | left to right |
| 12           | bitwise or              | `|`                                                                              | left to right |
| 11           | type check              | `as` `as?`                                                                       | left to right |
| 10           | range                   | `..`, `..=`                                                                      |               |
| 9            | infix function          | `simpleIdentifier`                                                               |               |
| 8            | named checks            | `in` `!in` `is` `!is`                                                            |               |
| 7            | comparison              | `<` `<=` `>` `>=`                                                                |               |
| 6            | equality                | `==` `!=` `===` `!==`                                                            |               |
| 5            | logical and             | `&&`                                                                             | left to right |
| 4            | logical or              | `||`                                                                             | left to right |
| 3            | logical implication     | `->` `<-`                                                                        | left to right |
| 2            | spread                  | `...`                                                                            |               |
| 1 (lowest)   | assignment              | `=` `*=` `/=` `~/=` `%=` `+=` `-=` `&=` `|=` `^=` `&&=` `||=` `<<=` `>>=` `>>>=` | right to left |

- implicit multiplication: a literal number before an identifier creates an implicit multiplication: `2 apples` is equivalent to `2 * apples`
  - something like `2 to -2` is equivalent to `2.to(-2)`, not `2 * to - 2`


Spread in function calls:

```rust
let tuple = (x, y)
Point(tuple.x, tuple.y)
Point(...tuple)
```

TODO: ??/?:, :, ternary, `??=`

### 8.2. Labels

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

### 8.3. If

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


### 8.4. When/Match
### 8.5. Try?

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

## 10. Modules & Scripts

- `use`: import a module
- `public use`: import & export a module


## 11. Types

### 11.1. Function Types

```kotlin
R.(T1 t1, T2 t2, …, Tn tn = dn) -> T
```

### 11.2. Value Constraints

```kotlin
fun a(Pair<Int, Int> pair)
    where pair.first <= pair.second {
  
}
```


### 11.3. Comments


