# 🍭 Candy

A sweet programming language that is robust, minimalistic, and expressive.

Candy aims to have excellent tooling – most language features are designed with tooling in mind.
Many languages have a strict separation between compile-time and runtime.
Candy blurs the line between those stages, for example, by replacing compile-time types with edit-time fuzzing.

## Quick introduction

* **Values are at the center of your computations.**
  Only a handful of predefined types of values exist:
  ```
  3                   # int
  "Candy"             # text
  Green               # symbol
  [ Name: "Candy" ]   # struct
  { it -> add it 2 }  # closure
  ```
* **Minimalistic syntax.**
  Defining variables and functions works without braces or keywords cluttering up your code.
  The syntax is indentation-aware.
  ```
  foo = 42
  println message =
    print message
    print "\n"
  println "Hello, world!"
  ```
* **Extensive compile-time evaluation.**
  Many values can already be computed at compile-time.
  In your editor, you'll see the results on the right side:
  ```
  foo = double 2  # foo = 4
  ```
* **Fuzzing instead of traditional types.**
  In Candy, functions have to specify their needs *exactly.*
  As you type, the tooling automatically tests your code with many input to see if one breaks the code:
  ```
  foo a =             # If you pass a = 0,
    needs (isInt a)
    math.logarithm a  # then this fails because logarithm only works on positive numbers.

  efficientTextReverse text =
    needs (isText text)
    needs (isPalindrome text) "efficientTextReverse only works on palindromes"
    text

  greetBackwards name =                   # If you pass name = "Test",
    "Hello, {efficientTextReverse name}"  # then this fails because efficientTextReverse only works on palindromes.
  ```

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
  - lists
  - text interpolation
  - fibers, channels
  - patterns
  - pipe operator
  - "type" proofs
  - fuzzing of the compiler itself
  - package root marker
  - package path dependencies
  - performance
    - multithreading
    - object deduplication
    - profiler
    - memory representation
      - inlining of ints/etc.
      - size of an object
      - heap management
  - LLVM, WASM
- IDE support:
  - generate debug files
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

- make `PROJECT_DIRECTORY` non-global
- use BigInts
- minimize inputs found through fuzzing
- make condition whether to keep running more granular
- fuzz parser
- add caching to hints server
- implement fibers and nurseries
- implement channels
- remove builtinPrint
- support recursion
- tail call optimization
- new name?
- mini-heaps?
- parse calls with call as a receiver: `(foo Bar) Baz`. or do we even need the `CallReceiver`?
- parse function declaration with doc comment but no code
- complain about comment lines with too much indentation

## How to use Candy

1. Install Rust.
2. Clone this repo.
3. Open the workspace in VS Code.
4. In the VS Code settings (JSON), add the following: `"candy.languageServerCommand": "cargo run --manifest-path <path-to-the-candy-folder>/compiler/Cargo.toml -- lsp",`.
5. Run `npm install` inside `vscode_extension/`.
6. Run the launch config “Run Extension (VS Code Extension)”.
7. In the new VS Code window that opens, you can enjoy 🍭 Candy :)
