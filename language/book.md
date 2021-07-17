# The Candy Book

> Note: This document is not up-to-date.

## Foreword

Welcome to the Candy Book, an introductory book to Candy!
Candy is a simple, extensible language enabling everyone to focus on getting things done.

This book assumes you're familiar with another programming language already, but doesn't make any assumptions about which one.

## Getting started

TODO(later, marcelgarus): Installing Candy

### Hello world

Create a new directory and inside it, a file `main.candy` with the following contents:

```kotlin
fun main() {
  print("Hello, world!")
}
```

You can run this code by executing `candy main.candy` in a command line.
If it printed `Hello, world!`, you've now officially written your first Candy program. Congratulations!

Here's what happened in more detail. Here's the first part:

```kotlin
fun main() {

}
```

These lines define a function in Candy. The function called `main` is special – it marks the entry point of your program.
Functions can also return something or accept parameters – these would go inside the parentheses.
If a function is called, the code inside its curly braces is executed.

There's also a shortcut for writing small functions that only do one thing: Instead of curly braces, you can also use an arrow `=>` followed by the code directly.
The following code has the same effect:

```kotlin
fun main() => print("Hello, world!")
```

Inside the main, there's the following line:

```kotlin
print("Hello, world!")
```

The built-in `print`-function simply prints its argument to the command line.

## Variables
