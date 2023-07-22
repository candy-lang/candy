# How to fuzz the compiler

The fuzzing in this repo is set up by following the [Rust Fuzz Book](https://rust-fuzz.github.io/book/).
These steps should suffice to get you up and running:

1. Install the `cargo fuzz` tool:

   ```bash
   cargo install cargo-fuzz
   ```

2. List the available fuzz cases:

   ```bash
   cargo fuzz list
   ```

3. Run a fuzzer:

   ```bash
   cargo fuzz run fuzz_vm
   ```
