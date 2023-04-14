## Number of expressions in optimized MIR

when                  | type | todo | Core  | fibonacci
----------------------|------|------|-------|----------
before                |   74 | 4696 | 37303 |     37446
more constant folding |   74 | 4650 | 43690 |     43832
StructHasKey          |   17 |   72 |  4933 |       245
correct typeOf        |   47 |   72 |  4933 |       245


## Tracing visualization

debugging: from trace file

What we want:
- custom fast rendering, no UI lag
- visualize even very large traces
- hot reload of traces
- two modes
  - driven from the outside (the editor): live updates can be pushed to the trace visualization
  - self-contained: visualizes a fixed trace

Architecture:
- use low-level web stack (WASM, canvas)
- parsing and rendering logic written in Rust, compiled to WASM
- multiple components (running in parallel, possibly via WebWorkers)
  - trace server working with data and making data available
  - UI requesting data from server

