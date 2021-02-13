# 🍭 Candy

A sweet programming language, mainly inspired by Kotlin, Rust and Dart.

[Join our Discord server.](https://discord.gg/5Vr4eAJ7gU)

## Quick introduction

* **Rust-like type system with classes, enums, and traits.**
  You can implement traits for existing types, even the ones from the standard library.
  ```rust
  trait Foo {
    fun foo(): String
  }
  impl Bool: Foo {
    fun foo(): String { "blub" }
  }
  ```
* **Immutability.**
  Use `var` for reassignable variables, otherwise `let`.
  That works both in local scope and in classes.
* **Trailing lambdas.**
  Use brackets to define inline lambdas:
  ```rust
  let someLambda = {
    print("Hello, world!")
  }
  someLambda()
  ```
  Just like in Kotlin, if a lambda is the last parameter passed to a function, you can write it behind the parentheses of the parameter list.
  You can even omit the parentheses completely if the lambda is the only parameter:
  ```rust
  someFunction { a, b -> a + b }
  ```
  If it's inferred from the context that the lambda accepts a single parameter and you don't specify one, it's bound to the `it` variable.
* **Indentation-aware.**
  Candy uses indentation to find out what's an expression.
  We still keep brackets and parentheses for clarity – it's a very important information that you leave the scope of a long class.
  Rather, we use indentation so that dots for navigation and semicolons between expressions are no longer necessary:
  ```swift
  let candy = programmingLanguages
    where { it name == "Candy" & it age < 3 years }
    map { it specification }
    single()
  ```
* **Keep magic to a minimum.**
  All types and functions are defined in Candy (although some are marked `builtin`, which means the compiler implements them).
  For example, here's the definition of `Bool`:
  ```rust
  enum Bool { True, False }
  let true = Bool True
  let false = Bool False
  ```
  Most "control-flow structures" are just functions – take the following code:
  ```rust
  let foo = if (true) { 1 }  // foo = Some(1)
  let bar = if (false) { 2 } // bar = None
  let baz = if (true) { 3 } else { 4 } // baz = 3
  ```
  `if` is just a `builtin` function that takes a `Bool` and another function, which is provided as a trailing lambda.
  And the `Maybe<T>` that it returns has an `else` function that returns just a `T` by defaulting to the result of the given function if it's `None`.
  Tadaa! We just built a fancy if-else construct without baking it into the language.  
  Similarly, `loop` is a `builtin` function and instead of `for`-loops, we just add a `do` function on `Iterable`:
  ```rust
  0..3 do {
    print("Hello, {it}!")
  }
  ```
  Note we also want to add pattern matching on the language-level, but we didn't think about how to do that yet, because other features were more urgent.
* **Be modular.**
  All elements are private by default. The `public` modifier makes functions, properties and types available if the module gets imported.
  The module system itself is very much inspired by Rust – folders, files, and explicit `module` declarations can all be used interchangably. Also, defining types creates implicit modules.  
  Our imports are called *use-lines*, because they look like this:
  ```rust
  use MongoDB
  use LaTeX
  use .MySubmodule
  use ..SiblingModule
  use ....WayUpTop APublicSubmodule
  ```
  No dots indicate other packages that were declared in the candyspec file.  
  Dots indicate local modules from the same package. The number of dots indicates how much to go up in the hierarchy before traversing down.
  A nice side-effect of the dot syntax is that paths are automatically canonical, `../foo/../bar` is not possible.
* **Enforce conventions.**
  Packages and modules (including types) are uppercased.
  Builtin types like `Bool` or `UInt8` are no exception.
  Uppercasing packages and organizations is also nice because they have proper names: Want to depend on `Google Maps` or `Rust FFI`? The capitalization matches the actual project name.
  For ease-of-use, package names are case-insensitive and autoformatted to the canonical capitalization.
* **Keywords.**
  You can define your own keywords, which are similar to macros in other languages.
  There's a `keyword` keyword to do that:
  ```rust
  keyword let foobar = ... // A code transformer
  foobar class Baz { ... }
  async fun doStuff() { ... }
  ```
* **Tooling.**
  Of course, we want to offer great tooling including syntax highlighting, autocomplete based on the popularity of code and its documentation, a package ecosystem etc.  
  We aim to not have one global namespace for packages, but publish them hierarchically under people/organizations. Popular packages can opt-in to also being available globally. Very popular packages may be auto-imported, i.e. if you try to use the `jsonDecode` function, you'll get the autocomplete option to depend on the `JSON` package.

## The current state

We implemented a first version of the Candy compiler in Dart.
Currently, we're working on making Candy selfhosting, so we're working on creating the Candy compiler in Candy.

Some features are not implemented yet and will be added later (most notably indentation-based expressions and trailing lambdas). The first version also still contains a lot of magic like `if` and `while` and the type system is very weak.

Regarding tooling, we already have a language server that provides syntax highlighting and go-to-definition.

## How to use Candy

1. Download the [latest release bundle](https://github.com/JonasWanke/candy/releases/latest).
2. Extract the files:
   * `candy2dart.exe`: the compiler
   * `lsp-server.exe`: the Language Server
   * `vscode-extension.vsix`: the VS Code extension
   * `candy`: the folder containing the standard library
3. [Install](https://code.visualstudio.com/docs/editor/extension-gallery#_install-from-a-vsix) the VS Code extension.
4. In the settings (<kbd>ctrl</kbd> + <kbd>,</kbd>), adjust the paths in the Candy section:
   * The Candy Path should point to the standard library.
   * The Language Server Command should point to the `lsp-server.exe`.
5. Open a project.
6. Create the following:
   * a `candyspec.yml` file with a `name: something` field
   * a `src` folder
   * a `main.candy` inside the `src` folder with a `main` function
7. Execute code actions (by default, that's <kbd>ctrl</kbd> + <kbd>.</kbd>).
8. Select "Build".
9. If you actually want to run Candy code, also [install Dart](https://dart.dev/get-dart).
