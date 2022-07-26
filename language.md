# Candy

Candy aims to be a sweet programming language that is **robust**, **minimalistic**, and **expressive**.
In particular, this means the following:

- **Candy is robust.**
  Candy aims to make it easy to write programs that do the correct thing and handle all edge cases.
  That's mainly achieved by providing good tooling so that the feedback loop is fast.
- **Candy is minimalistic.**
  The language itself is simple and can be explained in a few minutes.
  Candy intentionally leaves out advanced concepts like types in favor of good editor tooling.
- **Candy is expressive.**
  You can be flexible with how you model your data in Candy.
  We aim to have a reasonable concise syntax to express common patterns.

Candy aims to blur the line between compile-time and run-time, for example, by replacing compile-time types with edit-time fuzzing.

This document assumes you're familiar with other programming languages and covers most of what's planned for Candy.
Note that not all of the features described here are implemented or even finalized.

- [Basic Syntax](#basic-syntax)
- [Values](#values)
  - [Integers](#integers)
  - [Texts](#texts)
  - [Symbols](#symbols)
  - [Structs](#structs)
  - [Closures](#closures)
  - [More?](#more)
  - [Channel Ends](#channel-ends)
- [Variables](#variables)
- [Functions](#functions)
- [Modules](#modules)
- [Comments](#comments)
- [Panics](#panics)
- [Needs](#needs)
- [Pattern matching](#pattern-matching)
- [Concurrency](#concurrency)
- [Packages](#packages)
- [Environment](#environment)
- [Build system](#build-system)
- [Interoperability with other languages](#interoperability-with-other-languages)
- [Optimizing code](#optimizing-code)
- [Deploying code](#deploying-code)

## Basic Syntax

Candy's syntax is inspired by Elm and Haskell.

Source code is stored in plain text files with a `.candy` file extension.

Line [comments](#comments) start with `#` and end at the end of the line:

```candy
# This is a comment.
```

Naming rules are similar to other programming languages.
Identifiers start with a lowecase letter and may contain letters or digits.
Case is sensitive.

```candy
hi
camelCase
abc123
```

Newlines and indentation are both meaningful in Candy.
Newlines separate code expressions.
Indentation always consists of two spaces and is used to group several expressions in [scopes](#variables).
Scopes evaluate to their last expression.

```candy
foo = 42
bar =
  baz = 3
  5
# bar is now 5
```

## Values

All data in your program is composed of values.
Values can be created through literals, expressions that evaluate to a value.
All values are immutable – once created, they do not change.
The number `3` is always the number `3`.
The string `"frozen"` can never have its character array modified in place.

There only exist few types of values in Candy.

### Integers

Integers are arbitrarily large, whole numbers.
You write them in decimal.

```candy
0
42
123456789012345678901234567890
```

TODO: Different radixes

### Texts

Texts are Unicode strings.
They start and end with double quotes (`"`) and can span multiple lines if they're indented.

You can also start texts with any number of single quotes (`'`) followed by a double quote (`"`).
This so-called meta-text can only be ended with a double quote and the same number of single quotes that it started with.

Finally, you can use curly braces (`{}`) containing a value to insert a stringified version of the value into the string at that position.

```candy
"Hello!"
"A somewhat
  long
  text."
'"This is a meta text, where you can use " inside the text."'
''"This is a double-meta text, allowing you to use "' inside it without ending it."''
"Some {interpolation}."
'"In meta texts, {{interpolation}} requires more curly braces, otherwise the values are {not interpolated}."'
```

### Symbols

Symbols are uppercase identifiers that can only be compared for equality.

```candy
True
Green
Foo
```

### Structs

Structs are mappings from keys to values (also known as dictionaries or hash maps in other languages).

```candy
[
  Name: "Candy",
  Foo: 42,
]
```

TODO: Struct access using dot

TODO: Modifying structs. Original idea: `{ Name: "Marcel", Age: 21 }` copied using `{ original | Name: "Jonas" }`

### Closures

Closures are pieces of code that can be executed.

```candy
identityFunction = { argument -> argument }
longClosure = { foo ->
  ...
}
```

### More?

TODO: Lists? Sets?

- lists: [1, 2, 3]
- sets: Clojure has %{ value }
  - or like Toit? {hey, you, there} for set, empty map is {:}

### Channel Ends

Channels ends allow you to interact with a [channel](#concurrency).
There are receive ends and send ends to receive and send data from a channel, respectively.

## Variables

Variables are named slots for storing values.
You define a new variable using the equals sign `=`, like so:

```candy
foo = "Hello!"
```

This creates a new variable `foo` in the current scope and initializes it with the result of the expression following the `=`.
Once a variable has been defined, it can never be reassigned.
You can access variables by their name.

```candy
foo = "Hi!"
bar = foo  # bar = "Hi!"
```

Variables only exist until the end of the scope they're defined in.

```candy
foo =
  bar = hello  # error because hello doesn't exist yet
  hello = 5
  4
bar = hello  # error because hello doesn't exist anymore
```

Variables at the top level of a file are visible to the module system.
All other variables are local.

Declaring a variable with the same name as another simply shadows that variable:
From that point forward, the name will refer to the new variable.
Note that this is different from reassigning to a variable.

```candy
foo = 5
foo = 3
# there is no way to get to the 5
bar =
  foo = 4
# here, foo is still 3
```

## Functions

Functions can either be defined using closure literals or by writing them as parameterized variables with arguments in front of the `=`.
Both representations are equivalent with respect to what they do during runtime.

```candy
# Both these definitions are equivalent.
identity = { a -> a }
identity a = a
```

You can call functions by writing their name followed by the arguments.
Grouping using parentheses is only necessary if you have nested calls.

```
five = identity 5
five = identity (identity 5)
error = identity identity 5  # error because the first `identity` is called with two arguments
```

TODO: Piping

## Modules

For bigger project it becomes necessary to split code into multiple files.
In Candy, *modules* are a unit of composition.
Modules are self-contained units of code that choose what to expose to the outside world.

Modules correspond either to single candy files or directories containing a single file that is named just `.candy`.
For example, a Candy project might look like this:

```
main.candy
green/
  .candy
  brown.candy
red/
  .candy
  yellow/
    .candy
    purple.candy
  blue.candy
```

This directory structure corresponds to the following module hierarchy:

```
main        # from main.candy
green       # from green/.candy
  brown     # from green/brown.candy
red         # from red/.candy
  yellow    # from red/yellow/.candy
    purple  # from red/yellow/purple.candy
  blue      # from red/blue.candy
```

Inside a module, top-level variable definitions can use a `:=` instead of `=` to export a variable.
In each module, there automatically exists a `use` function that will import other modules from the module tree.
You pass it a text that describes what module to import.

```
# inside red/yellow/.candy

foo = use ".purple"  # imports the purple child module
foo = use "..blue"   # imports the blue sibling module
foo = use "...green" # imports the green parent module
```

Each additional dot at the beginning symbolizes a navigation one level up.
The possible multiple dots are followed by the name of the module to import.
Note that you can't navigate further than one level in – for example, the `yellow` module can't import the `brown` module, only its parent module `green`.

The `use` call evaluates the given module and returns a struct containing all its exported definitions (variables and functions using `:=`).

```candy
# inside green/brown.candy

foo = 3
bar := foo
baz a := a
```

```candy
# inside green/.candy

brown = use ".brown"

# equivalent:
brown =
  foo = 3
  bar = foo
  baz a = a
  [
    Bar: foo,
    Baz: bar,
  ]

# equivalent:
brown = [
  Bar: 5,
  Baz: { a -> a },
]
```

TODO: `useAsset`

## Comments

TODO: Write something including doc comments

## Panics

Candy programs can panic, causing them to crash.
Contrary to crashes in other programming languages, it's always programmatically clear which part of the code is at fault.

For example, in Rust, if a function panics you have to look at its documentation to understand if the panic is your fault or not:
The panic of `None.unwrap()` is not `unwrap`'s fault, while a panicking call to `my_complicated_algorithm(input)` may well be the fault of the algorithm itself.

In Candy, code panics if a `needs` is not satisfied.

## Needs

Instead of types, Candy has a special function-like primitive called `needs`.
Similar to `assert`s in other languages, `needs` accept a symbol that has to be either `True` or `False`.

Functions can use `needs` to specify requirements for their arguments.
Essentially, by defining `needs`, a function can *reject* certain inputs and mark the crash as the fault of the caller.
For example, here's a function that only accepts integers:

```candy
core = use "Core"

foo a =
  needs (core.int.is a)
  a

bar = foo 5  # foo = 5
bar = foo A  # error
```

Note that there is a difference between functions written as parameterized variables (`foo a = a`) and functions written as closures (`foo = { a -> a }`).
`needs` always refer to the surrounding *parameterized variable*.
Consequently, closures can't reject inputs, but they also don't promise that they can handle every input correctly.

```candy
foo a =
  needs (core.int.is a)

  # `product` is a parameterized variable, so it needs to handle every input
  product b =
    needs (core.int.is b)
    core.int.multiply a b

  (core.range 0 10) | core.iter.forEach { b ->
    # If this needs fails, this is the fault of the caller of `foo`.
    needs (core.int.lessThan (product a b) 12)
  }
```

Optionally, you can pass a reason to the `needs` function that describes why your function requires the condition to hold.

```candy
foo a =
  needs (core.int.is a) "life's not fair"

foo Hey  # Calling `foo Hey` panics because life's not fair.
```

The editor tooling will analyze your functions and try them out with different values.
If an input crashes in a way that your code is at fault, you will see a hint.

```candy
mySqrt a =               # If you pass `a = -1`,
  needs (core.int.is a)  # this needs succeeds because `core.int.is -1 = True`,
  core.int.sqrt a        # but calling `core.int.sqrt -1` panics because sqrt only works on non-negative integers. If you think this should be different, check out the `ComplexNumbers` package.
```

## Pattern matching

TODO: Write something

TODO: Irrefutable and refutable patterns

## Concurrency

Candy supports a lightweight version of threads called *fibers*.
To enforce structured concurrency, they can only be spawned using a special concurrency object called the *nursery*.
In the following code, the surrounding call to `core.parallel` only exits when all fibers inside have completed.

```candy
foo = { print ->
  core.parallel { nursery ->
    core.fiber.spawn nursery { print "Banana" }
    core.fiber.spawn nursery { print "Kiwi" }
    # Banana and Kiwi may print in any order
  }
  print "Peach"  # Always prints after the others
}
```

This way, if you call a function, you can be sure that it doesn't continue running code in the background even after it returns.
The only exception is if you pass it a nursery, which it can use to spawn other fibers.

TODO: Channels

## Packages

TODO: Write something

## Environment

At some point, your Candy program needs to have side-effects – otherwise, it's just heating up your CPU.
To model that, the `main` function receives an `environment`, which is a struct containing platform-specific values, including channels.

For example, on desktop platforms, the environment looks something like this:

```candy
[
  Stdin: <receive end>,
  Stdout: <send end>,
  Variables: [
    ...
  ],
  Arguments: ...,
]
```

TODO: Write about permissions

## Build system

TODO: Write something

## Interoperability with other languages

TODO: Write something

## Optimizing code

TODO: Write something

## Deploying code

- VM
- LLVM
- WASM
