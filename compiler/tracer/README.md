# Tracer

This is the tracer used for visualizing traces of Candy programs.

## How to build

Install necessary stuff:

1. Install [`wasm-pack`](https://rustwasm.github.io/wasm-pack/): `curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh`
2. Install `basic-http-server`: `cargo install basic-http-server`

Compile and run the project:

1. Navigate to the directory containing this README.
2. `wasm-pack build --target web` (generates WASM modules and binding in a `pkg` folder)
3. `basic-http-server` (just serves the current directory as a webserver)
