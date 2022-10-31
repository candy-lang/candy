# Candy

Candy aims to be a sweet programming language that is **robust**, **minimalistic**, and **expressive**.
In particular, this means the following:

- **Candy is robust.**
  Candy aims to make it easy to write programs that do the correct thing and handle all edge cases.
  That's mainly achieved by providing good tooling so that the feedback loop is fast.
- **Candy is minimalistic.**
  The language itself is simple and can be explained in a few minutes.
  Candy intentionally leaves out advanced concepts like types, offsetting that with good editor tooling.
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
  - [Ports](#ports)
  - [More?](#more)
- [Variables](#variables)
- [Functions](#functions)
- [Modules](#modules)
- [Comments](#comments)
- [Panics](#panics)
- [Needs](#needs)
- [Pattern Matching](#pattern-matching)
- [Concurrency](#concurrency)
  - [Channels](#channels)
- [Packages](#packages)
- [Environment and Capabilities](#environment-and-capabilities)
- [Interoperability With Other Languages](#interoperability-with-other-languages)
  - [Add to the Environment](#add-to-the-environment)
  - [Contain Pure Code](#contain-pure-code)
- [Deploying code](#deploying-code)

## Basic Syntax

Candy's syntax is inspired by Elm and Haskell.

Source code is stored in plain text files with a `.candy` file extension.

[Comments](#comments) start with `##` and end at the end of the line:

```candy
## This is a comment.
```

> One `#` is also a comment, but a doc comment for the item above it. See [Comments](#comments) for more info on that.

Naming rules are similar to other programming languages.
Identifiers start with a lowercase letter and may contain letters or digits.
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
All values are immutable – once created, they do not change.
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

Finally, you can use curly braces (`{}`) containing a text value to insert it into the text at that position.

```candy
"Hello!"
"A somewhat
  long
  text."
'"This is a meta text, where you can use " inside the text."'
''"This is a double-meta text, allowing you to use "' inside it without ending it."''
"Some {interpolation}."
'"In meta texts, {{interpolation}} requires more curly braces; otherwise, the values are {not interpolated}."'
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
  "TextKey": 4,
  3: 2,
]
```

Omitting keys is equivalent to using zero-indexed numbers as keys. You can also add fields with explicit keys after these:

```candy
[Hello, World, Foo: Bar]

# equivalent:
[0: Hello, 1: World, Foo: Bar]
```

To lookup a key that is a symbol, you can use the dot syntax:

```candy
foo = [Name: "Candy", Foo: 42]
foo.name  # "Candy"
```

TODO: Modifying structs. Original idea: `{ Name: "Marcel", Age: 21 }` copied using `{ original | Name: "Jonas" }`

### Closures

Closures are pieces of code that can be executed.

```candy
identityFunction = { argument -> argument }
longClosure = { foo ->
  ...
}
```

### Ports

Ports allow you to interact with a [channel](#concurrency).
There are receive ports and send ports to receive and send data from a channel, respectively.
See the [Concurrency](#concurrency) section for more information.

### More?

TODO: Tuples? Tags? Sets?

- sets: Clojure has `%{ value }`
  - or like Toit? `{hey, you, there}` for set, empty map is `{:}`

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

Variables at the file's top level are visible to the module system (“public”) if they are declared using `:=`.
All other variables are local.

Declaring a variable with the same name as another simply shadows that variable, though that's not allowed for public variables.
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

Functions can either be defined using closure literals or by writing them as parameterized variables with arguments in front of the `=` or `:=`.
Both representations are equivalent with respect to what they do during runtime.

```candy
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

You can also split arguments across multiple lines using indentation.
This allows you to omit parentheses for nested calls.

```
foo = add
  subtract 5 3
  multiply
    logarithm 5
    divide 8 4
```

TODO: Piping

## Modules

For bigger project it becomes necessary to split code into multiple files.
In Candy, *modules* are a unit of composition.
Modules are self-contained units of code that choose what to expose to the outside world.

Modules correspond either to single candy files or directories containing a single file that is named just `_.candy`.
For example, a Candy project might look like this:

```
main.candy
green/
  _.candy
  brown.candy
red/
  _.candy
  yellow/
    _.candy
    purple.candy
  blue.candy
```

This directory structure corresponds to the following module hierarchy:

```
main        # from main.candy
green       # from green/_.candy
  brown     # from green/brown.candy
red         # from red/_.candy
  yellow    # from red/yellow/_.candy
    purple  # from red/yellow/purple.candy
  blue      # from red/blue.candy
```

Inside a module, top-level variable definitions can use a `:=` instead of `=` to export a variable.
In each module, there automatically exists a `use` function that will import other modules from the module tree.
You pass it a text that describes what module to import.

```
# inside red/yellow/_.candy

foo = use ".purple"  # imports the purple child module
foo = use "..blue"   # imports the blue sibling module
foo = use "...green" # imports the green parent module
```

Each additional dot at the beginning symbolizes a navigation one level up.
The possible multiple dots are followed by the name of the module to import.
Note that you can't navigate further than one level in – for example, the `yellow` module can't import the `brown` module, only its parent module `green`.

The `use` call evaluates the given module and returns a struct containing all its exported definitions (variables and functions using `:=`).

```candy
# inside green/brown.candy

foo = 3
bar := foo
baz a := a
```

```candy
# inside green/_.candy

brown = use ".brown"

# equivalent during runtime:
brown =
  foo = 3
  bar = foo
  baz a = a
  [
    Bar: foo,
    Baz: bar,
  ]

# equivalent during runtime:
brown = [
  Bar: 5,
  Baz: { a -> a },
]
```

The `useAsset` also allows you to import arbitrary non-Candy files that are part of your module hierarchy.
In some cases, it makes more sense to express some data in other formats.
For example, you might want to store user-facing translations for your program in a JSON file.

```
main.candy
translations.json
```

```
# inside main.candy

translations = json.parse (useAsset "..translations.json")
translations.helloWorld
```

Changes to these files are also tracked by the Candy tooling and autocompletions and hints will update accordingly.

## Comments

TODO: Write something including doc comments

## Panics

Candy programs can panic, causing them to crash.
Contrary to crashes in other programming languages, it's always programmatically clear which part of the code is at fault.

For example, in Rust, if a function panics, you have to look at its documentation to understand if the panic is your fault or not:
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

  # `product` is a parameterized variable, so it needs to handle every input.
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

Here are some recommended guidelines for writing reasons:

- For `needs` that only check the type, you typically don't need a reason. 
- Try to keep the reason short and simple.
- Phrase the reason as a self-contained sentence, including a period at the end.
- Write concrete references such as function or parameter names in backticks.
- Prefer concepts over concrete functions. For example, write "This function needs a non-negative int." rather than "This function needs an int that `isNonNegative`." – after all, users can always jump to the `needs` itself.
- Consider also highlighting what is wrong with the input rather than just spelling out the needs.
- Consider starting new sentences in long reasons.
- Consider special-casing typical erroneous inputs with custom reasons.

The editor tooling will analyze your functions and try them out with different values.
If an input crashes in a way that your code is at fault, you will see a hint.

```candy
mySqrt a =               # If you pass `a = -1`,
  needs (core.int.is a)  # this needs succeeds because `core.int.is -1 = True`,
  core.int.sqrt a        # but calling `core.int.sqrt -1` panics: `sqrt` only works on non-negative integers. You might want to check out the `ComplexNumbers` package.
```

## Pattern Matching

Candy supports structural pattern matching using the match operator `%`.

```candy
bar = foo 5 %
  [Ok, value] -> ...
  [Error, errorValue], core.int.isEven errorValue -> ...
  _ -> ...
```

Here, each indented line after the match operator represents a match case.
Each case can match based on the pattern as well an optional condition separated by a comma.
The first matching case is executed.
If no case matches, your code panics.

If you're sure about the structure of a value, you can also use patterns on the left-hand side of an assignment.
These are called irrefutable patterns.
Again, if the pattern doesn't match, the code panics.

```candy
[a, b] = myList
core.int.add a b

# actually a pattern:
foo = bar 5
```

## Concurrency

Candy supports a lightweight version of threads called *fibers*.
To enforce structured concurrency, they can only be spawned using a special concurrency object called the *nursery*.
In the following code, the surrounding call to `core.parallel` only exits when all fibers inside have completed.

Note: The actual `print` works differently, using [capabilities](#environment-and-capabilities).

```candy
core.parallel { nursery ->
  core.async nursery { print "Banana" }
  core.async nursery { print "Kiwi" }
  # Banana and Kiwi may print in any order
}
print "Peach"  # Always prints after the others
```

This way, if you call a function, you can be sure that it doesn't continue running code in the background even after it returns.
The only exception is if you pass it a nursery, which it can use to spawn other fibers.

### Channels

Channels can be used to communicate between fibers.
You can think of a channel like a conveyer belt or a pipe:
It has a *send end*, which you can use to put messages into it, and it has a *receive end*, which you can use to read messages from it.
A channel also has a capacity, which indicates how many messages it can hold simultaneously.

```candy
channel = core.channel.create 5  # creates a new channel with capacity 5
sendPort = channel.sendPort
receivePort = channel.receivePort

core.channel.send sendPort Foo
core.channel.send sendPort Bar
core.channel.receive receivePort  # Foo
core.channel.receive receivePort  # Bar
```

There is no guaranteed ordering between messages sent by multiple fibers, but messages coming from the same fiber are guaranteed to arrive in the same order they were sent.

All channel operations are blocking.
Thus, channels also function as a synchronization primitive and can generate *backpressure*.

```candy
core.parallel { nursery ->
  channel = core.channel.create 3
  core.async nursery {
    loop {
      core.channel.send channel.sendPort "Hi!"
    }
  }
  core.async nursery {
    loop {
      print (core.channel.receive channel.receivePort)
    }
  }
}
```

In this example, if the printing takes longer than the generating of new texts to print, the generator will wait for the printing to happen.
At most 3 texts will exist before being printed.

## Packages

TODO: Write something

## Environment and Capabilities

At some point, your Candy program needs to have side effects – otherwise, it's just heating up your CPU.
To model that, the `main` function receives an `environment`, which is a struct containing platform-specific values, including channels.

For example, on desktop platforms, the environment looks something like this:

```candy
[
  Stdin: <channel receive port>,
  Stdout: <channel send port>,
  WorkingDirectory: ...,
  Variables: [
    ...
  ],
  Arguments: ...,
]
```

Channels also function as *capabilities* here:
If you don't pass the stdout channel to a function, there's no way for it to print anything.
This is especially useful for more "powerful" capabilities like accessing the file system or network:
When using a package, without reading its source code, you can be confident that it won't delete your files under some special circumstances.

If a function expects a stdout channel, there's no way it can tell if you gave it another channel that you just created.
You could for example process the output of the function, filter some information out, and forward the rest to the real stdout channel.

## Interoperability With Other Languages

Candy has no plans to directly support Foreign Function Interfaces (FFI) to communicate with other code.
The reason is that doing so inherently breaks the isolation of code.

Depending on the use case, we offer two alternative options:

### Add to the Environment

If you develop for a new platform or want to enable more functionality in the native platform, we will have some way of plugging a new part of native code into the runtime that can make its own capabilities available on the environment passed to `main`.

For example, on a microcontroller, the stdout capability doesn't make sense.
Instead, you might have a pin capability that allows you to modify the voltage of the hardware pins.

### Contain Pure Code

If you want to use existing code that implements pure functions, it can make sense to compile the existing code into WebAssembly (WASM).
You can put the resulting WASM module in your Candy module hierarchy, call `useAsset` with that file, and pass the data to a WASM runtime that we'll write in Candy when we get to it.

This approach moves the native code entirely into the Candy domain, so the Candy compiler can also reason about the WASM code.
For example, if you call a function of your WASM module only with inputs known at compile-time, the Candy compiler may execute those calls directly and not include the original WASM code in the binary at all.

## Deploying code

This chapter is especially experimental and spitball-y.

Next to the interpreted VM, we plan to compile to LLVM or WASM.

Similar to how Zig build scripts work, we may support having a `build.candy` file that contains information about how to compile and optimize your code.

For instance, to build a project for some custom platform, you may need to combine several native code libraries and integrate those with the Candy code by making some capabilities available via the [environment](#environment-and-capabilities).

Regarding optimization, one idea we had is to let you provide a custom scoring function in the build script instead of having binary options like "optimize for speed" or "optimize for performance".
This scoring function could get used by the optimizer to choose which paths to take.
For example, you could formulate a build where you're okay with a resulting binary blowup of 1  KiB per 10 ms of saved time in some annotated performance-critical section.
Or, when developing for an embedded device with limited storage capacity, you might want to generate a binary that fits within the limit but is otherwise as fast as possible.
