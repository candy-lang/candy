# 🍭 Candy

A sweet programming language that is robust, minimalistic, and expressive.

Candy aims to have excellent tooling – most language features are designed with tooling in mind.
Many languages have a strict separation between compile-time and runtime.
Candy blurs the line between those stages, for example, by replacing compile-time types with edit-time fuzzing.

## Quick introduction

- **Values are at the center of your computations.**
  Only a handful of predefined types of values exist:

  ```candy
  3                   # int
  "Candy"             # text
  Green               # symbol
  (Foo, Bar)          # list
  [Name: "Candy"]     # struct
  { it -> add it 2 }  # closure
  ```

- **Minimalistic syntax.**
  Defining variables and functions works without braces or keywords cluttering up your code.
  The syntax is indentation-aware.

  ```candy
  foo = 42
  println message =
    print message
    print "\n"
  println "Hello, world!"
  ```

- **Extensive compile-time evaluation.**
  Many values can already be computed at compile-time.
  In your editor, you'll see the results on the right side:

  ```candy
  foo = double 2  # foo = 4
  ```

- **Fuzzing instead of traditional types.**
  In Candy, functions have to specify their needs _exactly._
  As you type, the tooling automatically tests your code with many inputs to see if one breaks the code:

  ```candy
  foo a =             # If you pass a = 0,
    needs (isInt a)
    math.logarithm a  # then this panics: The `input` must be a positive number.

  efficientTextReverse text =
    needs (isText text)
    needs (isPalindrome text) "Only palindromes can be efficiently reversed."
    text

  greetBackwards name =                   # If you pass name = "Test",
    "Hello, {efficientTextReverse name}"  # then this panics: Only palindromes can be efficiently reversed.
  ```

To get a more in-depth introduction, read the [language document](language.md).

## Discussion

[Join our Discord server.](https://discord.gg/5Vr4eAJ7gU)

## The current state

We are currently implementing a first version of Candy in Rust.
We already have a language server that provides some tooling.

## Long-term TODOs

- Core
  - io
  - random
  - timing
  - environment variables
  - HTTP, UDP
- compiler
  - make functions independent of their order in top-level scope
  - patterns
  - improve pattern match panic messages: `[Foo, 1, {a}] = [Foo, 2, {A: B]]` could generate a message like `` Expected `[_, 1, _]`, got `[_, 2, _]`. ``
  - "type" proofs
  - fuzzing of the compiler itself
  - package root marker
  - package path dependencies
  - LLVM, WASM
- VM
  - multithreading
  - object deduplication
  - profiler
  - memory representation
    - inlining of ints/etc.
    - size of an object
    - heap management
- IDE support:
  - generate debug files
  - DAP (debug adapter protocol)
  - [ ] completion, completion resolve
  - [ ] hover
  - [ ] signatureHelp
  - [x] ~~declaration~~, definition, ~~typeDefinition~~
  - [ ] implementation
  - [x] references
  - [x] documentHighlight
  - [ ] documentSymbol
  - [ ] codeAction, codeAction resolve
  - [ ] codeLens, codeLens resolve, codeLens refresh
  - [ ] documentLink, documentLink resolve
  - [x] ~~documentColor, colorPresentation~~
  - [ ] formatting
  - [ ] rangeFormatting
  - [ ] onTypeFormatting
  - [ ] rename, prepareRename
  - [x] foldingRange
  - [ ] selectionRange
  - [ ] prepareCallHierarchy
  - [ ] callHierarchy incoming, callHierarchy outgoing
  - [x] semantic tokens
  - [x] ~~linkedEditingRange~~
  - [ ] moniker
- packages
  - stdin/out utilities such as a print method
  - files
  - logging
  - HTTP Server
  - Markdown
  - custom binary serialization
  - Cap'n Proto
  - DateTime?
  - SI?
  - MongoDB?
  - package manager
- online playground
- clean up repo (delete a bunch of stuff!)

## Short-term TODOs

- new name?
- add caching while compile-time evaluating code
- tags
- pattern matching
- add tests
- add a more lightweight tracer that only tracks stack traces
- optimize: inline functions
- minimize inputs found through fuzzing
- fuzz parser
- remove builtinPrint
- tracing visualization
- distinguish packages from normal modules
- complain about comment lines with too much indentation
- develop guidelines about how to format reasons
- disallow passing named closures as parameters? or auto-propagate caller's fault to called parameters?
- replace occurrences of `Id::complicated_responsibility()`
- fix usage of pipes in indented code such as this:

  ```candy
  foo
    bar | baz
  ## Currently, this is parsed as `baz (foo bar)`.
  ```

- more efficient argument preparation in LIR function call (so we don't have to push references if the evaluation order doesn't change conceptually)
- fix evaluation order of pipe expression by keeping it in the AST
- shorter ID formatting for generated debug files
- support destructuring in lambda parameters
- find references in patterns
- convert the readme todos into GitHub issues

## How to use Candy

1. Install [<img height="16" src="https://rust-lang.org/static/images/favicon.svg"> Rust](https://rust-lang.org): https://www.rust-lang.org/tools/install.
2. Configure Rust to use the nightly toolchain: `rustup default nightly`.
3. Install Rust's Clippy (a linter): `rustup component add clippy`.
4. Clone this repo.
5. Open the workspace (`compiler.code-workspace`) in VS Code.
6. In the VS Code settings (JSON), add the following: `"candy.languageServerCommand": "cargo run --manifest-path <path-to-the-candy-folder>/compiler/Cargo.toml -- lsp"`.  
   If you want to write code in 🍭 Candy (as opposed to working on the compiler), you should also add `--release` before the standalone `--`.
   This makes the IDE tooling faster, but startup will take longer.
7. Run `npm install` inside `vscode_extension/`.
8. Run the launch config “Run Extension (VS Code Extension)”.
9. In the new VS Code window that opens, you can enjoy 🍭 Candy :)
