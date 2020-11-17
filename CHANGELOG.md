# 0.1.0

* This is our initial release of Candy! 🎉
* For now, Candy supports the following language features:
  * imports using `use`
  * `class`es
  * `trait`s
  * `impl`s
  * `fun`ctions that can contain code
  * operators: `==`, `!=`, `<`, `>`, `<=`, `>=`, `is`
  * casts via `as`
  * a standard library including `Iterable`
* We provide the following tools:
  * A compiler that transpiles Candy code to Dart (a CLI tool named `candy2dart`).
  * An LSP server that provides syntax highlighting, jump-to-definition, info on hover, compilation and much more.
  * A VSCode extension that integrates the LSP. By invoking code actions (by default, that's <kbd>ctrl</kbd> + <kbd>.</kbd>), you can compile and run a Candy project.
* Candy is in its very early pre-alpha stage, so don't expect everything to work. Currently, it's a game of hit or miss.
  Just so you aren't disappointed, here are the most notable limitations of Candy as of now:
  * The type system is completely hacked together: Don't expect complex type constraint solving to work – for example, an `impl<T> Foo<T>: Map<T, List<T>>` doesn't make `Foo<String>` implement `Map<String, List<String>>`. Don't nest your types, duh!
  * The generated Dart code is not guaranteed to be runnable. If you generate invalid code, you have to guess what caused the error and fix it in Candy. Most notably, overloading functions doesn't generate valid Dart code.
