# Packages that are written in Candy

To run a file, execute the following command inside this directory:

```sh
# Rust compiler is faster, Candy is slower:
cargo run -- run <file>
# Rust compiler is slower, Candy is faster:
cargo run --release -- run <file>
```

`<file>` is the file you want to run, e.g., `./Examples/echo.candy`.
