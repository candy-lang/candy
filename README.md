# 🍭 Candy

A sweet programming language that is minimalistic and still expressive and fun to work with.

[Join our Discord server.](https://discord.gg/5Vr4eAJ7gU)

## Quick introduction

* **Values are at the center of your computations.**
  Only some predefined types of immutable values exist.
  ```
  3    # Int
  "Hi" # String
  Blub # Symbol
  # TODO: List
  # TODO: Map
  # TODO: Set
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
  Loads of values can already be computed at compile-time.
  In your editor, you'll see the results on the right side as you type:
  ```
  foo = double 2  # foo = 4
  ```
* **Something better than traditional types.**
  The days of runtime errors like "logarithm only accepts positive numbers" or "first only works on non-empty lists" are over.
  In Candy, functions have to specify their needs exactly.
  ```
  efficientStringReverse string =
    needs (isString string)
    needs (isPalindrome string)
    string
  ```
* **Permanent fuzzing.**
  While editing your code, it's automatically tested with loads of inputs to see if one breaks the code. You'll be immediately notified of any unhandled inputs.
  ```
  foo a =
    needs (isInt a)
    logarithm a  # fails for a = 0
  ```

## The current state

We are currently implementing a first version of Candy in Rust.
We already have a language server that provides some tooling.

Our TODO list:

* [x] build a basic parser
* [x] lower CST to AST
* [x] lower AST to HIR
* [x] build a basic interpreter
* [x] add CLI arguments for printing the CST, AST, or HIR
* [ ] make functions independent of their order in source where possible
* [ ] support importing other files
* [ ] namespaces/modules including visibility modifiers
* [ ] IDE support:
  * [ ] completion, completion resolve
  * [ ] hover
  * [ ] signatureHelp
  * [ ] declaration, definition
  * [ ] typeDefinition
  * [ ] implementation
  * [ ] references
  * [ ] documentHighlight
  * [ ] documentSymbol
  * [ ] codeAction, codeAction resolve
  * [ ] codeLens, codeLens resolve, codeLens refresh
  * [ ] documentLink, documentLink resolve
  * [ ] documentColor
  * [ ] colorPresentation
  * [ ] formatting
  * [ ] rangeFormatting
  * [ ] onTypeFormatting
  * [ ] rename, prepareRename
  * [x] foldingRange
  * [ ] selectionRange
  * [ ] prepareCallHierarchy
  * [ ] callHierarchy incoming, callHierarchy outgoing
  * [x] semantic tokens
  * [ ] linkedEditingRange
  * [ ] moniker
* [x] incremental compilation
* [ ] lists
* [ ] maps
* [ ] sets
* [ ] string interpolation
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
4. Run the launch config “Run Extension (Rust-Compiler: vscode_extension)”.
5. When you are prompted to select the build task, choose “npm: watch (1) Rust-Compiler: vscode_extension” (should be the first entry).
6. In the new VS Code window that opens, you can enjoy 🍭 Candy :)
