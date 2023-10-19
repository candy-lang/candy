# How to Fuzz the Compiler

The fuzzing in this repo is set up by following the [Rust Fuzz Book](https://rust-fuzz.github.io/book/).
These steps should suffice to get you up and running:

```bash
cargo install cargo-fuzz
cargo fuzz run vm
```
