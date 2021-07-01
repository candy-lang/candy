This is not the whole book, but only a short cheatsheet for people coming from other languages.

# Literals

There are only few literals:

* **Strings** are defined using double quotes: `"Hello, world!"`
* **Int**s (integers) are defined by just typing the number: `42`
* **Lambda literals** are defined by writing code inside curly braces: `{ print("Hello, world!") }`

# Functions

In Candy, all executable code is inside functions.
Functions have to use lowercase names.
If a function doesn't take any parameters, you don't need to write parentheses after the name in the definition.
Each parameter needs to have a type and optionally, you can add a default parameter.
The last expression in a function is automatically returned.

```
fun main {
  print("Hello, world!")
}

fun fibonacci(n: Int): Int {
  if(n < 2) {
    0
  } else {
    fibonacci(n - 1) + fibonacci(n - 2)
  }
}

fun todo(foo: String = "Todo", bar: () -> Bool) {
  let someBool = bar
  panic(foo)
}
```

Parameter names are part of the function signature and if you call a function with named arguments, the order doesn't matter.
If the last argument is a lambda literal, you can also move that out of the parentheses of the parameter list.
Finally, if the parentheses don't contain anything, you can omit them.

These are all valid calls of the functions above:

```
main
fibonacci(5)
fibonacci(n = 12)
test(foo = "Foo", bar = { true })
test(bar = { false }, foo = "Foo")
test("Bar") {
  false
}
test {
  print("Hello.")
  true
}
```

# Types

In Candy, every variable has a static type.

These are the ways to define new types:

* **Tuples** like `(String, Int)`.
  Tuples always contain all their members.
  You can create instances using `("Foo", 2)` and access members using the functions `first`, `second`, etc.: `myTuple first`
* **Named tuples** like `(foo: String, bar: Int)`.
  Named tuples always contain all their members.
  You can create instances using `(foo = "String", bar = 42)` and access the members using their name: `myTuple foo`
* **Enum types** like `Foo | Bar String`.
  Enums define a type that is either one of the variant types.
  It's constructed by combining multiple variants using a pipe `|`.
  Each variant consists of a name (`Foo` and `Bar` in the example above) and optionally another type.
  You can create instances using `Bar("Hey")` in a context where the expected enum type is known.
* **Identity types** have an identity – you can't just swap the reference out with the type definition.
  You can define new identity types using the `type` keyword: `type Age = Int` and construct them using their name used as a function on an instance of the right type: `Age(42)`
  You can use the value function to get the inner value: `Age(42).value == 42`
* **Traits** are types defined by their behavior.
  They are useful to abstract from the underlying representation.
  You can use the `trait` keyword to define them:
  ```
  trait Foo {
    fun foo: String
    static fun bar: Int
  }
  ```
  The functions contained in a trait definition don't need a body.
  `static` methods are available on the type itself, not an instance of it.
  
  You can then make another type implement a trait using an `impl` block:
  ```
  impl Bool: Foo {
    fun foo: String { "Hello!" }
    static fun bar: Int { 42 }
  }
  ```
  Now, a `Bool` can be used anywhere where a `Foo` was expected.
  In particular, `true foo == "Hello!"` and `Bool bar == 42`, as long as `Foo` is included in the file.

# Modules

In Candy, modules are used to organize code.
A module usually corresponds to a file or folder, but you can define `module` blocks inside files as well.

For example, take the following file structure:

* MyModule (folder)
  * .candy
  * Foo.candy
  * Bar (folder)
    * .candy
    * Baz.candy

```
module MyModule {
  module Foo {}
  module Bar {
    module Baz {}
  }
}
```

# Fibers
