# 🍭 Candy

A sweet programming language, mainly inspired by Kotlin, Rust and Dart.

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
