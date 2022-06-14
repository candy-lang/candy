# 🍭 Candy

A sweet programming language that is robst, minimalistic, and expressive.

Candy aims to blur the line between dynamically-typed and statically-typed languages.
Like dynamic languages, it is permissive during compilation, allowing you to quickly prototype new ideas.
You can freely compose data without having to specify its structure before.
Like static languages, the tooling highlights potential errors before they happen.

## Quick introduction

* **Values are at the center of your computations.**
  Only some predefined types of immutable values exist: ints, texts, symbols, and structs.
  ```
  3
  "Candy"
  Green
  { Name: "Candy" }
  ```
* **Minimalistic syntax.**
  Defining variables and functions all works without braces cluttering up your code.
  The syntax is indentation-aware.
  ```
  foo = a
  println message =
    print message
    print "\n"
  println "Hello, world!"
  ```
* **Extensive compile-time evaluation.**
  Many values can already be computed at compile-time.
  In your editor, you'll see the results on the right side as you type:
  ```
  foo = double 2  # foo = 4
  ```
* **Something better than traditional types.**
  The days of runtime errors like "logarithm only accepts positive numbers" or "first only works on non-empty lists" are over.
  In Candy, functions have to specify their needs *exactly.*
  ```
  efficientTextReverse text =
    needs (isText text)
    needs (isPalindrome text)
    text
  ```
* **Permanent fuzzing.**
  While editing your code, the tooling automatically tests it with many input to see if one breaks the code.
  You'll be immediately notified of any unhandled inputs.
  This is how the tooling could look like:
  ```
  foo a =            # If you pass a = 0, ...
    needs (isInt a)
    logarithm a      # ... then this fails because logarithm only works on positive numbers.
  ```

## Discussion

[Join our Discord server.](https://discord.gg/5Vr4eAJ7gU)

## The current state

We are currently implementing a first version of Candy in Rust.
We already have a language server that provides some tooling.

Our TODO list:

* [x] build a basic parser
* [x] lower CST to AST
* [x] lower AST to HIR
* [x] build a basic interpreter
* [x] add CLI arguments for printing the CST, AST, or HIR
* [ ] make functions independent of their order in top-level scope
* [x] support importing other files (`use`)
* [ ] namespaces/modules including visibility modifiers
* [ ] IDE support:
  * [ ] completion, completion resolve
  * [ ] hover
  * [ ] signatureHelp
  * [x] ~~declaration~~, definition, ~~typeDefinition~~
  * [ ] implementation
  * [x] references
  * [x] documentHighlight
  * [ ] documentSymbol
  * [ ] codeAction, codeAction resolve
  * [ ] codeLens, codeLens resolve, codeLens refresh
  * [ ] documentLink, documentLink resolve
  * [x] ~~documentColor, colorPresentation~~
  * [ ] formatting
  * [ ] rangeFormatting
  * [ ] onTypeFormatting
  * [ ] rename, prepareRename
  * [x] foldingRange
  * [ ] selectionRange
  * [ ] prepareCallHierarchy
  * [ ] callHierarchy incoming, callHierarchy outgoing
  * [x] semantic tokens
  * [x] ~~linkedEditingRange~~
  * [ ] moniker
* [x] incremental compilation
* [ ] lists
* [ ] maps
* [ ] sets
* [ ] text interpolation
* [ ] constant evaluation
* [ ] fibers
* [ ] channels
* [ ] io
* [ ] random
* [ ] standard library
* [ ] pipe operator
* [ ] auto-fuzzing
* [ ] "type" proofs
* [ ] testing
* [ ] fuzzing of the compiler itself
* [ ] clean up repo (delete a bunch of stuff!)

## How to use Candy

1. Install Rust.
2. Clone this repo.
3. Open the workspace in VS Code.
4. In the VS Code settings (JSON), add the following: `"candy.languageServerCommand": "cargo run --manifest-path <path-to-the-candy-folder>/compiler/Cargo.toml -- lsp",`.
5. Run `npm install` inside `vscode_extension/`.
6. Run the launch config “Run Extension (VS Code Extension)”.
7. In the new VS Code window that opens, you can enjoy 🍭 Candy :)
