# How to fuzz the compiler

The fuzzing in this repo is set up by following the [Rust Fuzz Book](https://rust-fuzz.github.io/book/).
These steps should suffice to get you up and running:

1. Change to Rust nightly:

   ```bash
   rustup install nightly
   rustup default nightly
   ```

2. Install the `cargo fuzz` tool:

   ```bash
   cargo install cargo-fuzz
   ```

3. List the available fuzz cases:

   ```bash
   cargo fuzz list
   ```

4. Run a fuzzer:

   ```bash
   cargo fuzz run fuzz_vm
   ```
