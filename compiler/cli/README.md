# 🍭 Candy CLI

## Profiling

Create a [flamegraph](https://github.com/flamegraph-rs/flamegraph#readme):

```bash
cargo flamegraph --bin=candy --deterministic --output=<output file name> -- run <candy file>
# For example:
cargo flamegraph --bin=candy --deterministic --output=flamegraph.svg -- run packages/Examples/fibonacci.candy
```

Use [hyperfine](https://github.com/sharkdp/hyperfine#readme):

```bash
hyperfine --warmup 1 "cargo run --release -- run <candy file>"
# For example:
hyperfine --warmup 1 "cargo run --release -- run packages/Examples/fibonacci.candy"
```
