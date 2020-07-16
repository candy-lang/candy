# Language X

## TODO

- Compiler Plugins

```
val instance = FieldClass()

users.map(User::toJson)
val method: (param1: Param1Type) -> ReturnType = instance.doFoo
method(param1)
```


## 1. Visibility Modifiers

- `secret`: functions/methods only callable in the same `class`/`impl`
- `private`: functions/methods only callable in the same `class` and `impl`s
- `internal`: visible in the same file
- `protected`: visible in the class, subclasses, and impls
- `public`: visible everywhere

| keyword     | same `class`/`impl` | same file | direct outer scope | subclasses | everywhere |
| :---------- | :-----------------: | :-------: | :----------------: | :--------: | :--------: |
| `secret`    |          ✅          |     ❌     |         ❌          |     ❌      |     ❌      |
| `private`   |          ✅          |     ❌     |         ✅          |     ❌      |     ❌      |
| `protected` |          ✅          |     ❌     |         ✅          |     ✅      |     ❌      |
| `public`    |          ✅          |     ✅     |         ✅          |     ✅      |     ✅      |

By default, everything has the lowest possible visibility modifier (usually `secret`).

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
fun abc(a: Int, b: String = 'abc') {
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

named fun onlyAllowsNamedParameters(a: Int = 0, c: Double, b: String = 'abc') {
  /// When calling this function, all parameters must be named. This allows
  /// new parameters to be added in a backwards compatible change, such as `c`
  /// above.

  // Can be called like:
  onlyAllowsNamedParameters()
  onlyAllowsNamedParameters(b: "abc")
  onlyAllowsNamedParameters(a: 0, b: "abc")
}
```

Overloading is supported based on parameters and return type.


## 4. Classes

```kotlin
class VerySimpleClass;

class SimpleClass1 {
  let foo: String
  let bar: Int
}
// equivalent to:
class SimpleClass2(this.foo, this.bar) {
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

impl SimpleClass2 {
  constructor(baz: Foobar) => this(baz.foo, baz.bar)
  constructor(baz: Foobar) {
    return this(baz.foo, baz.bar)
  }
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

### 4.2. Interfaces

```kotlin
interface Foo
```

- cannot be instantiated

### 4.3. `impl`

Abstract classes and interfaces can be implemented:

```rust
impl Foo for Bar
```

Visibility (can't have a manual modifier): intersection of `class` and abstract class/interface visibilities


## 5. Enums


## 6. Traits ❤️

TODO: inline `impl` of `interface`

## 7. Generics

```rust
named trait Abc<T1, T2, …, Tn: Foo = Bar>
  where <ValueConstraints> {}

impl Abc<Foo, Tn: Bar, T2: Baz> for MyStruct
  where <ValueConstraints> {}
```

The behavior of `named` and named/positional type arguments is the same as that of function calls.


## 8. Metadata

## 9. Expressions

- Implicit member access (see Swift)

### Operators

| Precedence | Description                | Operators                                                                        | Associativity |
| :--------- | :------------------------- | :------------------------------------------------------------------------------- | :-----------: |
| Highest    | grouping                   | `(expr)`                                                                         |       —       |
|            | unary postfix              | `expr++` `expr--` `.` `?.` `expr(args)` `expr?(args)` `expr[args]` `expr?[args]` |               |
|            | unary prefix               | `-expr` `!expr` `~expr` `++expr` `--expr` label                                  |       —       |
|            | multiplicative             | `*` `/` `~/` `%` `÷`                                                             | left to right |
|            | additive                   | `+` `-`                                                                          | left to right |
|            | shift                      | `<<` `>>` `>>>`                                                                  | left to right |
|            | bitwise AND                | `&`                                                                              | left to right |
|            | bitwise XOR                | `^`                                                                              | left to right |
|            | bitwise OR                 | `|`                                                                              | left to right |
|            | type check                 | `as` `as?`                                                                       | left to right |
|            | range                      | `..`, `..=`                                                                      |               |
|            | infix function             | `simpleIdentifier`                                                               |               |
|            | named checks               | `in` `!in` `is` `!is`                                                            |               |
|            | comparison                 | `<` `>` `<=` `>=`                                                                |               |
|            | equality                   | `==` `!=` `===` `!==`                                                            |               |
|            | logical and                | `&&`                                                                             | left to right |
|            | logical or                 | `||`                                                                             | left to right |
|            | logical implication        | `->` `<-`                                                                        | left to right |
|            | spread (in function calls) | `...`                                                                            |               |
| Lowest     | assignment                 | `=` `*=` `/=` `~/=` `%=` `+=` `-=` `&=` `|=` `^=` `&&=` `||=` `<<=` `>>=` `>>>=` | right to left |

Spread in function calls:

```rust
let tuple = (x, y)
Point(tuple.x, tuple.y)
Point(...tuple)
```

TODO: ??/?:, :, ternary, `??=`

### Labels

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

### 9.1. If

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

### 9.2. When/Match
### 9.3. Try?

## 10. Statements

### 10.1. For?
### 10.2. While?
### 10.3. Do-While?
### 10.4. Rethrow?
### 10.5. Return
### 10.6. Labels
### 10.7. Break
### 10.8. Continue
### 10.9. Yield & Yield-Each
### 10.10. Assert

## 11. Modules & Scripts

- `use`: import a module
- `public use`: import & export a module


## 12. Types

### 12.1. Function Types

```kotlin
[named] R.(T1 t1, T2 t2, …, Tn tn = dn) -> T
```

### 12.2. Value Constraints

```kotlin
fun a(Pair<Int, Int> pair)
    where pair.first <= pair.second {
  
}
```


## 13. Reference

### 13.1. Comments

