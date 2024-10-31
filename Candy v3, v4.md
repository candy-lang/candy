> [!CAUTION] Parts of this document are out of date.

Here we go again.

## v3

```python
myValue: myType = Abc


# Idea for a new language.
# Idea: Dependent types. At compile-time, there is no distinction between values
# and types. The only limitation at runtime (after constant folding):
# The types of all variables need to be known.

let fruit = [Name: "Banana", Color: Yellow]
let color := Yellow | Red

let f1: [name: "Banana", Color: Yellow] = fruit
let f2: fruit = fruit
let f3: [name: text, Color: color] = fruit
let fruit = [name: text, Color: color]
let f4: fruit = fruit

# type inference
foo =
  bool = True
  bool = False

boolean = True | False
true: boolean = True
false: boolean = False

int = type @opaque(8)

fruit2 = type([name: text, color: color]) # nominal type (equality = this code position + content)
fruit3 = type([name: text, color: color]) # nominal type (equality = this code position + content)
f5: fruit2 = fruit
f6: fruit3 = fruit
# f7: fruit3 = f5 # Doesn't work! Can't assign from fruit2 to fruit3.

toHex(color: color): text =
  color %
    Yellow => "#ffff00"
    Red => "#ff0000"

# red = Red.toHex() # Doesn't work! toHex(Yellow) not defined.
red = {Red as color}.toHex() # works

let maybe(t: type(_)) -> type := {
  type { Some: T | None }
}

foo(bar: maybe(t)) -> type =
  bar %
    Some t -> t
    None -> type(@opaque(0))

unwrap(aMaybe: maybe(_)) -> _ =
  aMaybe %
    Some x -> x
    None -> panic("None")

add(t: type, aList: list(t), item: t) -> list(t) =
  aList + item

myList.add(myItem, t = abc)

foo(bar: (int) -> unit) -> type =
  type(@opaque(0))

# Types
intType = int
textType = text
structType = [name: text, color: color]
enumType = Foo int | Bar
arrayType = array int


[int, list] = use "Core"
structType.name()




foo bar: 1 baz: 2 | 3 =

list { 1, 2, 3 } # Inspired by Swift's result builders

array T = <builtin>
builtinArrayLength (a: array _) : int = <builtin>

slice T = []

uint = type int where
  needs (int >= 0)

date = type [year: int, month: int, day: int] where
  needs (month >= 1 and month <= 12)
  monthLength = …
  needs (day >= 1 and day <= monthLength)

dateOrTime = type Date date | Time time
```

```text
# date.candy
duration = use ".duration"

self := type [year: int, month: int, day: int] where
  needs (month >= 1 and month <= 12)
  monthLength = …
  needs (day >= 1 and day <= monthLength)

format (date: self) := …
add(date: self, aDuration: duration) :=
  needs(aDuration.days >= 0)
  date.format().print()
  …

# main.candy
date = use ".date"

myDate = date([year: 2020, month: 2, day: 29])
myDate.add([days: 1])

foo = date
```

- UFCS
- types:
  - builtin: int, text, structs, array T, enums
  - are values, but must be known at compile time
  - “generic” via functions that return types (like Zig)

### TODOs

- [ ] function overloading

## v4

```rust
pub struct MyStruct[A: Equals] {
  pub a: Int
  pub b: A
} where {
  let foo = 2 * a
  needs(a > 0, "")
}
pub struct Uint {
  value: Int
} where value >= 0
enum MyEnum {
  foo,
  bar: Int,
}

fun foo(a: Int, b: Int) Int {
  a + b
}
fun foo[A](a: A) A {
  a
}
fun foo[A](A) A {
  a
}
fun MyStruct.new() Self {
  a
}

pub struct Nothing {}
pub enum Never {}

fun main() {
  # TODO: separate syntax for instantiation?
  let a = MyStruct[Int](1, 2)
  let b = MyStruct[Text](1, "b")
  let c = MyEnum.foo
  let d = MyEnum.bar(1)

  if(
    condition,
    () {
      foo(1, 2)
    },
    () {
      foo(2, 3)
    }
  )

  switch   {
    true =>
  }
}

# TODO: imports
# TODO: pattern matching instead of `switch`
# TODO: maybe associated types
# TODO: maybe tuples
# TODO: default parameters

# TODO: change function type to `Fun (T) R`
fun map[T, R](list: List[T], f: Fun[T, R]) List[R] {}

fun average[T: Number](list: List[T]) T {
  needs(!list.is_empty())
  # TODO: maybe implicit `it`
  # list.reduce(T.zero(), (a: T, b: T) { a + b })
}

trait Number {
  fun zero() Self
}

trait BinaryPlus[Rhs, Result] {
  fun add(self, rhs: Rhs) Result
  fun add_static(rhs: Rhs) Result
}

trait Add {
  fun add(self, rhs: Self) Self
}
impl[T: Add] T: BinaryPlus[T, T]


impl Int: Add {
  fun add(self, rhs: Int) Int {
    self.builtin_add(rhs)
  }
}

trait Writable {
  fun write_to[W: Writer](writable: Self, writer: W)
}
trait Writer {
  fun write_bytes(writable: Self, bytes: Bytes)
}
fun write[W: Writer, T: Writable](writer: W, value: T) {
  value.write_to(writer)
}
```

```dart
abstract class Calendar<Year, Month extends Enum> {}
class Date<C extends Calendar<Y, M>, Y, M extends Enum> {
  final Year year;
}

typedef GregorianDate = Date<Gregorian>;
```
