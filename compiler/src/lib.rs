// Keep these in sync with `main.rs`!
#![feature(async_closure)]
#![feature(box_patterns)]
#![feature(entry_insert)]
#![feature(let_chains)]
#![feature(never_type)]
#![feature(try_trait_v2)]
#![allow(clippy::module_inception)]

pub mod builtin_functions;
pub mod compiler;
pub mod database;
pub mod fuzzer;
pub mod language_server;
pub mod module;
pub mod utils;
pub mod vm;
